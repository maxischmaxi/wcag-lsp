use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::html_attrs;
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
        check_html_element(node, source, diagnostics);
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diagnostics);
    }
}

fn check_html_element(element: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let tag = match html_attrs::element_tag(element) {
        Some(t) => t,
        None => return,
    };

    if !html_attrs::tag_name(&tag, source).is_some_and(|n| n.eq_ignore_ascii_case("html")) {
        return;
    }

    // A bound `:lang`/`v-bind:lang` provides a (dynamic) language → treat as set.
    // A static `lang` must have a non-empty value.
    let lang_ok = match html_attrs::attrs(&tag, source).into_iter().find(|a| a.name_eq("lang")) {
        Some(a) if a.bound => true,
        Some(a) => a.value.as_deref().is_some_and(|v| !v.trim().is_empty()),
        None => false,
    };

    if !lang_ok {
        diagnostics.push(make_diagnostic(element));
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

    fn check_vue(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Vue).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = HtmlLang;
        rule.check(&tree.root_node(), source, FileType::Vue)
    }

    #[test]
    fn test_vue_bound_lang_passes() {
        let diags = check_vue(r#"<html :lang="locale"><body></body></html>"#);
        assert_eq!(diags.len(), 0, "bound :lang counts as set, got: {diags:?}");
    }

    #[test]
    fn test_vue_missing_lang_fails() {
        let diags = check_vue("<html><body></body></html>");
        assert_eq!(diags.len(), 1);
    }
}
