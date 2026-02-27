use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct ScopeAttr;

static METADATA: RuleMetadata = RuleMetadata {
    id: "scope-attr",
    description: "scope attribute should only be used on <th> elements",
    wcag_level: WcagLevel::A,
    wcag_criterion: "1.3.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html",
    default_severity: Severity::Warning,
};

impl Rule for ScopeAttr {
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
    let mut tag_name: Option<String> = None;
    let mut has_scope = false;

    let mut cursor = start_tag.walk();
    for child in start_tag.children(&mut cursor) {
        if child.kind() == "tag_name" {
            tag_name = Some(source[child.byte_range()].to_ascii_lowercase());
        }
        if child.kind() == "attribute" {
            let attr_name = extract_html_attr_name(&child, source);
            if let Some(name) = attr_name
                && name.eq_ignore_ascii_case("scope")
            {
                has_scope = true;
            }
        }
    }

    if has_scope
        && let Some(ref name) = tag_name
        && name != "th"
    {
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
    match node.kind() {
        "jsx_self_closing_element" => {
            check_jsx_self_closing(node, source, diagnostics);
        }
        "jsx_element" => {
            check_jsx_element(node, source, diagnostics);
        }
        _ => {}
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_jsx(&child, source, diagnostics);
    }
}

fn check_jsx_self_closing(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut tag_name: Option<String> = None;
    let mut has_scope = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            tag_name = Some(source[child.byte_range()].to_string());
        }
        if child.kind() == "jsx_attribute" {
            let attr_name = extract_jsx_attr_name(&child, source);
            if let Some(name) = attr_name
                && name == "scope"
            {
                has_scope = true;
            }
        }
    }

    if has_scope
        && let Some(ref name) = tag_name
        && name != "th"
    {
        diagnostics.push(make_diagnostic(node));
    }
}

fn check_jsx_element(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            let mut tag_name: Option<String> = None;
            let mut has_scope = false;

            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "identifier" {
                    tag_name = Some(source[inner_child.byte_range()].to_string());
                }
                if inner_child.kind() == "jsx_attribute" {
                    let attr_name = extract_jsx_attr_name(&inner_child, source);
                    if let Some(name) = attr_name
                        && name == "scope"
                    {
                        has_scope = true;
                    }
                }
            }

            if has_scope
                && let Some(ref name) = tag_name
                && name != "th"
            {
                diagnostics.push(make_diagnostic(node));
            }
        }
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
        let rule = ScopeAttr;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = ScopeAttr;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_th_with_scope_passes() {
        let diags = check_html(r#"<th scope="col">Name</th>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_td_with_scope_fails() {
        let diags = check_html(r#"<td scope="col">Name</td>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("scope-attr".to_string()))
        );
    }

    #[test]
    fn test_div_with_scope_fails() {
        let diags = check_html(r#"<div scope="row">Content</div>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_td_without_scope_passes() {
        let diags = check_html(r#"<td>Data</td>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_scope_passes() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_th_with_scope_passes() {
        let diags = check_tsx(r#"const App = () => <th scope="col">Name</th>;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_td_with_scope_fails() {
        let diags = check_tsx(r#"const App = () => <td scope="col">Name</td>;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_self_closing_td_with_scope_fails() {
        let diags = check_tsx(r#"const App = () => <td scope="col" />;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_self_closing_th_with_scope_passes() {
        let diags = check_tsx(r#"const App = () => <th scope="col" />;"#);
        assert_eq!(diags.len(), 0);
    }
}
