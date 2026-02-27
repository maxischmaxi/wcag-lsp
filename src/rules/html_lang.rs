use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct HtmlLang;

static METADATA: RuleMetadata = RuleMetadata {
    id: "html-lang",
    description: "<html> element must have a lang attribute",
    wcag_level: WcagLevel::A,
    wcag_criterion: "3.1.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/language-of-page.html",
    default_severity: Severity::Error,
};

impl Rule for HtmlLang {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(&self, root: &Node, source: &str, file_type: FileType) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        if file_type.is_jsx_like() {
            return diagnostics;
        }
        visit_html(root, source, &mut diagnostics);
        diagnostics
    }
}

fn visit_html(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "element" {
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
    let mut is_html = false;
    let mut has_lang = false;
    let mut lang_is_empty = false;

    let mut cursor = start_tag.walk();
    for child in start_tag.children(&mut cursor) {
        if child.kind() == "tag_name" {
            let name = &source[child.byte_range()];
            if name.eq_ignore_ascii_case("html") {
                is_html = true;
            }
        }
        if child.kind() == "attribute" {
            let mut attr_cursor = child.walk();
            let mut is_lang_attr = false;
            let mut has_value = false;
            let mut value_is_empty = true;

            for attr_child in child.children(&mut attr_cursor) {
                if attr_child.kind() == "attribute_name" {
                    let attr_name = &source[attr_child.byte_range()];
                    if attr_name.eq_ignore_ascii_case("lang") {
                        is_lang_attr = true;
                    }
                }
                if attr_child.kind() == "quoted_attribute_value" {
                    has_value = true;
                    // Check if there's an attribute_value child inside the quoted value
                    let mut val_cursor = attr_child.walk();
                    for val_child in attr_child.children(&mut val_cursor) {
                        if val_child.kind() == "attribute_value" {
                            let val = &source[val_child.byte_range()];
                            if !val.trim().is_empty() {
                                value_is_empty = false;
                            }
                        }
                    }
                }
            }

            if is_lang_attr {
                has_lang = true;
                // lang="" (has_value true but value_is_empty true) means empty lang
                // lang (no value at all) also counts as empty
                if !has_value || value_is_empty {
                    lang_is_empty = true;
                }
            }
        }
    }

    if is_html && (!has_lang || lang_is_empty) {
        diagnostics.push(make_diagnostic(element_node));
    }
}

fn make_diagnostic(node: &Node) -> Diagnostic {
    let meta = &METADATA;
    Diagnostic {
        range: node_to_range(node),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String("html-lang".to_string())),
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
        let rule = HtmlLang;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    #[test]
    fn test_html_without_lang() {
        let diags = check_html("<html><body></body></html>");
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("html-lang".to_string()))
        );
    }

    #[test]
    fn test_html_with_lang() {
        let diags = check_html(r#"<html lang="en"><body></body></html>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_html_with_empty_lang() {
        let diags = check_html(r#"<html lang=""><body></body></html>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_no_html_element() {
        let diags = check_html("<div>Hello</div>");
        assert_eq!(diags.len(), 0);
    }
}
