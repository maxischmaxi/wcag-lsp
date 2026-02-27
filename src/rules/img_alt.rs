use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct ImgAlt;

static METADATA: RuleMetadata = RuleMetadata {
    id: "img-alt",
    description: "<img> elements must have an alt attribute",
    wcag_level: WcagLevel::A,
    wcag_criterion: "1.1.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/non-text-content.html",
    default_severity: Severity::Error,
};

impl Rule for ImgAlt {
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

fn visit_html(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "element" {
        // Look for a start_tag child
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "start_tag" {
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
    let mut is_img = false;
    let mut has_alt = false;

    let mut cursor = start_tag.walk();
    for child in start_tag.children(&mut cursor) {
        if child.kind() == "tag_name" {
            let name = &source[child.byte_range()];
            if name.eq_ignore_ascii_case("img") {
                is_img = true;
            }
        }
        if child.kind() == "attribute" {
            // The attribute node's first child is attribute_name
            let mut attr_cursor = child.walk();
            for attr_child in child.children(&mut attr_cursor) {
                if attr_child.kind() == "attribute_name" {
                    let attr_name = &source[attr_child.byte_range()];
                    if attr_name.eq_ignore_ascii_case("alt") {
                        has_alt = true;
                    }
                }
            }
        }
    }

    if is_img && !has_alt {
        diagnostics.push(make_diagnostic(element_node));
    }
}

fn visit_jsx(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "jsx_self_closing_element" {
        check_jsx_element(node, source, diagnostics);
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_jsx(&child, source, diagnostics);
    }
}

fn check_jsx_element(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut is_img = false;
    let mut has_alt = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = &source[child.byte_range()];
            if name == "img" {
                is_img = true;
            }
        }
        if child.kind() == "jsx_attribute" {
            // First child is property_identifier (attribute name)
            let mut attr_cursor = child.walk();
            for attr_child in child.children(&mut attr_cursor) {
                if attr_child.kind() == "property_identifier" {
                    let attr_name = &source[attr_child.byte_range()];
                    if attr_name == "alt" {
                        has_alt = true;
                    }
                }
            }
        }
    }

    if is_img && !has_alt {
        diagnostics.push(make_diagnostic(node));
    }
}

fn make_diagnostic(node: &Node) -> Diagnostic {
    let meta = &METADATA;
    Diagnostic {
        range: node_to_range(node),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String("img-alt".to_string())),
        code_description: Some(CodeDescription {
            href: meta
                .wcag_url
                .parse()
                .expect("valid URL"),
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
        let rule = ImgAlt;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = ImgAlt;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_img_without_alt_fails() {
        let diags = check_html(r#"<img src="photo.jpg">"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("img-alt".to_string()))
        );
    }

    #[test]
    fn test_img_with_alt_passes() {
        let diags = check_html(r#"<img src="photo.jpg" alt="A photo">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_img_with_empty_alt_passes() {
        let diags = check_html(r#"<img src="spacer.gif" alt="">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_img_no_diagnostic() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_multiple_imgs_mixed() {
        let diags = check_html(r#"<div><img src="a.jpg" alt="A"><img src="b.jpg"></div>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_img_without_alt_fails() {
        let diags = check_tsx(r#"const App = () => <img src="photo.jpg" />;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_img_with_alt_passes() {
        let diags = check_tsx(r#"const App = () => <img src="photo.jpg" alt="A photo" />;"#);
        assert_eq!(diags.len(), 0);
    }
}
