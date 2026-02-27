use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct AriaAllowedAttr;

static METADATA: RuleMetadata = RuleMetadata {
    id: "aria-allowed-attr",
    description: "ARIA attributes must be allowed for the element's role",
    wcag_level: WcagLevel::A,
    wcag_criterion: "4.1.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html",
    default_severity: Severity::Error,
};

static GLOBAL_ARIA_ATTRS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "aria-atomic",
        "aria-busy",
        "aria-controls",
        "aria-current",
        "aria-describedby",
        "aria-description",
        "aria-details",
        "aria-disabled",
        "aria-dropeffect",
        "aria-errormessage",
        "aria-flowto",
        "aria-grabbed",
        "aria-haspopup",
        "aria-hidden",
        "aria-invalid",
        "aria-keyshortcuts",
        "aria-label",
        "aria-labelledby",
        "aria-live",
        "aria-owns",
        "aria-relevant",
        "aria-roledescription",
        "aria-braillelabel",
        "aria-brailleroledescription",
    ])
});

static ALLOWED_ATTRS_BY_ROLE: LazyLock<HashMap<&'static str, &'static [&'static str]>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();
        map.insert("alert", &[] as &[&str]);
        map.insert("alertdialog", &["aria-modal"] as &[&str]);
        map.insert("button", &["aria-expanded", "aria-pressed"] as &[&str]);
        map.insert(
            "checkbox",
            &["aria-checked", "aria-readonly", "aria-required"] as &[&str],
        );
        map.insert(
            "combobox",
            &[
                "aria-activedescendant",
                "aria-autocomplete",
                "aria-expanded",
                "aria-required",
            ] as &[&str],
        );
        map.insert("dialog", &["aria-modal"] as &[&str]);
        map.insert(
            "grid",
            &[
                "aria-activedescendant",
                "aria-colcount",
                "aria-multiselectable",
                "aria-readonly",
                "aria-rowcount",
            ] as &[&str],
        );
        map.insert(
            "gridcell",
            &[
                "aria-colindex",
                "aria-colspan",
                "aria-expanded",
                "aria-readonly",
                "aria-required",
                "aria-rowindex",
                "aria-rowspan",
                "aria-selected",
            ] as &[&str],
        );
        map.insert("heading", &["aria-level"] as &[&str]);
        map.insert("img", &[] as &[&str]);
        map.insert("link", &["aria-expanded"] as &[&str]);
        map.insert("list", &[] as &[&str]);
        map.insert(
            "listbox",
            &[
                "aria-activedescendant",
                "aria-expanded",
                "aria-multiselectable",
                "aria-orientation",
                "aria-required",
            ] as &[&str],
        );
        map.insert(
            "listitem",
            &["aria-level", "aria-posinset", "aria-setsize"] as &[&str],
        );
        map.insert("log", &[] as &[&str]);
        map.insert(
            "menu",
            &["aria-activedescendant", "aria-orientation"] as &[&str],
        );
        map.insert(
            "menubar",
            &["aria-activedescendant", "aria-orientation"] as &[&str],
        );
        map.insert("menuitem", &["aria-posinset", "aria-setsize"] as &[&str]);
        map.insert(
            "menuitemcheckbox",
            &["aria-checked", "aria-posinset", "aria-setsize"] as &[&str],
        );
        map.insert(
            "menuitemradio",
            &["aria-checked", "aria-posinset", "aria-setsize"] as &[&str],
        );
        map.insert(
            "meter",
            &[
                "aria-valuemax",
                "aria-valuemin",
                "aria-valuenow",
                "aria-valuetext",
            ] as &[&str],
        );
        map.insert("navigation", &[] as &[&str]);
        map.insert(
            "option",
            &[
                "aria-checked",
                "aria-posinset",
                "aria-selected",
                "aria-setsize",
            ] as &[&str],
        );
        map.insert(
            "progressbar",
            &[
                "aria-valuemax",
                "aria-valuemin",
                "aria-valuenow",
                "aria-valuetext",
            ] as &[&str],
        );
        map.insert(
            "radio",
            &["aria-checked", "aria-posinset", "aria-setsize"] as &[&str],
        );
        map.insert(
            "radiogroup",
            &["aria-orientation", "aria-readonly", "aria-required"] as &[&str],
        );
        map.insert(
            "row",
            &[
                "aria-colindex",
                "aria-expanded",
                "aria-level",
                "aria-posinset",
                "aria-rowindex",
                "aria-selected",
                "aria-setsize",
            ] as &[&str],
        );
        map.insert(
            "rowheader",
            &[
                "aria-colindex",
                "aria-colspan",
                "aria-expanded",
                "aria-readonly",
                "aria-required",
                "aria-rowindex",
                "aria-rowspan",
                "aria-selected",
                "aria-sort",
            ] as &[&str],
        );
        map.insert(
            "scrollbar",
            &[
                "aria-controls",
                "aria-orientation",
                "aria-valuemax",
                "aria-valuemin",
                "aria-valuenow",
                "aria-valuetext",
            ] as &[&str],
        );
        map.insert(
            "searchbox",
            &[
                "aria-activedescendant",
                "aria-autocomplete",
                "aria-multiline",
                "aria-placeholder",
                "aria-readonly",
                "aria-required",
            ] as &[&str],
        );
        map.insert(
            "separator",
            &[
                "aria-orientation",
                "aria-valuemax",
                "aria-valuemin",
                "aria-valuenow",
                "aria-valuetext",
            ] as &[&str],
        );
        map.insert(
            "slider",
            &[
                "aria-orientation",
                "aria-readonly",
                "aria-valuemax",
                "aria-valuemin",
                "aria-valuenow",
                "aria-valuetext",
            ] as &[&str],
        );
        map.insert(
            "spinbutton",
            &[
                "aria-readonly",
                "aria-required",
                "aria-valuemax",
                "aria-valuemin",
                "aria-valuenow",
                "aria-valuetext",
            ] as &[&str],
        );
        map.insert("status", &[] as &[&str]);
        map.insert("switch", &["aria-checked", "aria-readonly"] as &[&str]);
        map.insert(
            "tab",
            &[
                "aria-expanded",
                "aria-posinset",
                "aria-selected",
                "aria-setsize",
            ] as &[&str],
        );
        map.insert("table", &["aria-colcount", "aria-rowcount"] as &[&str]);
        map.insert(
            "tablist",
            &[
                "aria-activedescendant",
                "aria-multiselectable",
                "aria-orientation",
            ] as &[&str],
        );
        map.insert("tabpanel", &[] as &[&str]);
        map.insert(
            "textbox",
            &[
                "aria-activedescendant",
                "aria-autocomplete",
                "aria-multiline",
                "aria-placeholder",
                "aria-readonly",
                "aria-required",
            ] as &[&str],
        );
        map.insert(
            "toolbar",
            &["aria-activedescendant", "aria-orientation"] as &[&str],
        );
        map.insert("tooltip", &[] as &[&str]);
        map.insert(
            "tree",
            &[
                "aria-activedescendant",
                "aria-multiselectable",
                "aria-orientation",
                "aria-required",
            ] as &[&str],
        );
        map.insert(
            "treegrid",
            &[
                "aria-activedescendant",
                "aria-colcount",
                "aria-multiselectable",
                "aria-orientation",
                "aria-readonly",
                "aria-required",
                "aria-rowcount",
            ] as &[&str],
        );
        map.insert(
            "treeitem",
            &[
                "aria-checked",
                "aria-expanded",
                "aria-level",
                "aria-posinset",
                "aria-selected",
                "aria-setsize",
            ] as &[&str],
        );
        map
    });

