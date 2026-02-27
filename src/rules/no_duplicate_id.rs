use std::collections::HashMap;

use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct NoDuplicateId;

static METADATA: RuleMetadata = RuleMetadata {
    id: "no-duplicate-id",
    description: "id attribute values must be unique",
    wcag_level: WcagLevel::A,
    wcag_criterion: "4.1.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/parsing.html",
    default_severity: Severity::Error,
};

impl Rule for NoDuplicateId {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(&self, root: &Node, source: &str, file_type: FileType) -> Vec<Diagnostic> {
        let mut id_entries: Vec<(String, Node)> = Vec::new();
        if file_type.is_jsx_like() {
            collect_ids_jsx(root, source, &mut id_entries);
        } else {
            collect_ids_html(root, source, &mut id_entries);
        }

        let mut diagnostics = Vec::new();
        let mut seen: HashMap<String, bool> = HashMap::new();

        for (id_value, node) in &id_entries {
            if let Some(_first_seen) = seen.get(id_value) {
                // This is a duplicate; report on this (second or subsequent) occurrence
                diagnostics.push(make_diagnostic(node, id_value));
            } else {
                seen.insert(id_value.clone(), true);
            }
        }

        diagnostics
    }
}

// ---------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------

fn collect_ids_html<'a>(node: &Node<'a>, source: &str, entries: &mut Vec<(String, Node<'a>)>) {
    if node.kind() == "element" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "start_tag"
                && let Some(id_value) = extract_html_id(&child, source)
            {
                entries.push((id_value, *node));
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_ids_html(&child, source, entries);
    }
}

/// Extract the value of an `id` attribute from an HTML start_tag, if present.
fn extract_html_id(start_tag: &Node, source: &str) -> Option<String> {
    let mut cursor = start_tag.walk();
    for child in start_tag.children(&mut cursor) {
        if child.kind() == "attribute" {
            let (attr_name, attr_value) = extract_html_attribute(&child, source);
            if let Some(name) = attr_name
                && name.eq_ignore_ascii_case("id")
                && let Some(val) = attr_value
                && !val.trim().is_empty()
            {
                return Some(val);
            }
        }
    }
    None
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
            // If we found a quoted_attribute_value but no inner attribute_value,
            // it's an empty string like id=""
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

fn collect_ids_jsx<'a>(node: &Node<'a>, source: &str, entries: &mut Vec<(String, Node<'a>)>) {
    match node.kind() {
        "jsx_self_closing_element" => {
            if let Some(id_value) = extract_jsx_id(node, source) {
                entries.push((id_value, *node));
            }
        }
        "jsx_element" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "jsx_opening_element"
                    && let Some(id_value) = extract_jsx_id(&child, source)
                {
                    entries.push((id_value, *node));
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_ids_jsx(&child, source, entries);
    }
}

/// Extract the value of an `id` attribute from a JSX element or opening element node.
fn extract_jsx_id(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_attribute" {
            let (attr_name, attr_value) = extract_jsx_attribute(&child, source);
            if let Some(name) = attr_name
                && name == "id"
                && let Some(val) = attr_value
                && !val.trim().is_empty()
            {
                return Some(val);
            }
        }
    }
    None
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

fn make_diagnostic(node: &Node, id_value: &str) -> Diagnostic {
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
            "Duplicate id attribute value \"{}\" - {} [WCAG {} Level {:?}]",
            id_value, meta.description, meta.wcag_criterion, meta.wcag_level
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
        let rule = NoDuplicateId;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = NoDuplicateId;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_unique_ids_passes() {
        let diags = check_html(r#"<div id="a"></div><div id="b"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_duplicate_ids_fails() {
        let diags = check_html(r#"<div id="a"></div><div id="a"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("no-duplicate-id".to_string()))
        );
    }

    #[test]
    fn test_triple_duplicate_ids_reports_two() {
        let diags = check_html(r#"<div id="x"></div><div id="x"></div><div id="x"></div>"#);
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn test_no_ids_passes() {
        let diags = check_html(r#"<div></div><span></span>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_empty_id_ignored() {
        let diags = check_html(r#"<div id=""></div><div id=""></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_unique_ids_passes() {
        let diags = check_tsx(r#"const App = () => <><div id="a" /><div id="b" /></>;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_duplicate_ids_fails() {
        let diags = check_tsx(r#"const App = () => <><div id="a" /><div id="a" /></>;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_no_ids_passes() {
        let diags = check_tsx(r#"const App = () => <div />;"#);
        assert_eq!(diags.len(), 0);
    }
}
