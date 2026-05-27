use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::html_attrs;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use std::collections::HashMap;
use std::sync::LazyLock;
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct AriaRequiredAttr;

static METADATA: RuleMetadata = RuleMetadata {
    id: "aria-required-attr",
    description: "Elements with ARIA roles must have all required ARIA attributes",
    wcag_level: WcagLevel::A,
    wcag_criterion: "4.1.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html",
    default_severity: Severity::Error,
};

static REQUIRED_ATTRS_BY_ROLE: LazyLock<HashMap<&'static str, Vec<&'static str>>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();
        map.insert("checkbox", vec!["aria-checked"]);
        map.insert("combobox", vec!["aria-expanded"]);
        map.insert("heading", vec!["aria-level"]);
        map.insert("meter", vec!["aria-valuenow"]);
        map.insert("option", vec!["aria-selected"]);
        map.insert("radio", vec!["aria-checked"]);
        map.insert("scrollbar", vec!["aria-controls", "aria-valuenow"]);
        map.insert("separator", vec!["aria-valuenow"]);
        map.insert("slider", vec!["aria-valuenow"]);
        map.insert("spinbutton", vec!["aria-valuenow"]);
        map.insert("switch", vec!["aria-checked"]);
        map
    });

impl Rule for AriaRequiredAttr {
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
    let attrs = html_attrs::element_attrs(element, source);

    let role = match attrs.iter().find(|a| a.name_eq("role")) {
        Some(r) => r,
        None => return,
    };

    // A bound role (`:role="x"`) is a runtime expression — can't validate it.
    if role.bound {
        return;
    }

    let role_value = match &role.value {
        Some(val) => val.clone(),
        None => return,
    };

    // A bound `:aria-x` still counts as the attribute being present.
    let present_attrs: Vec<String> = attrs.iter().map(|a| a.name_lower()).collect();

    check_required_attrs(&role_value, &present_attrs, element, diagnostics);
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
    let mut present_attrs: Vec<String> = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_attribute" {
            let (attr_name, attr_value) = extract_jsx_attribute(&child, source);
            if let Some(ref name) = attr_name {
                present_attrs.push(name.clone());
                if name == "role"
                    && let Some(val) = attr_value
                {
                    role_value = Some(val);
                }
            }
        }
    }

    if let Some(role) = role_value {
        check_required_attrs(&role, &present_attrs, node, diagnostics);
    }
}

fn check_jsx_element(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            let mut role_value: Option<String> = None;
            let mut present_attrs: Vec<String> = Vec::new();

            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "jsx_attribute" {
                    let (attr_name, attr_value) = extract_jsx_attribute(&inner_child, source);
                    if let Some(ref name) = attr_name {
                        present_attrs.push(name.clone());
                        if name == "role"
                            && let Some(val) = attr_value
                        {
                            role_value = Some(val);
                        }
                    }
                }
            }

            if let Some(role) = role_value {
                check_required_attrs(&role, &present_attrs, node, diagnostics);
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
            // JSX string values include the quotes; strip them
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

fn check_required_attrs(
    role: &str,
    present_attrs: &[String],
    node: &Node,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(required) = REQUIRED_ATTRS_BY_ROLE.get(role) {
        let missing: Vec<&str> = required
            .iter()
            .filter(|attr| !present_attrs.iter().any(|a| a.eq_ignore_ascii_case(attr)))
            .copied()
            .collect();

        if !missing.is_empty() {
            diagnostics.push(make_diagnostic(node, role, &missing));
        }
    }
}

fn make_diagnostic(node: &Node, role: &str, missing: &[&str]) -> Diagnostic {
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
            "Role '{}' requires attributes: {}. {} [WCAG {} Level {:?}]",
            role,
            missing.join(", "),
            meta.description,
            meta.wcag_criterion,
            meta.wcag_level
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
        let rule = AriaRequiredAttr;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AriaRequiredAttr;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    fn check_vue(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Vue).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AriaRequiredAttr;
        rule.check(&tree.root_node(), source, FileType::Vue)
    }

    #[test]
    fn test_vue_bound_aria_checked_passes() {
        let diags =
            check_vue(r#"<template><div role="checkbox" :aria-checked="checked"></div></template>"#);
        assert_eq!(diags.len(), 0, "bound :aria-checked should count as present, got: {diags:?}");
    }

    #[test]
    fn test_vue_static_missing_aria_checked_fails() {
        let diags = check_vue(r#"<template><div role="checkbox"></div></template>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_checkbox_with_aria_checked_passes() {
        let diags = check_html(r#"<div role="checkbox" aria-checked="true"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_checkbox_without_aria_checked_fails() {
        let diags = check_html(r#"<div role="checkbox"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-required-attr".to_string()))
        );
    }

    #[test]
    fn test_slider_without_aria_valuenow_fails() {
        let diags = check_html(r#"<div role="slider"></div>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_slider_with_aria_valuenow_passes() {
        let diags = check_html(r#"<div role="slider" aria-valuenow="50"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_div_without_role_passes() {
        let diags = check_html(r#"<div class="container"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_scrollbar_missing_both_attrs_fails() {
        let diags = check_html(r#"<div role="scrollbar"></div>"#);
        assert_eq!(diags.len(), 1);
        // The message should mention both missing attributes
        assert!(diags[0].message.contains("aria-controls"));
        assert!(diags[0].message.contains("aria-valuenow"));
    }

    #[test]
    fn test_scrollbar_with_all_attrs_passes() {
        let diags =
            check_html(r#"<div role="scrollbar" aria-controls="panel" aria-valuenow="50"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_heading_with_aria_level_passes() {
        let diags = check_html(r#"<div role="heading" aria-level="2"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_heading_without_aria_level_fails() {
        let diags = check_html(r#"<div role="heading"></div>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_unknown_role_passes() {
        let diags = check_html(r#"<div role="button"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_switch_without_aria_checked_fails() {
        let diags = check_html(r#"<div role="switch"></div>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_checkbox_with_aria_checked_passes() {
        let diags = check_tsx(r#"const App = () => <div role="checkbox" aria-checked="true" />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_checkbox_without_aria_checked_fails() {
        let diags = check_tsx(r#"const App = () => <div role="checkbox" />;"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-required-attr".to_string()))
        );
    }

    #[test]
    fn test_tsx_element_with_role_fails() {
        let diags = check_tsx(r#"const App = () => <div role="slider">content</div>;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_element_with_role_passes() {
        let diags =
            check_tsx(r#"const App = () => <div role="slider" aria-valuenow="50">content</div>;"#);
        assert_eq!(diags.len(), 0);
    }
}
