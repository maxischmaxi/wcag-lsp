use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use std::collections::HashMap;
use std::sync::LazyLock;
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct AriaValidAttrValue;

static METADATA: RuleMetadata = RuleMetadata {
    id: "aria-valid-attr-value",
    description: "ARIA attribute values must be valid",
    wcag_level: WcagLevel::A,
    wcag_criterion: "4.1.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html",
    default_severity: Severity::Error,
};

#[derive(Debug, Clone)]
enum AttrValueType {
    Boolean,
    Tristate,
    Token(&'static [&'static str]),
    Integer,
    Number,
}

static ARIA_ATTR_TYPES: LazyLock<HashMap<&'static str, AttrValueType>> = LazyLock::new(|| {
    let mut map = HashMap::new();

    // Boolean attributes
    for attr in &[
        "aria-atomic",
        "aria-busy",
        "aria-disabled",
        "aria-grabbed",
        "aria-hidden",
        "aria-modal",
        "aria-multiline",
        "aria-multiselectable",
        "aria-readonly",
        "aria-required",
    ] {
        map.insert(*attr, AttrValueType::Boolean);
    }

    // Tristate attributes
    map.insert("aria-checked", AttrValueType::Tristate);
    map.insert("aria-pressed", AttrValueType::Tristate);

    // Token attributes
    map.insert(
        "aria-autocomplete",
        AttrValueType::Token(&["inline", "list", "both", "none"]),
    );
    map.insert(
        "aria-current",
        AttrValueType::Token(&["page", "step", "location", "date", "time", "true", "false"]),
    );
    map.insert(
        "aria-dropeffect",
        AttrValueType::Token(&["copy", "execute", "link", "move", "none", "popup"]),
    );
    map.insert(
        "aria-haspopup",
        AttrValueType::Token(&["true", "false", "menu", "listbox", "tree", "grid", "dialog"]),
    );
    map.insert(
        "aria-invalid",
        AttrValueType::Token(&["grammar", "false", "spelling", "true"]),
    );
    map.insert(
        "aria-live",
        AttrValueType::Token(&["assertive", "off", "polite"]),
    );
    map.insert(
        "aria-orientation",
        AttrValueType::Token(&["horizontal", "vertical", "undefined"]),
    );
    map.insert(
        "aria-relevant",
        AttrValueType::Token(&["additions", "removals", "text", "all"]),
    );
    map.insert(
        "aria-sort",
        AttrValueType::Token(&["ascending", "descending", "none", "other"]),
    );
    map.insert(
        "aria-expanded",
        AttrValueType::Token(&["true", "false", "undefined"]),
    );
    map.insert(
        "aria-selected",
        AttrValueType::Token(&["true", "false", "undefined"]),
    );

    // Integer attributes
    for attr in &[
        "aria-colcount",
        "aria-colindex",
        "aria-colspan",
        "aria-level",
        "aria-posinset",
        "aria-rowcount",
        "aria-rowindex",
        "aria-rowspan",
        "aria-setsize",
    ] {
        map.insert(*attr, AttrValueType::Integer);
    }

    // Number attributes
    for attr in &["aria-valuemax", "aria-valuemin", "aria-valuenow"] {
        map.insert(*attr, AttrValueType::Number);
    }

    map
});

