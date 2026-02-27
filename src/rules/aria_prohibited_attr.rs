use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use std::collections::HashMap;
use std::sync::LazyLock;
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct AriaProhibitedAttr;

static METADATA: RuleMetadata = RuleMetadata {
    id: "aria-prohibited-attr",
    description: "ARIA attributes must not be used where they are prohibited",
    wcag_level: WcagLevel::A,
    wcag_criterion: "4.1.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html",
    default_severity: Severity::Error,
};

static PROHIBITED_ATTRS_BY_ROLE: LazyLock<HashMap<&'static str, &'static [&'static str]>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();
        map.insert("caption", &["aria-label", "aria-labelledby"] as &[&str]);
        map.insert("code", &["aria-label", "aria-labelledby"] as &[&str]);
        map.insert("definition", &["aria-label", "aria-labelledby"] as &[&str]);
        map.insert("deletion", &["aria-label", "aria-labelledby"] as &[&str]);
        map.insert("emphasis", &["aria-label", "aria-labelledby"] as &[&str]);
        map.insert(
            "generic",
            &["aria-label", "aria-labelledby", "aria-roledescription"] as &[&str],
        );
        map.insert("insertion", &["aria-label", "aria-labelledby"] as &[&str]);
        map.insert("none", &["aria-label", "aria-labelledby"] as &[&str]);
        map.insert("paragraph", &["aria-label", "aria-labelledby"] as &[&str]);
        map.insert(
            "presentation",
            &["aria-label", "aria-labelledby"] as &[&str],
        );
        map.insert("strong", &["aria-label", "aria-labelledby"] as &[&str]);
        map.insert("subscript", &["aria-label", "aria-labelledby"] as &[&str]);
        map.insert("superscript", &["aria-label", "aria-labelledby"] as &[&str]);
        map.insert("term", &["aria-label", "aria-labelledby"] as &[&str]);
        map.insert("time", &["aria-label", "aria-labelledby"] as &[&str]);
        map
    });

impl Rule for AriaProhibitedAttr {
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
        check_html_element(node, source, diagnostics);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diagnostics);
    }
}

fn check_html_element(element: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" || child.kind() == "self_closing_tag" {
            check_html_start_tag(&child, source, diagnostics, element);
        }
    }
}

fn check_html_start_tag(
    start_tag: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    element_node: &Node,
) {
    let mut role_value: Option<String> = None;
    let mut aria_attrs: Vec<String> = Vec::new();

    let mut cursor = start_tag.walk();
    for child in start_tag.children(&mut cursor) {
        if child.kind() == "attribute" {
            let (attr_name, attr_value) = extract_html_attribute(&child, source);
            if let Some(ref name) = attr_name {
                let lower = name.to_lowercase();
                if lower.starts_with("aria-") {
                    aria_attrs.push(lower);
                }
                if name.eq_ignore_ascii_case("role")
                    && let Some(val) = attr_value
                {
                    role_value = Some(val);
                }
            }
        }
    }

    if let Some(role) = role_value {
        check_prohibited_attrs(&role, &aria_attrs, element_node, diagnostics);
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
        }
    }

    (name, value)
}

// ---------------------------------------------------------------------------
// JSX / TSX
// ---------------------------------------------------------------------------

fn visit_jsx(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    match node.kind() {
        "jsx_self_closing_element" => {
            check_jsx_self_closing(node, source, diagnostics);
        }
        "jsx_element" => {
            check_jsx_element(node, source, diagnostics);
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_jsx(&child, source, diagnostics);
    }
}

fn check_jsx_self_closing(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut role_value: Option<String> = None;
    let mut aria_attrs: Vec<String> = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_attribute" {
            let (attr_name, attr_value) = extract_jsx_attribute(&child, source);
            if let Some(ref name) = attr_name {
                if name.starts_with("aria-") {
                    aria_attrs.push(name.clone());
                }
                if name == "role"
                    && let Some(val) = attr_value
                {
                    role_value = Some(val);
                }
            }
        }
    }

    if let Some(role) = role_value {
        check_prohibited_attrs(&role, &aria_attrs, node, diagnostics);
    }
}

fn check_jsx_element(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            let mut role_value: Option<String> = None;
            let mut aria_attrs: Vec<String> = Vec::new();

            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "jsx_attribute" {
                    let (attr_name, attr_value) = extract_jsx_attribute(&inner_child, source);
                    if let Some(ref name) = attr_name {
                        if name.starts_with("aria-") {
                            aria_attrs.push(name.clone());
                        }
                        if name == "role"
                            && let Some(val) = attr_value
                        {
                            role_value = Some(val);
                        }
                    }
                }
            }

            if let Some(role) = role_value {
                check_prohibited_attrs(&role, &aria_attrs, node, diagnostics);
            }
        }
    }
}

/// Extract (attribute_name, Option<string_value>) from a JSX attribute node.
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

fn check_prohibited_attrs(
    role: &str,
    aria_attrs: &[String],
    node: &Node,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(prohibited) = PROHIBITED_ATTRS_BY_ROLE.get(role) {
        for attr in aria_attrs {
            let attr_lower = attr.to_lowercase();
            if prohibited
                .iter()
                .any(|p| p.eq_ignore_ascii_case(&attr_lower))
            {
                diagnostics.push(make_diagnostic(node, role, &attr_lower));
            }
        }
    }
}

fn make_diagnostic(node: &Node, role: &str, attr: &str) -> Diagnostic {
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
            "Attribute '{}' is prohibited on role '{}'. {} [WCAG {} Level {:?}]",
            attr, role, meta.description, meta.wcag_criterion, meta.wcag_level
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
        let rule = AriaProhibitedAttr;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AriaProhibitedAttr;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_generic_with_aria_label_fails() {
        let diags = check_html(r#"<div role="generic" aria-label="test"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-prohibited-attr".to_string()))
        );
        assert!(diags[0].message.contains("aria-label"));
        assert!(diags[0].message.contains("generic"));
    }

    #[test]
    fn test_generic_with_aria_hidden_passes() {
        let diags = check_html(r#"<div role="generic" aria-hidden="true"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_presentation_with_aria_labelledby_fails() {
        let diags = check_html(r#"<div role="presentation" aria-labelledby="x"></div>"#);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("aria-labelledby"));
    }

    #[test]
    fn test_button_with_aria_label_passes() {
        let diags = check_html(r#"<div role="button" aria-label="test"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_none_with_aria_label_fails() {
        let diags = check_html(r#"<div role="none" aria-label="test"></div>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_emphasis_with_aria_label_fails() {
        let diags = check_html(r#"<span role="emphasis" aria-label="test"></span>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_generic_with_aria_label_fails() {
        let diags = check_tsx(r#"const App = () => <div role="generic" aria-label="test" />;"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-prohibited-attr".to_string()))
        );
    }

    #[test]
    fn test_tsx_button_with_aria_label_passes() {
        let diags = check_tsx(r#"const App = () => <div role="button" aria-label="test" />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_generic_with_multiple_prohibited_attrs() {
        let diags = check_html(
            r#"<div role="generic" aria-label="test" aria-labelledby="x" aria-roledescription="y"></div>"#,
        );
        assert_eq!(diags.len(), 3);
    }

    #[test]
    fn test_no_role_skips_check() {
        let diags = check_html(r#"<div aria-label="test"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_unknown_role_skips_check() {
        let diags = check_html(r#"<div role="unknownrole" aria-label="test"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_element_with_prohibited_attr() {
        let diags = check_tsx(
            r#"const App = () => <div role="presentation" aria-labelledby="x">content</div>;"#,
        );
        assert_eq!(diags.len(), 1);
    }
}
