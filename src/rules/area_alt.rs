use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct AreaAlt;

static METADATA: RuleMetadata = RuleMetadata {
    id: "area-alt",
    description: "<area> elements must have an alt, aria-label, or aria-labelledby attribute",
    wcag_level: WcagLevel::A,
    wcag_criterion: "1.1.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/non-text-content.html",
    default_severity: Severity::Error,
};

impl Rule for AreaAlt {
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
    let mut is_area = false;
    let mut has_label = false;

    let mut cursor = start_tag.walk();
    for child in start_tag.children(&mut cursor) {
        if child.kind() == "tag_name" {
            let name = &source[child.byte_range()];
            if name.eq_ignore_ascii_case("area") {
                is_area = true;
            }
        }
        if child.kind() == "attribute" {
            let attr_name = extract_html_attr_name(&child, source);
            if let Some(name) = attr_name
                && (name.eq_ignore_ascii_case("alt")
                    || name.eq_ignore_ascii_case("aria-label")
                    || name.eq_ignore_ascii_case("aria-labelledby"))
            {
                has_label = true;
            }
        }
    }

    if is_area && !has_label {
        diagnostics.push(make_diagnostic(element_node));
    }
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
    let mut is_area = false;
    let mut has_label = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = &source[child.byte_range()];
            if name == "area" {
                is_area = true;
            }
        }
        if child.kind() == "jsx_attribute" {
            let attr_name = extract_jsx_attr_name(&child, source);
            if let Some(name) = attr_name
                && (name == "alt"
                    || name == "aria-label"
                    || name == "aria-labelledby"
                    || name == "ariaLabel"
                    || name == "ariaLabelledby")
            {
                has_label = true;
            }
        }
    }

    if is_area && !has_label {
        diagnostics.push(make_diagnostic(node));
    }
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
        let rule = AreaAlt;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AreaAlt;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_area_without_alt_fails() {
        let diags = check_html(r#"<area href="/link">"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("area-alt".to_string()))
        );
    }

    #[test]
    fn test_area_with_alt_passes() {
        let diags = check_html(r#"<area href="/link" alt="Description">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_area_with_aria_label_passes() {
        let diags = check_html(r#"<area href="/link" aria-label="Description">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_area_with_aria_labelledby_passes() {
        let diags = check_html(r#"<area href="/link" aria-labelledby="desc">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_area_no_diagnostic() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_area_without_alt_fails() {
        let diags = check_tsx(r#"const App = () => <area href="/link" />;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_area_with_alt_passes() {
        let diags = check_tsx(r#"const App = () => <area href="/link" alt="Description" />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_area_with_aria_label_passes() {
        let diags =
            check_tsx(r#"const App = () => <area href="/link" aria-label="Description" />;"#);
        assert_eq!(diags.len(), 0);
    }
}
