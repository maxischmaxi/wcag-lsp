use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct AriaHiddenBody;

static METADATA: RuleMetadata = RuleMetadata {
    id: "aria-hidden-body",
    description: "<body> must not have aria-hidden=\"true\"",
    wcag_level: WcagLevel::A,
    wcag_criterion: "4.1.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html",
    default_severity: Severity::Error,
};

impl Rule for AriaHiddenBody {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(&self, root: &Node, source: &str, file_type: FileType) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        if file_type.is_jsx_like() {
            visit_jsx(root, source, &mut diagnostics);
        } else {
            visit_html(root, source, &mut diagnostics);
        }
        diagnostics
    }
}

// ---------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------

fn visit_html(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "element" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "start_tag" {
                check_html_start_tag(&child, source, diagnostics, node);
            }
        }
    }

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
    let mut is_body = false;
    let mut has_aria_hidden_true = false;

    let mut cursor = start_tag.walk();
    for child in start_tag.children(&mut cursor) {
        if child.kind() == "tag_name" {
            let name = &source[child.byte_range()];
            if name.eq_ignore_ascii_case("body") {
                is_body = true;
            }
        }
        if child.kind() == "attribute" {
            let mut attr_cursor = child.walk();
            let mut is_aria_hidden = false;
            let mut value_is_true = false;

            for attr_child in child.children(&mut attr_cursor) {
                if attr_child.kind() == "attribute_name" {
                    let attr_name = &source[attr_child.byte_range()];
                    if attr_name.eq_ignore_ascii_case("aria-hidden") {
                        is_aria_hidden = true;
                    }
                }
                if attr_child.kind() == "quoted_attribute_value" {
                    let mut val_cursor = attr_child.walk();
                    for val_child in attr_child.children(&mut val_cursor) {
                        if val_child.kind() == "attribute_value" {
                            let val = &source[val_child.byte_range()];
                            if val.eq_ignore_ascii_case("true") {
                                value_is_true = true;
                            }
                        }
                    }
                }
            }

            if is_aria_hidden && value_is_true {
                has_aria_hidden_true = true;
            }
        }
    }

    if is_body && has_aria_hidden_true {
        diagnostics.push(make_diagnostic(element_node));
    }
}

// ---------------------------------------------------------------------------
// JSX / TSX
// ---------------------------------------------------------------------------

fn visit_jsx(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "jsx_element" {
        check_jsx_element(node, source, diagnostics);
    }
    if node.kind() == "jsx_self_closing_element" {
        check_jsx_self_closing(node, source, diagnostics);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_jsx(&child, source, diagnostics);
    }
}

fn check_jsx_element(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            check_jsx_opening_or_self_closing(&child, source, diagnostics, node);
        }
    }
}

fn check_jsx_self_closing(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    check_jsx_opening_or_self_closing(node, source, diagnostics, node);
}

fn check_jsx_opening_or_self_closing(
    tag_node: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    report_node: &Node,
) {
    let mut is_body = false;
    let mut has_aria_hidden_true = false;

    let mut cursor = tag_node.walk();
    for child in tag_node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = &source[child.byte_range()];
            if name == "body" {
                is_body = true;
            }
        }
        if child.kind() == "jsx_attribute" {
            let mut attr_cursor = child.walk();
            let mut is_aria_hidden = false;
            let mut value_is_true = false;

            for attr_child in child.children(&mut attr_cursor) {
                if attr_child.kind() == "property_identifier" {
                    let name = &source[attr_child.byte_range()];
                    if name == "aria-hidden" {
                        is_aria_hidden = true;
                    }
                }
                if attr_child.kind() == "string" {
                    let raw = &source[attr_child.byte_range()];
                    let trimmed = raw.trim_matches('"').trim_matches('\'');
                    if trimmed.eq_ignore_ascii_case("true") {
                        value_is_true = true;
                    }
                }
            }

            if is_aria_hidden && value_is_true {
                has_aria_hidden_true = true;
            }
        }
    }

    if is_body && has_aria_hidden_true {
        diagnostics.push(make_diagnostic(report_node));
    }
}

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

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
        let rule = AriaHiddenBody;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AriaHiddenBody;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_body_with_aria_hidden_true_fails() {
        let diags = check_html(r#"<body aria-hidden="true"></body>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-hidden-body".to_string()))
        );
    }

    #[test]
    fn test_body_without_aria_hidden_passes() {
        let diags = check_html("<body></body>");
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_body_with_aria_hidden_false_passes() {
        let diags = check_html(r#"<body aria-hidden="false"></body>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_div_with_aria_hidden_true_passes() {
        let diags = check_html(r#"<div aria-hidden="true"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_nested_body_with_aria_hidden_true_fails() {
        let diags = check_html(r#"<html><body aria-hidden="true"><p>text</p></body></html>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_body_with_aria_hidden_true_fails() {
        let diags = check_tsx(r#"const App = () => <body aria-hidden="true"><p>text</p></body>;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_div_with_aria_hidden_true_passes() {
        let diags = check_tsx(r#"const App = () => <div aria-hidden="true" />;"#);
        assert_eq!(diags.len(), 0);
    }
}
