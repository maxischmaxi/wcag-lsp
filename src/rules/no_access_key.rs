use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct NoAccessKey;

static METADATA: RuleMetadata = RuleMetadata {
    id: "no-access-key",
    description: "accesskey attribute should not be used",
    wcag_level: WcagLevel::A,
    wcag_criterion: "2.4.3",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/focus-order.html",
    default_severity: Severity::Warning,
};

impl Rule for NoAccessKey {
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
            let name = &source[child.byte_range()];
            if name.eq_ignore_ascii_case("accesskey") {
                diagnostics.push(make_diagnostic(node));
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
            if name == "accessKey" || name == "accesskey" {
                diagnostics.push(make_diagnostic(node));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

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
        let rule = NoAccessKey;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = NoAccessKey;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_element_with_accesskey_fails() {
        let diags = check_html(r#"<button accesskey="s">Save</button>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("no-access-key".to_string()))
        );
    }

    #[test]
    fn test_element_without_accesskey_passes() {
        let diags = check_html(r#"<button>Save</button>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_elements_passes() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_element_with_accesskey_fails() {
        let diags = check_tsx(r#"const App = () => <button accessKey="s">Save</button>;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_element_without_accesskey_passes() {
        let diags = check_tsx(r#"const App = () => <button>Save</button>;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_self_closing_with_accesskey_fails() {
        let diags = check_tsx(r#"const App = () => <input accessKey="s" />;"#);
        assert_eq!(diags.len(), 1);
    }
}
