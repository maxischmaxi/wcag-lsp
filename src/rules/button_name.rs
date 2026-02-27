use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct ButtonName;

static METADATA: RuleMetadata = RuleMetadata {
    id: "button-name",
    description: "<button> elements must have an accessible name",
    wcag_level: WcagLevel::A,
    wcag_criterion: "4.1.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html",
    default_severity: Severity::Error,
};

impl Rule for ButtonName {
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
    let mut is_button = false;
    let mut has_accessible_name = false;

    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" {
            let mut tag_cursor = child.walk();
            for tag_child in child.children(&mut tag_cursor) {
                if tag_child.kind() == "tag_name" {
                    let name = &source[tag_child.byte_range()];
                    if name.eq_ignore_ascii_case("button") {
                        is_button = true;
                    }
                }
                if tag_child.kind() == "attribute" {
                    let attr_name = extract_html_attr_name(&tag_child, source);
                    if let Some(name) = attr_name
                        && (name.eq_ignore_ascii_case("aria-label")
                            || name.eq_ignore_ascii_case("aria-labelledby")
                            || name.eq_ignore_ascii_case("title"))
                    {
                        has_accessible_name = true;
                    }
                }
            }
        }
    }

    if !is_button {
        return;
    }

    if has_accessible_name {
        return;
    }

    if has_content(element, source) {
        return;
    }

    diagnostics.push(make_diagnostic(element));
}

/// Check whether an HTML element has any meaningful content: non-whitespace text
/// or child elements (which may themselves provide text).
fn has_content(element: &Node, source: &str) -> bool {
    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        match child.kind() {
            "text" => {
                let text = &source[child.byte_range()];
                if !text.trim().is_empty() {
                    return true;
                }
            }
            "element" => {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn extract_html_attr_name(attr_node: &Node, source: &str) -> Option<String> {
    let mut cursor = attr_node.walk();
    for child in attr_node.children(&mut cursor) {
        if child.kind() == "attribute_name" {
            return Some(source[child.byte_range()].to_string());
        }
    }
    None
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
    let mut is_button = false;
    let mut has_accessible_name = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = &source[child.byte_range()];
            if name == "button" {
                is_button = true;
            }
        }
        if child.kind() == "jsx_attribute" {
            let attr_name = extract_jsx_attr_name(&child, source);
            if let Some(name) = attr_name
                && (name == "aria-label"
                    || name == "aria-labelledby"
                    || name == "ariaLabel"
                    || name == "ariaLabelledby"
                    || name == "title")
            {
                has_accessible_name = true;
            }
        }
    }

    if is_button && !has_accessible_name {
        // Self-closing <button /> has no children, so it always fails
        diagnostics.push(make_diagnostic(node));
    }
}

fn check_jsx_element(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut is_button = false;
    let mut has_accessible_name = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "identifier" {
                    let name = &source[inner_child.byte_range()];
                    if name == "button" {
                        is_button = true;
                    }
                }
                if inner_child.kind() == "jsx_attribute" {
                    let attr_name = extract_jsx_attr_name(&inner_child, source);
                    if let Some(name) = attr_name
                        && (name == "aria-label"
                            || name == "aria-labelledby"
                            || name == "ariaLabel"
                            || name == "ariaLabelledby"
                            || name == "title")
                    {
                        has_accessible_name = true;
                    }
                }
            }
        }
    }

    if !is_button {
        return;
    }

    if has_accessible_name {
        return;
    }

    if has_jsx_content(node, source) {
        return;
    }

    diagnostics.push(make_diagnostic(node));
}

/// Check whether a JSX element has any meaningful child content.
fn has_jsx_content(node: &Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "jsx_text" => {
                let text = &source[child.byte_range()];
                if !text.trim().is_empty() {
                    return true;
                }
            }
            "jsx_element" | "jsx_self_closing_element" | "jsx_expression" => {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn extract_jsx_attr_name(attr_node: &Node, source: &str) -> Option<String> {
    let mut cursor = attr_node.walk();
    for child in attr_node.children(&mut cursor) {
        if child.kind() == "property_identifier" {
            return Some(source[child.byte_range()].to_string());
        }
    }
    None
}

fn make_diagnostic(node: &Node) -> Diagnostic {
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
        let rule = ButtonName;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = ButtonName;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_button_with_text_passes() {
        let diags = check_html(r#"<button>Submit</button>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_empty_button_fails() {
        let diags = check_html(r#"<button></button>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("button-name".to_string()))
        );
    }

    #[test]
    fn test_button_with_aria_label_passes() {
        let diags = check_html(r#"<button aria-label="Close"></button>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_button_with_aria_labelledby_passes() {
        let diags = check_html(r#"<button aria-labelledby="btn-label"></button>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_button_with_title_passes() {
        let diags = check_html(r#"<button title="Close"></button>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_button_with_child_element_passes() {
        let diags = check_html(r#"<button><img alt="icon"></button>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_button_whitespace_only_fails() {
        let diags = check_html(r#"<button>   </button>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_no_button_passes() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_button_with_text_passes() {
        let diags = check_tsx(r#"const App = () => <button>Submit</button>;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_empty_button_fails() {
        let diags = check_tsx(r#"const App = () => <button></button>;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_button_with_aria_label_passes() {
        let diags = check_tsx(r#"const App = () => <button aria-label="Close" />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_button_with_child_element_passes() {
        let diags = check_tsx(r#"const App = () => <button><img alt="icon" /></button>;"#);
        assert_eq!(diags.len(), 0);
    }
}
