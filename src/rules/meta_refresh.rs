use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::html_attrs;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct MetaRefresh;

static METADATA: RuleMetadata = RuleMetadata {
    id: "meta-refresh",
    description: "Do not use meta refresh with a time limit",
    wcag_level: WcagLevel::A,
    wcag_criterion: "2.2.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/timing-adjustable.html",
    default_severity: Severity::Error,
};

impl Rule for MetaRefresh {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(&self, root: &Node, source: &str, file_type: FileType) -> Vec<Diagnostic> {
        // This rule is HTML-only; JSX doesn't have meta tags in components.
        if file_type.is_jsx_like() {
            return Vec::new();
        }

        let mut diagnostics = Vec::new();
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

    if !html_attrs::tag_name(&tag, source).is_some_and(|n| n.eq_ignore_ascii_case("meta")) {
        return;
    }

    let mut is_refresh = false;
    let mut content_value: Option<String> = None;

    for attr in html_attrs::attrs(&tag, source) {
        // Bound `:http-equiv`/`:content` are runtime expressions — can't evaluate.
        if attr.bound {
            continue;
        }
        if attr.name_eq("http-equiv")
            && attr.value.as_deref().is_some_and(|v| v.eq_ignore_ascii_case("refresh"))
        {
            is_refresh = true;
        }
        if attr.name_eq("content") {
            content_value = attr.value;
        }
    }

    if is_refresh
        && let Some(ref content) = content_value
        && has_nonzero_delay(content)
    {
        diagnostics.push(make_diagnostic(element));
    }
}

/// Check whether the content attribute value starts with a number > 0.
/// content="0;url=/new" → false (immediate redirect, OK)
/// content="5" → true (5-second delay)
/// content="30;url=/new" → true (30-second delay)
fn has_nonzero_delay(content: &str) -> bool {
    let trimmed = content.trim();
    // Extract the leading number before any semicolon
    let num_part = if let Some(idx) = trimmed.find(';') {
        &trimmed[..idx]
    } else {
        trimmed
    };
    let num_part = num_part.trim();
    if let Ok(n) = num_part.parse::<u64>() {
        n > 0
    } else {
        false
    }
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
        let rule = MetaRefresh;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    #[test]
    fn test_meta_refresh_with_delay_fails() {
        let diags = check_html(r#"<meta http-equiv="refresh" content="5">"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("meta-refresh".to_string()))
        );
    }

    #[test]
    fn test_meta_refresh_immediate_redirect_passes() {
        let diags = check_html(r#"<meta http-equiv="refresh" content="0;url=/new">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_meta_refresh_with_delay_and_url_fails() {
        let diags = check_html(r#"<meta http-equiv="refresh" content="30;url=/new">"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_meta_charset_passes() {
        let diags = check_html(r#"<meta charset="utf-8">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_meta_element_passes() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_jsx_skipped() {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let source = r#"const App = () => <div />;"#;
        let tree = parser.parse(source, None).unwrap();
        let rule = MetaRefresh;
        let diags = rule.check(&tree.root_node(), source, FileType::Tsx);
        assert_eq!(diags.len(), 0);
    }
}
