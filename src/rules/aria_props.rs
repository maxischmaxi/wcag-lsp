use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use std::collections::HashSet;
use std::sync::LazyLock;
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct AriaProps;

static METADATA: RuleMetadata = RuleMetadata {
    id: "aria-props",
    description: "ARIA attributes must be valid",
    wcag_level: WcagLevel::A,
    wcag_criterion: "4.1.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html",
    default_severity: Severity::Error,
};

static VALID_ARIA_ATTRS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let attrs = [
        "aria-activedescendant",
        "aria-atomic",
        "aria-autocomplete",
        "aria-braillelabel",
        "aria-brailleroledescription",
        "aria-busy",
        "aria-checked",
        "aria-colcount",
        "aria-colindex",
        "aria-colindextext",
        "aria-colspan",
        "aria-controls",
        "aria-current",
        "aria-describedby",
        "aria-description",
        "aria-details",
        "aria-disabled",
        "aria-dropeffect",
        "aria-errormessage",
        "aria-expanded",
        "aria-flowto",
        "aria-grabbed",
        "aria-haspopup",
        "aria-hidden",
        "aria-invalid",
        "aria-keyshortcuts",
        "aria-label",
        "aria-labelledby",
        "aria-level",
        "aria-live",
        "aria-modal",
        "aria-multiline",
        "aria-multiselectable",
        "aria-orientation",
        "aria-owns",
        "aria-placeholder",
        "aria-posinset",
        "aria-pressed",
        "aria-readonly",
        "aria-relevant",
        "aria-required",
        "aria-roledescription",
        "aria-rowcount",
        "aria-rowindex",
        "aria-rowindextext",
        "aria-rowspan",
        "aria-selected",
        "aria-setsize",
        "aria-sort",
        "aria-valuemax",
        "aria-valuemin",
        "aria-valuenow",
        "aria-valuetext",
    ];
    attrs.into_iter().collect()
});

impl Rule for AriaProps {
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
    if node.kind() == "attribute" {
        check_html_attribute(node, source, diagnostics);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diagnostics);
    }
}

fn check_html_attribute(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute_name" {
            let name = source[child.byte_range()].to_lowercase();
            if name.starts_with("aria-") && !VALID_ARIA_ATTRS.contains(name.as_str()) {
                diagnostics.push(make_diagnostic(&child, &name));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// JSX / TSX
// ---------------------------------------------------------------------------

fn visit_jsx(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "jsx_attribute" {
        check_jsx_attribute(node, source, diagnostics);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_jsx(&child, source, diagnostics);
    }
}

fn check_jsx_attribute(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "property_identifier" {
            let name = &source[child.byte_range()];
            if name.starts_with("aria-") && !VALID_ARIA_ATTRS.contains(name) {
                diagnostics.push(make_diagnostic(&child, name));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

fn make_diagnostic(node: &Node, invalid_attr: &str) -> Diagnostic {
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
            "Invalid ARIA attribute '{}'. {} [WCAG {} Level {:?}]",
            invalid_attr, meta.description, meta.wcag_criterion, meta.wcag_level
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
        let rule = AriaProps;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AriaProps;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_valid_aria_label_passes() {
        let diags = check_html(r#"<div aria-label="test"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_invalid_aria_attr_fails() {
        let diags = check_html(r#"<div aria-foo="bar"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-props".to_string()))
        );
    }

    #[test]
    fn test_multiple_valid_aria_attrs_passes() {
        let diags = check_html(r#"<div aria-hidden="true" aria-label="x"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_aria_attrs_passes() {
        let diags = check_html(r#"<div class="container"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_valid_aria_attr_passes() {
        let diags = check_tsx(r#"const App = () => <div aria-label="test" />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_invalid_aria_attr_fails() {
        let diags = check_tsx(r#"const App = () => <div aria-foo="bar" />;"#);
        assert_eq!(diags.len(), 1);
    }
}
