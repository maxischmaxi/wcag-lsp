use crate::rules::WcagLevel;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Suppression {
    all: bool,
    disable_a: bool,
    disable_aa: bool,
    disable_aaa: bool,
    rules: HashSet<String>,
}

impl Suppression {
    fn from_selectors(selectors: &str) -> Self {
        let mut suppression = Self::default();

        let tokens: Vec<_> = selectors
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|token| !token.is_empty())
            .collect();

        if tokens.is_empty() {
            suppression.all = true;
            return suppression;
        }

        for token in tokens {
            match token.to_ascii_uppercase().as_str() {
                "A" => suppression.disable_a = true,
                "AA" => suppression.disable_aa = true,
                "AAA" => suppression.disable_aaa = true,
                _ => {
                    suppression.rules.insert(token.to_ascii_lowercase());
                }
            }
        }

        suppression
    }

    fn merge(&mut self, other: Self) {
        self.all |= other.all;
        self.disable_a |= other.disable_a;
        self.disable_aa |= other.disable_aa;
        self.disable_aaa |= other.disable_aaa;
        self.rules.extend(other.rules);
    }

    fn disables(&self, rule_id: &str, level: WcagLevel) -> bool {
        self.all
            || match level {
                WcagLevel::A => self.disable_a,
                WcagLevel::AA => self.disable_aa,
                WcagLevel::AAA => self.disable_aaa,
            }
            || self.rules.contains(rule_id)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InlineDirectives {
    file: Suppression,
    line: BTreeMap<u32, Suppression>,
    next_line: BTreeMap<u32, Suppression>,
}

impl InlineDirectives {
    pub fn parse(source: &str) -> Self {
        let mut directives = Self::default();

        for comment in scan_header_comments(source) {
            for directive in parse_directives(comment.body) {
                if let DirectiveKind::Disable = directive.kind {
                    directives.file.merge(directive.suppression);
                }
            }
        }

        for comment in scan_comments(source) {
            for directive in parse_directives(comment.body) {
                match directive.kind {
                    DirectiveKind::Disable => {}
                    DirectiveKind::DisableLine => {
                        directives
                            .line
                            .entry(comment.end_line)
                            .or_default()
                            .merge(directive.suppression);
                    }
                    DirectiveKind::DisableNextLine => {
                        directives
                            .next_line
                            .entry(comment.end_line.saturating_add(1))
                            .or_default()
                            .merge(directive.suppression);
                    }
                }
            }
        }

        directives
    }

    pub fn disables_file_rule(&self, rule_id: &str, level: WcagLevel) -> bool {
        self.file.disables(rule_id, level)
    }

    pub fn disables_line_rule(&self, line: u32, rule_id: &str, level: WcagLevel) -> bool {
        self.line
            .get(&line)
            .map(|suppression| suppression.disables(rule_id, level))
            .unwrap_or(false)
            || self
                .next_line
                .get(&line)
                .map(|suppression| suppression.disables(rule_id, level))
                .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectiveKind {
    Disable,
    DisableLine,
    DisableNextLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Directive {
    kind: DirectiveKind,
    suppression: Suppression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Comment<'a> {
    body: &'a str,
    end_line: u32,
}

fn parse_directives(comment_body: &str) -> Vec<Directive> {
    let mut directives = Vec::new();

    for line in comment_body.lines() {
        let line = normalize_comment_line(line);

        if let Some(selectors) = parse_named_directive(line, "wcag-disable-line") {
            directives.push(Directive {
                kind: DirectiveKind::DisableLine,
                suppression: Suppression::from_selectors(selectors),
            });
            continue;
        }

        if let Some(selectors) = parse_named_directive(line, "wcag-disable-next-line") {
            directives.push(Directive {
                kind: DirectiveKind::DisableNextLine,
                suppression: Suppression::from_selectors(selectors),
            });
            continue;
        }

        if let Some(selectors) = parse_named_directive(line, "wcag-disable") {
            directives.push(Directive {
                kind: DirectiveKind::Disable,
                suppression: Suppression::from_selectors(selectors),
            });
        }
    }

    directives
}

fn normalize_comment_line(line: &str) -> &str {
    line.trim_start()
        .strip_prefix('*')
        .unwrap_or(line.trim_start())
        .trim_start()
}

fn parse_named_directive<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    if !line.starts_with(name) {
        return None;
    }

    let rest = &line[name.len()..];
    if rest.is_empty() || rest.starts_with(char::is_whitespace) {
        return Some(rest.trim());
    }

    None
}

fn scan_header_comments(source: &str) -> Vec<Comment<'_>> {
    let bytes = source.as_bytes();
    let mut comments = Vec::new();
    let mut i = 0;
    let mut line = 0u32;

    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            if bytes[i] == b'\n' {
                line += 1;
            }
            i += 1;
        }

        if i >= bytes.len() {
            break;
        }

        let Some((comment, next_i, next_line)) = scan_comment_at(source, i, line) else {
            break;
        };

        comments.push(comment);
        i = next_i;
        line = next_line;
    }

    comments
}

