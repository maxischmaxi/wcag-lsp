use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct HeadingContent;

static METADATA: RuleMetadata = RuleMetadata {
    id: "heading-content",
    description: "Heading elements must have text content",
    wcag_level: WcagLevel::AA,
    wcag_criterion: "2.4.6",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/headings-and-labels.html",
    default_severity: Severity::Warning,
};

impl Rule for HeadingContent {
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

/// Check whether a tag name is a heading (h1..h6).
fn is_heading_tag(tag_name: &str) -> bool {
    matches!(
        tag_name.to_ascii_lowercase().as_str(),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
    )
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
    let mut is_heading = false;
    let mut has_aria_label = false;

    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" {
            let mut tag_cursor = child.walk();
            for tag_child in child.children(&mut tag_cursor) {
                if tag_child.kind() == "tag_name" {
                    let name = &source[tag_child.byte_range()];
                    if is_heading_tag(name) {
                        is_heading = true;
                    }
                }
                if tag_child.kind() == "attribute" {
                    let attr_name = extract_html_attr_name(&tag_child, source);
                    if let Some(name) = attr_name
                        && (name.eq_ignore_ascii_case("aria-label")
                            || name.eq_ignore_ascii_case("aria-labelledby"))
                    {
                        has_aria_label = true;
                    }
                }
            }
        }
    }

    if !is_heading {
        return;
    }

    if has_aria_label {
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
    let mut is_heading = false;
    let mut has_aria_label = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = &source[child.byte_range()];
            if is_heading_tag(name) {
                is_heading = true;
            }
        }
        if child.kind() == "jsx_attribute" {
            let attr_name = extract_jsx_attr_name(&child, source);
            if let Some(name) = attr_name
                && (name == "aria-label"
                    || name == "aria-labelledby"
                    || name == "ariaLabel"
                    || name == "ariaLabelledby")
            {
                has_aria_label = true;
            }
        }
    }

    if is_heading && !has_aria_label {
        // Self-closing heading has no children, so it always fails
        diagnostics.push(make_diagnostic(node));
    }
}

fn check_jsx_element(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut is_heading = false;
    let mut has_aria_label = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "identifier" {
                    let name = &source[inner_child.byte_range()];
                    if is_heading_tag(name) {
                        is_heading = true;
                    }
                }
                if inner_child.kind() == "jsx_attribute" {
                    let attr_name = extract_jsx_attr_name(&inner_child, source);
                    if let Some(name) = attr_name
                        && (name == "aria-label"
                            || name == "aria-labelledby"
                            || name == "ariaLabel"
                            || name == "ariaLabelledby")
                    {
                        has_aria_label = true;
                    }
                }
            }
        }
    }

    if !is_heading {
        return;
    }

    if has_aria_label {
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
        let rule = HeadingContent;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = HeadingContent;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_h1_with_text_passes() {
        let diags = check_html(r#"<h1>Welcome</h1>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_h1_empty_fails() {
        let diags = check_html(r#"<h1></h1>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("heading-content".to_string()))
        );
    }

    #[test]
    fn test_h2_with_aria_label_passes() {
        let diags = check_html(r#"<h2 aria-label="Section"></h2>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_h2_with_aria_labelledby_passes() {
        let diags = check_html(r#"<h2 aria-labelledby="heading-ref"></h2>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_h3_with_child_element_passes() {
        let diags = check_html(r#"<h3><span>Title</span></h3>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_h4_whitespace_only_fails() {
        let diags = check_html(r#"<h4>   </h4>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_no_heading_passes() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_h1_with_text_passes() {
        let diags = check_tsx(r#"const App = () => <h1>Welcome</h1>;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_h1_empty_fails() {
        let diags = check_tsx(r#"const App = () => <h1></h1>;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_h2_with_aria_label_passes() {
        let diags = check_tsx(r#"const App = () => <h2 aria-label="Section"></h2>;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_h3_with_child_element_passes() {
        let diags = check_tsx(r#"const App = () => <h3><span>Title</span></h3>;"#);
        assert_eq!(diags.len(), 0);
    }
}
