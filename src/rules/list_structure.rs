use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct ListStructure;

static METADATA: RuleMetadata = RuleMetadata {
    id: "list-structure",
    description: "List items must be contained in appropriate list elements",
    wcag_level: WcagLevel::A,
    wcag_criterion: "1.3.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html",
    default_severity: Severity::Error,
};

/// Valid parent tag names for <li> elements.
const LI_PARENTS: &[&str] = &["ul", "ol", "menu"];

/// Valid parent tag names for <dt> and <dd> elements.
const DT_DD_PARENTS: &[&str] = &["dl"];

impl Rule for ListStructure {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(&self, root: &Node, source: &str, file_type: FileType) -> Vec<Diagnostic> {
        // HTML-only rule; JSX structure is harder to validate statically.
        if file_type.is_jsx_like() {
            return Vec::new();
        }

        let mut diagnostics = Vec::new();
        visit_html(root, source, &mut diagnostics);
        diagnostics
    }
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
    let tag_name = match get_tag_name(element, source) {
        Some(name) => name,
        None => return,
    };

    let required_parents = if tag_name.eq_ignore_ascii_case("li") {
        LI_PARENTS
    } else if tag_name.eq_ignore_ascii_case("dt") || tag_name.eq_ignore_ascii_case("dd") {
        DT_DD_PARENTS
    } else {
        return;
    };

    let parent_tag = get_parent_element_tag(element, source);

    let valid = match parent_tag {
        Some(ref name) => required_parents
            .iter()
            .any(|p| p.eq_ignore_ascii_case(name)),
        None => false,
    };

    if !valid {
        diagnostics.push(make_diagnostic(element));
    }
}

/// Extract the tag name from an "element" node by inspecting its "start_tag" > "tag_name" child.
fn get_tag_name(element: &Node, source: &str) -> Option<String> {
    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" {
            let mut tag_cursor = child.walk();
            for tag_child in child.children(&mut tag_cursor) {
                if tag_child.kind() == "tag_name" {
                    return Some(source[tag_child.byte_range()].to_string());
                }
            }
        }
    }
    None
}

/// Walk up ancestors to find the nearest parent "element" node and return its tag name.
fn get_parent_element_tag(node: &Node, source: &str) -> Option<String> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "element" {
            return get_tag_name(&parent, source);
        }
        current = parent.parent();
    }
    None
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
        let rule = ListStructure;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    #[test]
    fn test_li_inside_ul_passes() {
        let diags = check_html(r#"<ul><li>Item</li></ul>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_li_inside_ol_passes() {
        let diags = check_html(r#"<ol><li>Item</li></ol>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_li_inside_menu_passes() {
        let diags = check_html(r#"<menu><li>Item</li></menu>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_li_inside_div_fails() {
        let diags = check_html(r#"<div><li>Item</li></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("list-structure".to_string()))
        );
    }

    #[test]
    fn test_dt_inside_dl_passes() {
        let diags = check_html(r#"<dl><dt>Term</dt></dl>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_dd_inside_dl_passes() {
        let diags = check_html(r#"<dl><dd>Definition</dd></dl>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_dt_inside_div_fails() {
        let diags = check_html(r#"<div><dt>Term</dt></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("list-structure".to_string()))
        );
    }

    #[test]
    fn test_dd_inside_div_fails() {
        let diags = check_html(r#"<div><dd>Definition</dd></div>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_nested_valid_structure_passes() {
        let diags = check_html(r#"<ul><li>Item <ol><li>Nested</li></ol></li></ul>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_list_items_passes() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_jsx_returns_empty() {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let source = r#"const App = () => <div><li>Item</li></div>;"#;
        let tree = parser.parse(source, None).unwrap();
        let rule = ListStructure;
        let diags = rule.check(&tree.root_node(), source, FileType::Tsx);
        assert_eq!(diags.len(), 0);
    }
}
