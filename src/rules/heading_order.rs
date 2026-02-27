use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct HeadingOrder;

static METADATA: RuleMetadata = RuleMetadata {
    id: "heading-order",
    description: "Heading levels should not be skipped",
    wcag_level: WcagLevel::A,
    wcag_criterion: "1.3.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html",
    default_severity: Severity::Warning,
};

impl Rule for HeadingOrder {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(&self, root: &Node, source: &str, file_type: FileType) -> Vec<Diagnostic> {
        let mut headings = Vec::new();
        if file_type.is_jsx_like() {
            collect_headings_jsx(root, source, &mut headings);
        } else {
            collect_headings_html(root, source, &mut headings);
        }

        let mut diagnostics = Vec::new();
        let mut prev_level: u8 = 0;

        for (level, node_range) in &headings {
            if *level > prev_level + 1 {
                diagnostics.push(make_diagnostic(*node_range, prev_level, *level));
            }
            prev_level = *level;
        }

        diagnostics
    }
}

/// Extract heading level from a tag name like "h1" .. "h6". Returns None if not a heading.
fn heading_level(tag_name: &str) -> Option<u8> {
    let lower = tag_name.to_ascii_lowercase();
    match lower.as_str() {
        "h1" => Some(1),
        "h2" => Some(2),
        "h3" => Some(3),
        "h4" => Some(4),
        "h5" => Some(5),
        "h6" => Some(6),
        _ => None,
    }
}

/// Collect all headings from an HTML AST in document order as (level, range) pairs.
fn collect_headings_html(node: &Node, source: &str, headings: &mut Vec<(u8, Range)>) {
    if node.kind() == "element" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "start_tag" {
                let mut tag_cursor = child.walk();
                for tag_child in child.children(&mut tag_cursor) {
                    if tag_child.kind() == "tag_name" {
                        let name = &source[tag_child.byte_range()];
                        if let Some(level) = heading_level(name) {
                            headings.push((level, node_to_range(node)));
                        }
                    }
                }
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_headings_html(&child, source, headings);
    }
}

/// Collect all headings from a JSX/TSX AST in document order as (level, range) pairs.
fn collect_headings_jsx(node: &Node, source: &str, headings: &mut Vec<(u8, Range)>) {
    if node.kind() == "jsx_opening_element" || node.kind() == "jsx_self_closing_element" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                let name = &source[child.byte_range()];
                if let Some(level) = heading_level(name) {
                    headings.push((level, node_to_range(node)));
                }
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_headings_jsx(&child, source, headings);
    }
}

fn make_diagnostic(range: Range, prev_level: u8, current_level: u8) -> Diagnostic {
    let meta = &METADATA;
    let expected = prev_level + 1;
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(NumberOrString::String("heading-order".to_string())),
        code_description: Some(CodeDescription {
            href: meta.wcag_url.parse().expect("valid URL"),
        }),
        source: Some("wcag-lsp".to_string()),
        message: format!(
            "Heading level h{} skipped (expected h{} or lower) [WCAG {} Level {:?}]",
            current_level, expected, meta.wcag_criterion, meta.wcag_level
        ),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn check_html(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Html).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = HeadingOrder;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    #[test]
    fn test_skipped_heading_level() {
        let diags = check_html("<h1>A</h1><h3>B</h3>");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("heading-order".to_string()))
        );
    }

    #[test]
    fn test_correct_heading_order() {
        let diags = check_html("<h1>A</h1><h2>B</h2><h3>C</h3>");
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_heading_starts_at_h2() {
        let diags = check_html("<h2>A</h2>");
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_no_headings() {
        let diags = check_html("<div><p>Hello</p></div>");
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_same_level_repeated() {
        let diags = check_html("<h1>A</h1><h1>B</h1>");
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_decreasing_levels_ok() {
        let diags = check_html("<h1>A</h1><h2>B</h2><h3>C</h3><h2>D</h2>");
        assert_eq!(diags.len(), 0);
    }
}