impl Rule for AriaValidAttrValue {
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
// Validation
// ---------------------------------------------------------------------------

fn validate_value(attr_name: &str, value: &str, attr_type: &AttrValueType) -> bool {
    match attr_type {
        AttrValueType::Boolean => value == "true" || value == "false",
        AttrValueType::Tristate => value == "true" || value == "false" || value == "mixed",
        AttrValueType::Token(allowed) => {
            if attr_name == "aria-relevant" {
                // aria-relevant allows space-separated tokens
                value
                    .split_whitespace()
                    .all(|token| allowed.contains(&token))
                    && !value.trim().is_empty()
            } else {
                allowed.contains(&value)
            }
        }
        AttrValueType::Integer => value.parse::<i64>().is_ok(),
        AttrValueType::Number => value.parse::<f64>().is_ok(),
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

fn check_html_attribute(attr_node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let (name, value) = extract_html_attribute(attr_node, source);

    let attr_name = match name {
        Some(n) => n,
        None => return,
    };

    let lower_name = attr_name.to_lowercase();
    if !lower_name.starts_with("aria-") {
        return;
    }

    let attr_type = match ARIA_ATTR_TYPES.get(lower_name.as_str()) {
        Some(t) => t,
        None => return, // Unknown aria attribute or free-form (like aria-label)
    };

    let attr_value = match value {
        Some(v) => v,
        None => return, // No value specified
    };

    if !validate_value(&lower_name, &attr_value, attr_type) {
        diagnostics.push(make_diagnostic(
            attr_node,
            &lower_name,
            &attr_value,
            attr_type,
        ));
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
        }
    }

    (name, value)
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

fn check_jsx_attribute(attr_node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let (name, value) = extract_jsx_attribute(attr_node, source);

    let attr_name = match name {
        Some(n) => n,
        None => return,
    };

    if !attr_name.starts_with("aria-") {
        return;
    }

    let attr_type = match ARIA_ATTR_TYPES.get(attr_name.as_str()) {
        Some(t) => t,
        None => return, // Unknown aria attribute or free-form
    };

    let attr_value = match value {
        Some(v) => v,
        None => return, // No string value (could be JSX expression, skip)
    };

    if !validate_value(&attr_name, &attr_value, attr_type) {
        diagnostics.push(make_diagnostic(
            attr_node,
            &attr_name,
            &attr_value,
            attr_type,
        ));
    }
}

fn extract_jsx_attribute(attr_node: &Node, source: &str) -> (Option<String>, Option<String>) {
    let mut name = None;
    let mut value = None;
    let mut has_expression = false;

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
        if child.kind() == "jsx_expression" {
            has_expression = true;
        }
    }

    // If the value is a JSX expression, skip validation by returning None for value
    if has_expression {
        value = None;
    }

    (name, value)
}

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

fn make_diagnostic(
    node: &Node,
    attr_name: &str,
    attr_value: &str,
    attr_type: &AttrValueType,
) -> Diagnostic {
    let meta = &METADATA;
    let expected = match attr_type {
        AttrValueType::Boolean => "\"true\" or \"false\"".to_string(),
        AttrValueType::Tristate => "\"true\", \"false\", or \"mixed\"".to_string(),
        AttrValueType::Token(allowed) => {
            format!("one of: {}", allowed.join(", "))
        }
        AttrValueType::Integer => "a valid integer".to_string(),
        AttrValueType::Number => "a valid number".to_string(),
    };

    Diagnostic {
        range: node_to_range(node),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(meta.id.to_string())),
        code_description: Some(CodeDescription {
            href: meta.wcag_url.parse().expect("valid URL"),
        }),
        source: Some("wcag-lsp".to_string()),
        message: format!(
            "Invalid value \"{}\" for attribute '{}'. Expected {}. {} [WCAG {} Level {:?}]",
            attr_value, attr_name, expected, meta.description, meta.wcag_criterion, meta.wcag_level
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
        let rule = AriaValidAttrValue;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AriaValidAttrValue;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_aria_hidden_true_passes() {
        let diags = check_html(r#"<div aria-hidden="true"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_aria_hidden_false_passes() {
        let diags = check_html(r#"<div aria-hidden="false"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_aria_hidden_invalid_fails() {
        let diags = check_html(r#"<div aria-hidden="yes"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-valid-attr-value".to_string()))
        );
    }

    #[test]
    fn test_aria_checked_mixed_passes() {
        let diags = check_html(r#"<div aria-checked="mixed"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_aria_checked_invalid_fails() {
        let diags = check_html(r#"<div aria-checked="maybe"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-valid-attr-value".to_string()))
        );
    }

    #[test]
    fn test_aria_autocomplete_valid_passes() {
        let diags = check_html(r#"<div aria-autocomplete="list"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_aria_autocomplete_invalid_fails() {
        let diags = check_html(r#"<div aria-autocomplete="foo"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-valid-attr-value".to_string()))
        );
    }

    #[test]
    fn test_aria_level_valid_passes() {
        let diags = check_html(r#"<div aria-level="2"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_aria_level_invalid_fails() {
        let diags = check_html(r#"<div aria-level="abc"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-valid-attr-value".to_string()))
        );
    }

    #[test]
    fn test_aria_valuenow_valid_passes() {
        let diags = check_html(r#"<div aria-valuenow="3.5"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_aria_valuenow_invalid_fails() {
        let diags = check_html(r#"<div aria-valuenow="abc"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-valid-attr-value".to_string()))
        );
    }

    #[test]
    fn test_aria_live_valid_passes() {
        let diags = check_html(r#"<div aria-live="polite"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_aria_live_invalid_fails() {
        let diags = check_html(r#"<div aria-live="loud"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-valid-attr-value".to_string()))
        );
    }

    #[test]
    fn test_aria_label_freeform_passes() {
        let diags = check_html(r#"<div aria-label="text"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_aria_hidden_true_passes() {
        let diags = check_tsx(r#"const App = () => <div aria-hidden="true" />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_aria_hidden_invalid_fails() {
        let diags = check_tsx(r#"const App = () => <div aria-hidden="yes" />;"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-valid-attr-value".to_string()))
        );
    }

    #[test]
    fn test_tsx_expression_skipped() {
        let diags = check_tsx(r#"const x = <div aria-hidden={isHidden} />;"#);
        assert_eq!(diags.len(), 0);
    }
}