impl Rule for AriaAllowedAttr {
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
        check_allowed_attrs(&role, &aria_attrs, element_node, diagnostics);
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
        check_allowed_attrs(&role, &aria_attrs, node, diagnostics);
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
                check_allowed_attrs(&role, &aria_attrs, node, diagnostics);
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

fn check_allowed_attrs(
    role: &str,
    aria_attrs: &[String],
    node: &Node,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(role_specific) = ALLOWED_ATTRS_BY_ROLE.get(role) {
        for attr in aria_attrs {
            let attr_lower = attr.to_lowercase();
            if GLOBAL_ARIA_ATTRS.contains(attr_lower.as_str()) {
                continue;
            }
            if role_specific
                .iter()
                .any(|a| a.eq_ignore_ascii_case(&attr_lower))
            {
                continue;
            }
            diagnostics.push(make_diagnostic(node, role, &attr_lower));
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
            "Attribute '{}' is not allowed on role '{}'. {} [WCAG {} Level {:?}]",
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
        let rule = AriaAllowedAttr;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AriaAllowedAttr;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_checkbox_with_aria_checked_passes() {
        let diags = check_html(r#"<div role="checkbox" aria-checked="true"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_checkbox_with_aria_expanded_fails() {
        let diags = check_html(r#"<div role="checkbox" aria-expanded="true"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-allowed-attr".to_string()))
        );
    }

    #[test]
    fn test_button_with_aria_pressed_passes() {
        let diags = check_html(r#"<div role="button" aria-pressed="true"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_button_with_global_aria_label_passes() {
        let diags = check_html(r#"<div role="button" aria-label="x"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_alert_with_aria_selected_fails() {
        let diags = check_html(r#"<div role="alert" aria-selected="true"></div>"#);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("aria-selected"));
        assert!(diags[0].message.contains("alert"));
    }

    #[test]
    fn test_no_role_skips_check() {
        let diags = check_html(r#"<div aria-expanded="true"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_checkbox_with_aria_checked_passes() {
        let diags = check_tsx(r#"const App = () => <div role="checkbox" aria-checked="true" />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_checkbox_with_aria_expanded_fails() {
        let diags = check_tsx(r#"const App = () => <div role="checkbox" aria-expanded="true" />;"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-allowed-attr".to_string()))
        );
    }

    #[test]
    fn test_alert_with_global_attrs_passes() {
        let diags =
            check_html(r#"<div role="alert" aria-live="assertive" aria-busy="true"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_unknown_role_skips_check() {
        let diags =
            check_html(r#"<div role="unknownrole" aria-expanded="true" aria-level="3"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_multiple_disallowed_attrs() {
        let diags =
            check_html(r#"<div role="alert" aria-selected="true" aria-checked="false"></div>"#);
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn test_tsx_element_with_disallowed_attr() {
        let diags =
            check_tsx(r#"const App = () => <div role="alert" aria-selected="true">content</div>;"#);
        assert_eq!(diags.len(), 1);
    }
}
