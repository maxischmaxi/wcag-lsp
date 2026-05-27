use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::html_attrs;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct TableHeader;

static METADATA: RuleMetadata = RuleMetadata {
    id: "table-header",
    description: "Tables must have header cells",
    wcag_level: WcagLevel::A,
    wcag_criterion: "1.3.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html",
    default_severity: Severity::Warning,
};

impl Rule for TableHeader {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(&self, root: &Node, source: &str, file_type: FileType) -> Vec<Diagnostic> {
        // HTML-only rule
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
        check_html_element(node, source, diagnostics);
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diagnostics);
    }
}

fn check_html_element(element: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    // `element_tag_name` resolves both `start_tag` and `self_closing_tag`.
    let is_table = html_attrs::element_tag_name(element, source)
        .is_some_and(|n| n.eq_ignore_ascii_case("table"));
    if !is_table {
        return;
    }

    // Check if any descendant element is a <th>
    if has_th_descendant(element, source) {
        return;
    }

    diagnostics.push(make_diagnostic(element));
}

/// Recursively check whether the element contains a <th> descendant.
fn has_th_descendant(node: &Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "element" {
            let is_th = html_attrs::element_tag_name(&child, source)
                .is_some_and(|n| n.eq_ignore_ascii_case("th"));
            if is_th {
                return true;
            }
            // Recurse into nested elements
            if has_th_descendant(&child, source) {
                return true;
            }
        }
    }
    false
}

fn make_diagnostic(node: &Node) -> Diagnostic {
    let meta = &METADATA;
    Diagnostic {
        range: node_to_range(node),
        severity: Some(DiagnosticSeverity::WARNING),
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
        let rule = TableHeader;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_vue(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Vue).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = TableHeader;
        rule.check(&tree.root_node(), source, FileType::Vue)
    }

    #[test]
    fn test_vue_table_with_th_passes() {
        let diags = check_vue(
            r#"<template><table><tr><th>Header</th></tr><tr><td>Data</td></tr></table></template>"#,
        );
        assert_eq!(diags.len(), 0, "table with <th> should pass in Vue, got: {diags:?}");
    }

    #[test]
    fn test_vue_table_without_th_fails() {
        let diags = check_vue(r#"<template><table><tr><td>Data</td></tr></table></template>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("table-header".to_string()))
        );
    }

    #[test]
    fn test_table_without_th_fails() {
        let diags = check_html(r#"<table><tr><td>Data</td></tr></table>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("table-header".to_string()))
        );
    }

    #[test]
    fn test_table_with_th_passes() {
        let diags = check_html(r#"<table><tr><th>Header</th></tr><tr><td>Data</td></tr></table>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_table_passes() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_table_with_thead_th_passes() {
        let diags = check_html(
            r#"<table><thead><tr><th>Header</th></tr></thead><tbody><tr><td>Data</td></tr></tbody></table>"#,
        );
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_multiple_tables_mixed() {
        let diags =
            check_html(r#"<table><tr><th>H</th></tr></table><table><tr><td>D</td></tr></table>"#);
        assert_eq!(diags.len(), 1);
    }
}
