use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct Tabindex;

static METADATA: RuleMetadata = RuleMetadata {
    id: "no-positive-tabindex",
    description: "Avoid positive tabindex values",
    wcag_level: WcagLevel::A,
    wcag_criterion: "2.4.3",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/focus-order.html",
    default_severity: Severity::Warning,
};

impl Rule for Tabindex {
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
    if node.kind() == "attribute" {
        check_html_attribute(node, source, diagnostics);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diagnostics);
    }
}

fn check_html_attribute(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut is_tabindex = false;
    let mut value: Option<String> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute_name" {
            let name = &source[child.byte_range()];
            if name.eq_ignore_ascii_case("tabindex") {
                is_tabindex = true;
            }
        }
        if child.kind() == "quoted_attribute_value" {
            let mut val_cursor = child.walk();
            for val_child in child.children(&mut val_cursor) {
                if val_child.kind() == "attribute_value" {
                    value = Some(source[val_child.byte_range()].to_string());
                }
            }
        }
    }

    if is_tabindex {
        if let Some(val) = value {
            if let Ok(n) = val.trim().parse::<i32>() {
                if n > 0 {
                    diagnostics.push(make_diagnostic(node));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// JSX / TSX
// ---------------------------------------------------------------------------

fn visit_jsx(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "jsx_attribute" {
        check_jsx_attribute(node, source, diagnostics);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_jsx(&child, source, diagnostics);
    }
}

fn check_jsx_attribute(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut is_tabindex = false;
    let mut value: Option<String> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "property_identifier" {
            let name = &source[child.byte_range()];
            if name == "tabIndex" || name == "tabindex" {
                is_tabindex = true;
            }
        }
        if child.kind() == "string" {
            let raw = &source[child.byte_range()];
            let trimmed = raw.trim_matches('"').trim_matches('\'');
            value = Some(trimmed.to_string());
        }
        // Handle JSX expression like tabIndex={5}
        if child.kind() == "jsx_expression" {
            let mut expr_cursor = child.walk();
            for expr_child in child.children(&mut expr_cursor) {
                if expr_child.kind() == "number" {
                    value = Some(source[expr_child.byte_range()].to_string());
                }
            }
        }
    }

    if is_tabindex {
        if let Some(val) = value {
            if let Ok(n) = val.trim().parse::<i32>() {
                if n > 0 {
                    diagnostics.push(make_diagnostic(node));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

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
        let rule = Tabindex;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = Tabindex;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_positive_tabindex_fails() {
        let diags = check_html(r#"<div tabindex="1"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("no-positive-tabindex".to_string()))
        );
    }

    #[test]
    fn test_tabindex_zero_passes() {
        let diags = check_html(r#"<div tabindex="0"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tabindex_negative_passes() {
        let diags = check_html(r#"<div tabindex="-1"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_tabindex_passes() {
        let diags = check_html(r#"<div></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_positive_tabindex_fails() {
        let diags = check_tsx(r#"const App = () => <div tabIndex="1" />;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_tabindex_zero_passes() {
        let diags = check_tsx(r#"const App = () => <div tabIndex="0" />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_tabindex_negative_passes() {
        let diags = check_tsx(r#"const App = () => <div tabIndex="-1" />;"#);
        assert_eq!(diags.len(), 0);
    }
}