fn scan_comments(source: &str) -> Vec<Comment<'_>> {
    let bytes = source.as_bytes();
    let mut comments = Vec::new();
    let mut i = 0;
    let mut line = 0u32;

    while i < bytes.len() {
        if bytes[i] == b'\n' {
            line += 1;
            i += 1;
            continue;
        }

        if let Some((comment, next_i, next_line)) = scan_comment_at(source, i, line) {
            comments.push(comment);
            i = next_i;
            line = next_line;
            continue;
        }

        i += 1;
    }

    comments
}

fn scan_comment_at(
    source: &str,
    start: usize,
    start_line: u32,
) -> Option<(Comment<'_>, usize, u32)> {
    let bytes = source.as_bytes();

    if bytes[start..].starts_with(b"<!--") {
        let body_start = start + 4;
        let body_end = source[body_start..]
            .find("-->")
            .map(|offset| body_start + offset)
            .unwrap_or(source.len());
        let next_i = (body_end + 3).min(source.len());
        let end_line = start_line + count_newlines(&source[start..next_i]) as u32;
        return Some((
            Comment {
                body: &source[body_start..body_end],
                end_line,
            },
            next_i,
            end_line,
        ));
    }

    if bytes[start..].starts_with(b"/*") {
        let body_start = start + 2;
        let body_end = source[body_start..]
            .find("*/")
            .map(|offset| body_start + offset)
            .unwrap_or(source.len());
        let next_i = (body_end + 2).min(source.len());
        let end_line = start_line + count_newlines(&source[start..next_i]) as u32;
        return Some((
            Comment {
                body: &source[body_start..body_end],
                end_line,
            },
            next_i,
            end_line,
        ));
    }

    if bytes[start..].starts_with(b"//") {
        let body_start = start + 2;
        let body_end = source[body_start..]
            .find('\n')
            .map(|offset| body_start + offset)
            .unwrap_or(source.len());
        return Some((
            Comment {
                body: &source[body_start..body_end],
                end_line: start_line,
            },
            body_end,
            start_line,
        ));
    }

    None
}

fn count_newlines(text: &str) -> usize {
    text.bytes().filter(|byte| *byte == b'\n').count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_disable_all_from_html_comment() {
        let directives = InlineDirectives::parse(
            "<!-- wcag-disable -->\n<html><body><img src=\"x\"></body></html>",
        );

        assert!(directives.disables_file_rule("img-alt", WcagLevel::A));
        assert!(directives.disables_file_rule("heading-content", WcagLevel::AA));
    }

    #[test]
    fn test_header_disable_specific_level_and_rule() {
        let directives = InlineDirectives::parse(
            "/* wcag-disable AA img-alt */\nconst view = <img src=\"x\" />;",
        );

        assert!(directives.disables_file_rule("img-alt", WcagLevel::A));
        assert!(directives.disables_file_rule("heading-content", WcagLevel::AA));
        assert!(!directives.disables_file_rule("anchor-content", WcagLevel::A));
    }

    #[test]
    fn test_header_disable_ignored_after_content() {
        let directives =
            InlineDirectives::parse("<html>\n<!-- wcag-disable -->\n<img src=\"x\">\n</html>");

        assert!(!directives.disables_file_rule("img-alt", WcagLevel::A));
    }

    #[test]
    fn test_next_line_disable_for_html_comment() {
        let directives = InlineDirectives::parse(
            "<html>\n<!-- wcag-disable-next-line img-alt -->\n<img src=\"x\">\n</html>",
        );

        assert!(directives.disables_line_rule(2, "img-alt", WcagLevel::A));
        assert!(!directives.disables_line_rule(2, "anchor-content", WcagLevel::A));
    }

    #[test]
    fn test_next_line_uses_comment_end_line_for_block_comments() {
        let directives =
            InlineDirectives::parse("/*\n * wcag-disable-next-line img-alt\n */\n<img src=\"x\">");

        assert!(directives.disables_line_rule(3, "img-alt", WcagLevel::A));
        assert!(!directives.disables_line_rule(1, "img-alt", WcagLevel::A));
    }

    #[test]
    fn test_disable_line_targets_current_line() {
        let directives =
            InlineDirectives::parse("<div><!-- wcag-disable-line img-alt --><img src=\"x\"></div>");

        assert!(directives.disables_line_rule(0, "img-alt", WcagLevel::A));
        assert!(!directives.disables_line_rule(1, "img-alt", WcagLevel::A));
    }
}
