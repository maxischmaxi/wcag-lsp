use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use std::collections::HashMap;
use std::sync::LazyLock;
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct NoRedundantRoles;

static METADATA: RuleMetadata = RuleMetadata {
    id: "no-redundant-roles",
    description: "Elements should not have redundant ARIA roles",
    wcag_level: WcagLevel::A,
    wcag_criterion: "4.1.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html",
    default_severity: Severity::Warning,
};

static IMPLICIT_ROLES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mappings = [
        ("button", "button"),
        ("a", "link"),
        ("nav", "navigation"),
        ("main", "main"),
        ("header", "banner"),
        ("footer", "contentinfo"),
        ("aside", "complementary"),
        ("form", "form"),
        ("article", "article"),
        ("section", "region"),
        ("ul", "list"),
        ("ol", "list"),
        ("li", "listitem"),
        ("table", "table"),
        ("img", "img"),
        ("input", "textbox"),
        ("select", "combobox"),
    ];
    mappings.into_iter().collect()
});

impl Rule for NoRedundantRoles {
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
            if child.kind() == "start_tag" || child.kind() == "self_closing_tag" {
                check_html_tag(&child, source, diagnostics, node);
            }
        }
    }

    // Recurse into children
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
    let mut tag_name: Option<String> = None;
    let mut role_value: Option<String> = None;

    let mut cursor = tag.walk();
    for child in tag.children(&mut cursor) {
        if child.kind() == "tag_name" {
            tag_name = Some(source[child.byte_range()].to_ascii_lowercase());
        }
        if child.kind() == "attribute" {
            let (attr_name, attr_value) = extract_html_attribute(&child, source);
            if let Some(name) = attr_name
                && name.eq_ignore_ascii_case("role")
            {
                role_value = attr_value;
            }
        }
    }

    if let Some(ref name) = tag_name
        && let Some(ref role) = role_value
    {
        check_redundant_role(name, role, element_node, diagnostics);
    }
}

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
            if value.is_none() {
                value = Some(String::new());
            }
        }
    }

    (name, value)
}

// ---------------------------------------------------------------------------
// JSX / TSX
// ---------------------------------------------------------------------------

fn visit_jsx(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "jsx_self_closing_element" {
        check_jsx_self_closing(node, source, diagnostics);
    } else if node.kind() == "jsx_element" {
        check_jsx_opening(node, source, diagnostics);
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_jsx(&child, source, diagnostics);
    }
}

fn check_jsx_self_closing(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut tag_name: Option<String> = None;
    let mut role_value: Option<String> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            tag_name = Some(source[child.byte_range()].to_string());
        }
        if child.kind() == "jsx_attribute" {
            let (attr_name, attr_value) = extract_jsx_attribute(&child, source);
            if let Some(name) = attr_name
                && name == "role"
            {
                role_value = attr_value;
            }
        }
    }

    if let Some(ref name) = tag_name
        && let Some(ref role) = role_value
    {
        check_redundant_role(name, role, node, diagnostics);
    }
}

fn check_jsx_opening(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            let mut tag_name: Option<String> = None;
            let mut role_value: Option<String> = None;

            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "identifier" {
                    tag_name = Some(source[inner_child.byte_range()].to_string());
                }
                if inner_child.kind() == "jsx_attribute" {
                    let (attr_name, attr_value) = extract_jsx_attribute(&inner_child, source);
                    if let Some(name) = attr_name
                        && name == "role"
                    {
                        role_value = attr_value;
                    }
                }
            }

            if let Some(ref name) = tag_name
                && let Some(ref role) = role_value
            {
                check_redundant_role(name, role, node, diagnostics);
            }
        }
    }
}

fn extract_jsx_attribute(attr_node: &Node, source: &str) -> (Option<String>, Option<String>) {
    let mut name = None;
    let mut value = None;

    let mut cursor = attr_node.walk();
    for child in attr_node.children(&mut cursor) {
        if child.kind() == "property_identifier" {
            name = Some(source[child.byte_range()].to_string());
        }
        if child.kind() == "string" {
            let raw = &source[child.byte_range()];
            let trimmed = raw.trim_matches('"').trim_matches('\'');
            value = Some(trimmed.to_string());
        }
    }

    (name, value)
}

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

fn check_redundant_role(
    tag_name: &str,
    role: &str,
    node: &Node,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let tag_lower = tag_name.to_ascii_lowercase();
    if let Some(implicit_role) = IMPLICIT_ROLES.get(tag_lower.as_str())
        && role.eq_ignore_ascii_case(implicit_role)
    {
        diagnostics.push(make_diagnostic(node, &tag_lower, role));
    }
}

fn make_diagnostic(node: &Node, tag_name: &str, role: &str) -> Diagnostic {
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
            "Element '{}' has redundant role '{}'. {} [WCAG {} Level {:?}]",
            tag_name, role, meta.description, meta.wcag_criterion, meta.wcag_level
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
        let rule = NoRedundantRoles;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = NoRedundantRoles;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_button_with_redundant_role_fails() {
        let diags = check_html(r#"<button role="button">Click</button>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("no-redundant-roles".to_string()))
        );
    }

    #[test]
    fn test_button_with_different_role_passes() {
        let diags = check_html(r#"<button role="link">Click</button>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_div_with_role_button_passes() {
        let diags = check_html(r#"<div role="button">Click</div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_nav_with_redundant_role_fails() {
        let diags = check_html(r#"<nav role="navigation"><a href="/">Home</a></nav>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_no_role_attribute_passes() {
        let diags = check_html(r#"<button>Click</button>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_button_with_redundant_role_fails() {
        let diags = check_tsx(r#"const App = () => <button role="button">Click</button>;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_div_with_role_passes() {
        let diags = check_tsx(r#"const App = () => <div role="button" />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_nav_self_closing_with_redundant_role_fails() {
        let diags = check_tsx(r#"const App = () => <nav role="navigation" />;"#);
        assert_eq!(diags.len(), 1);
    }
}
