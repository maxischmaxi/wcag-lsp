use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use std::collections::HashSet;
use std::sync::LazyLock;
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct AriaDeprecatedRole;

static METADATA: RuleMetadata = RuleMetadata {
    id: "aria-deprecated-role",
    description: "ARIA role must not be a deprecated role value",
    wcag_level: WcagLevel::A,
    wcag_criterion: "4.1.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html",
    default_severity: Severity::Warning,
};

static DEPRECATED_ROLES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let roles = ["directory", "doc-biblioentry", "doc-endnote"];
    roles.into_iter().collect()
});

impl Rule for AriaDeprecatedRole {
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
    let mut is_role = false;
    let mut value: Option<(String, Node)> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute_name" {
            let name = &source[child.byte_range()];
            if name.eq_ignore_ascii_case("role") {
                is_role = true;
            }
        }
        if child.kind() == "quoted_attribute_value" {
            let mut val_cursor = child.walk();
            for val_child in child.children(&mut val_cursor) {
                if val_child.kind() == "attribute_value" {
                    value = Some((source[val_child.byte_range()].to_string(), val_child));
                }
            }
        }
    }

    if is_role && let Some((val, val_node)) = value {
        check_role_value(&val, &val_node, diagnostics);
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
    let mut is_role = false;
    let mut value: Option<(String, Node)> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "property_identifier" {
            let name = &source[child.byte_range()];
            if name == "role" {
                is_role = true;
            }
        }
        if child.kind() == "string" {
            let raw = &source[child.byte_range()];
            let trimmed = raw.trim_matches('"').trim_matches('\'');
            value = Some((trimmed.to_string(), child));
        }
    }

    if is_role && let Some((val, val_node)) = value {
        check_role_value(&val, &val_node, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

fn check_role_value(value: &str, node: &Node, diagnostics: &mut Vec<Diagnostic>) {
    for role in value.split_whitespace() {
        if DEPRECATED_ROLES.contains(role) {
            diagnostics.push(make_diagnostic(node, role));
        }
    }
}

fn make_diagnostic(node: &Node, deprecated_role: &str) -> Diagnostic {
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
            "Deprecated ARIA role '{}'. {} [WCAG {} Level {:?}]",
            deprecated_role, meta.description, meta.wcag_criterion, meta.wcag_level
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
        let rule = AriaDeprecatedRole;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AriaDeprecatedRole;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_deprecated_directory_role_fails() {
        let diags = check_html(r#"<div role="directory"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-deprecated-role".to_string()))
        );
    }

    #[test]
    fn test_valid_button_role_passes() {
        let diags = check_html(r#"<div role="button"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_valid_navigation_role_passes() {
        let diags = check_html(r#"<div role="navigation"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_role_attribute_passes() {
        let diags = check_html(r#"<div></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_deprecated_doc_biblioentry_fails() {
        let diags = check_html(r#"<div role="doc-biblioentry"></div>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_deprecated_doc_endnote_fails() {
        let diags = check_html(r#"<div role="doc-endnote"></div>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_deprecated_directory_role_fails() {
        let diags = check_tsx(r#"const App = () => <div role="directory" />;"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-deprecated-role".to_string()))
        );
    }

    #[test]
    fn test_tsx_valid_button_role_passes() {
        let diags = check_tsx(r#"const App = () => <div role="button" />;"#);
        assert_eq!(diags.len(), 0);
    }
}
