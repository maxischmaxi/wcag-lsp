use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct InputImageAlt;

static METADATA: RuleMetadata = RuleMetadata {
    id: "input-image-alt",
    description: "<input type=\"image\"> elements must have an alt attribute",
    wcag_level: WcagLevel::A,
    wcag_criterion: "1.1.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/non-text-content.html",
    default_severity: Severity::Error,
};

impl Rule for InputImageAlt {
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
                check_html_start_tag(&child, source, diagnostics, node);
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diagnostics);
    }
}

fn check_html_start_tag(
    start_tag: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    element_node: &Node,
) {
    let mut is_input = false;
    let mut is_type_image = false;
    let mut has_alt = false;

    let mut cursor = start_tag.walk();
    for child in start_tag.children(&mut cursor) {
        if child.kind() == "tag_name" {
            let name = &source[child.byte_range()];
            if name.eq_ignore_ascii_case("input") {
                is_input = true;
            }
        }
        if child.kind() == "attribute" {
            let (attr_name, attr_value) = extract_html_attribute(&child, source);
            if let Some(ref name) = attr_name {
                if name.eq_ignore_ascii_case("type")
                    && let Some(ref val) = attr_value
                    && val.eq_ignore_ascii_case("image")
                {
                    is_type_image = true;
                }
                if name.eq_ignore_ascii_case("alt") {
                    has_alt = true;
                }
            }
        }
    }

    if is_input && is_type_image && !has_alt {
        diagnostics.push(make_diagnostic(element_node));
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
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_jsx(&child, source, diagnostics);
    }
}

fn check_jsx_self_closing(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut is_input = false;
    let mut is_type_image = false;
    let mut has_alt = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = &source[child.byte_range()];
            if name == "input" {
                is_input = true;
            }
        }
        if child.kind() == "jsx_attribute" {
            let (attr_name, attr_value) = extract_jsx_attribute(&child, source);
            if let Some(ref name) = attr_name {
                if name == "type"
                    && let Some(ref val) = attr_value
                    && val == "image"
                {
                    is_type_image = true;
                }
                if name == "alt" {
                    has_alt = true;
                }
            }
        }
    }

    if is_input && is_type_image && !has_alt {
        diagnostics.push(make_diagnostic(node));
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
        let rule = InputImageAlt;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = InputImageAlt;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_input_image_without_alt_fails() {
        let diags = check_html(r#"<input type="image" src="submit.png">"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("input-image-alt".to_string()))
        );
    }

    #[test]
    fn test_input_image_with_alt_passes() {
        let diags = check_html(r#"<input type="image" src="submit.png" alt="Submit">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_input_text_no_diagnostic() {
        let diags = check_html(r#"<input type="text" name="username">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_input_no_diagnostic() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_input_image_without_alt_fails() {
        let diags = check_tsx(r#"const App = () => <input type="image" src="submit.png" />;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_input_image_with_alt_passes() {
        let diags =
            check_tsx(r#"const App = () => <input type="image" src="submit.png" alt="Submit" />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_input_text_no_diagnostic() {
        let diags = check_tsx(r#"const App = () => <input type="text" name="username" />;"#);
        assert_eq!(diags.len(), 0);
    }
}
