use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct MetaRefresh;

static METADATA: RuleMetadata = RuleMetadata {
    id: "meta-refresh",
    description: "Do not use meta refresh with a time limit",
    wcag_level: WcagLevel::A,
    wcag_criterion: "2.2.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/timing-adjustable.html",
    default_severity: Severity::Error,
};

impl Rule for MetaRefresh {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(&self, root: &Node, source: &str, file_type: FileType) -> Vec<Diagnostic> {
        // This rule is HTML-only; JSX doesn't have meta tags in components.
        if file_type.is_jsx_like() {
            return Vec::new();
        }

        let mut diagnostics = Vec::new();
        visit_html(root, source, &mut diagnostics);
        diagnostics
    }
}

fn visit_html(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "element" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "start_tag" {
                check_html_start_tag(&child, source, diagnostics, node);
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diagnostics);
    }
}

fn check_html_start_tag(
    start_tag: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    element_node: &Node,
) {
    let mut is_meta = false;
    let mut is_refresh = false;
    let mut content_value: Option<String> = None;

    let mut cursor = start_tag.walk();
    for child in start_tag.children(&mut cursor) {
        if child.kind() == "tag_name" {
            let name = &source[child.byte_range()];
            if name.eq_ignore_ascii_case("meta") {
                is_meta = true;
            }
        }
        if child.kind() == "attribute" {
            let (attr_name, attr_value) = extract_html_attribute(&child, source);
            if let Some(name) = attr_name {
                if name.eq_ignore_ascii_case("http-equiv")
                    && let Some(ref val) = attr_value
                    && val.eq_ignore_ascii_case("refresh")
                {
                    is_refresh = true;
                }
                if name.eq_ignore_ascii_case("content") {
                    content_value = attr_value;
                }
            }
        }
    }

    if is_meta
        && is_refresh
        && let Some(ref content) = content_value
        && has_nonzero_delay(content)
    {
        diagnostics.push(make_diagnostic(element_node));
    }
}

/// Check whether the content attribute value starts with a number > 0.
/// content="0;url=/new" → false (immediate redirect, OK)
/// content="5" → true (5-second delay)
/// content="30;url=/new" → true (30-second delay)
fn has_nonzero_delay(content: &str) -> bool {
    let trimmed = content.trim();
    // Extract the leading number before any semicolon
    let num_part = if let Some(idx) = trimmed.find(';') {
        &trimmed[..idx]
    } else {
        trimmed
    };
    let num_part = num_part.trim();
    if let Ok(n) = num_part.parse::<u64>() {
        n > 0
    } else {
        false
    }
}

/// Extract (attribute_name, Option<attribute_value>) from an HTML attribute node.
fn extract_html_attribute(attr_node: &Node, source: &str) -> (Option<String>, Option<String>) {
    let mut name = None;
    let mut value = None;

    let mut cursor = attr_node.walk();
    for child in attr_node.children(&mut cursor) {
        if child.kind() == "attribute_name" {
            name = Some(source[child.byte_range()].to_string());
        }
        if child.kind() == "quoted_attribute_value" {
            let mut val_cursor = child.walk();
            for val_child in child.children(&mut val_cursor) {
                if val_child.kind() == "attribute_value" {
                    value = Some(source[val_child.byte_range()].to_string());
                }
            }
            // If we found a quoted_attribute_value but no inner attribute_value,
            // it's an empty string like content=""
            if value.is_none() {
                value = Some(String::new());
            }
        }
    }

    (name, value)
}

fn make_diagnostic(node: &Node) -> Diagnostic {
    let meta = &METADATA;
    Diagnostic {
        range: node_to_range(node),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(meta.id.to_string())),
        code_description: Some(CodeDescription {
            href: meta.wcag_url.parse().expect("valid URL"),
        }),
        source: Some("wcag-lsp".to_string()),
        message: format!(
            "{} [WCAG {} Level {:?}]",
            meta.description, meta.wcag_criterion, meta.wcag_level
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
        let rule = MetaRefresh;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    #[test]
    fn test_meta_refresh_with_delay_fails() {
        let diags = check_html(r#"<meta http-equiv="refresh" content="5">"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("meta-refresh".to_string()))
        );
    }

    #[test]
    fn test_meta_refresh_immediate_redirect_passes() {
        let diags = check_html(r#"<meta http-equiv="refresh" content="0;url=/new">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_meta_refresh_with_delay_and_url_fails() {
        let diags = check_html(r#"<meta http-equiv="refresh" content="30;url=/new">"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_meta_charset_passes() {
        let diags = check_html(r#"<meta charset="utf-8">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_meta_element_passes() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_jsx_skipped() {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let source = r#"const App = () => <div />;"#;
        let tree = parser.parse(source, None).unwrap();
        let rule = MetaRefresh;
        let diags = rule.check(&tree.root_node(), source, FileType::Tsx);
        assert_eq!(diags.len(), 0);
    }
}
