use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::html_attrs;
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
    if node.kind() == "element"
        && let Some(tag) = html_attrs::element_tag(node)
    {
        check_html_tag(&tag, source, diagnostics, node);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diagnostics);
    }
}

fn check_html_tag(
    tag: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    element_node: &Node,
) {
    let is_body = html_attrs::tag_name(tag, source).is_some_and(|n| n.eq_ignore_ascii_case("body"));
    if !is_body {
        return;
    }

    // A bound `:aria-hidden`/`v-bind:aria-hidden` is a runtime expression whose
    // value we cannot evaluate literally — skip it. Only a static
    // `aria-hidden="true"` should be flagged.
    let has_aria_hidden_true = html_attrs::attrs(tag, source).iter().any(|a| {
        a.name_eq("aria-hidden")
            && !a.bound
            && a.value.as_deref().is_some_and(|v| v.eq_ignore_ascii_case("true"))
    });

    if has_aria_hidden_true {
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

    fn check_vue(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Vue).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AriaHiddenBody;
        rule.check(&tree.root_node(), source, FileType::Vue)
    }

    #[test]
    fn test_vue_bound_aria_hidden_body_not_flagged() {
        // `:aria-hidden` is a dynamic expression — value unknown, must not flag.
        let diags = check_vue(r#"<template><body :aria-hidden="hidden"></body></template>"#);
        assert_eq!(diags.len(), 0, "bound :aria-hidden must not flag, got: {diags:?}");
    }

    #[test]
    fn test_vue_static_aria_hidden_true_body_fails() {
        let diags = check_vue(r#"<template><body aria-hidden="true"></body></template>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-hidden-body".to_string()))
        );
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
