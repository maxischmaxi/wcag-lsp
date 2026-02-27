use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct PageTitle;

static METADATA: RuleMetadata = RuleMetadata {
    id: "page-title",
    description: "Document must have a <title> element with content",
    wcag_level: WcagLevel::A,
    wcag_criterion: "2.4.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/page-titled.html",
    default_severity: Severity::Error,
};

impl Rule for PageTitle {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(&self, root: &Node, source: &str, file_type: FileType) -> Vec<Diagnostic> {
        // This rule is HTML-only; JSX components don't define document titles this way.
        if file_type.is_jsx_like() {
            return Vec::new();
        }

        let mut diagnostics = Vec::new();
        check_document(root, source, &mut diagnostics);
        diagnostics
    }
}

/// Walk the entire document looking for a <title> element with non-empty text.
/// If no such element is found, report a diagnostic on the root node.
fn check_document(root: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if find_title_with_content(root, source) {
        return;
    }

    diagnostics.push(make_diagnostic(root));
}

/// Recursively search for a <title> element that has non-empty text content.
fn find_title_with_content(node: &Node, source: &str) -> bool {
    if node.kind() == "element" && is_title_element_with_content(node, source) {
        return true;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if find_title_with_content(&child, source) {
            return true;
        }
    }

    false
}

/// Check whether a given element node is a <title> with non-empty text content.
fn is_title_element_with_content(element: &Node, source: &str) -> bool {
    let mut is_title = false;

    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" {
            let mut tag_cursor = child.walk();
            for tag_child in child.children(&mut tag_cursor) {
                if tag_child.kind() == "tag_name" {
                    let name = &source[tag_child.byte_range()];
                    if name.eq_ignore_ascii_case("title") {
                        is_title = true;
                    }
                }
            }
        }
    }

    if !is_title {
        return false;
    }

    // Check for non-empty text content inside the element
    let mut content_cursor = element.walk();
    for child in element.children(&mut content_cursor) {
        if child.kind() == "text" {
            let text = &source[child.byte_range()];
            if !text.trim().is_empty() {
                return true;
            }
        }
    }

    false
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
        let rule = PageTitle;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    #[test]
    fn test_html_with_title_passes() {
        let diags = check_html(r#"<html><head><title>My Page</title></head><body></body></html>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_html_without_title_fails() {
        let diags = check_html(r#"<html><head></head><body></body></html>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("page-title".to_string()))
        );
    }

    #[test]
    fn test_html_with_empty_title_fails() {
        let diags = check_html(r#"<html><head><title></title></head><body></body></html>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_html_with_whitespace_title_fails() {
        let diags = check_html(r#"<html><head><title>   </title></head><body></body></html>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_non_html_returns_empty() {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let source = r#"const App = () => <div />;"#;
        let tree = parser.parse(source, None).unwrap();
        let rule = PageTitle;
        let diags = rule.check(&tree.root_node(), source, FileType::Tsx);
        assert_eq!(diags.len(), 0);
    }
}
