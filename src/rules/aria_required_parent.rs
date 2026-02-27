use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use std::collections::HashMap;
use std::sync::LazyLock;
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct AriaRequiredParent;

static METADATA: RuleMetadata = RuleMetadata {
    id: "aria-required-parent",
    description: "Elements with ARIA roles must be contained in required parent roles",
    wcag_level: WcagLevel::A,
    wcag_criterion: "1.3.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html",
    default_severity: Severity::Error,
};

static REQUIRED_PARENTS_BY_ROLE: LazyLock<HashMap<&'static str, Vec<&'static str>>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();
        map.insert("cell", vec!["row"]);
        map.insert("columnheader", vec!["row"]);
        map.insert("gridcell", vec!["row"]);
        map.insert("listitem", vec!["list", "group"]);
        map.insert("menuitem", vec!["menu", "menubar", "group"]);
        map.insert("menuitemcheckbox", vec!["menu", "menubar", "group"]);
        map.insert("menuitemradio", vec!["menu", "menubar", "group"]);
        map.insert("option", vec!["listbox", "group"]);
        map.insert("row", vec!["grid", "rowgroup", "table", "treegrid"]);
        map.insert("rowheader", vec!["row"]);
        map.insert("tab", vec!["tablist"]);
        map.insert("treeitem", vec!["tree", "group"]);
        map
    });

static IMPLICIT_PARENT_ROLES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert("ul", "list");
    map.insert("ol", "list");
    map.insert("menu", "list");
    map.insert("table", "table");
    map.insert("tr", "row");
    map.insert("thead", "rowgroup");
    map.insert("tbody", "rowgroup");
    map.insert("tfoot", "rowgroup");
    map
});

impl Rule for AriaRequiredParent {
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
        check_html_element(node, source, diagnostics);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diagnostics);
    }
}

fn check_html_element(element: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let role = match get_html_role(element, source) {
        Some(r) => r,
        None => return,
    };

    let required_parents = match REQUIRED_PARENTS_BY_ROLE.get(role.as_str()) {
        Some(parents) => parents,
        None => return,
    };

    if !has_required_html_parent(element, source, required_parents) {
        diagnostics.push(make_diagnostic(element, &role, required_parents));
    }
}

fn get_html_role(element: &Node, source: &str) -> Option<String> {
    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" || child.kind() == "self_closing_tag" {
            let mut tag_cursor = child.walk();
            for tag_child in child.children(&mut tag_cursor) {
                if tag_child.kind() == "attribute" {
                    let (name, value) = extract_html_attribute(&tag_child, source);
                    if let Some(ref n) = name
                        && n.eq_ignore_ascii_case("role")
                    {
                        return value;
                    }
                }
            }
        }
    }
    None
}

fn get_html_tag_name(element: &Node, source: &str) -> Option<String> {
    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" || child.kind() == "self_closing_tag" {
            let mut tag_cursor = child.walk();
            for tag_child in child.children(&mut tag_cursor) {
                if tag_child.kind() == "tag_name" {
                    return Some(source[tag_child.byte_range()].to_lowercase());
                }
            }
        }
    }
    None
}

fn get_ancestor_role_html(element: &Node, source: &str) -> Option<String> {
    // Check explicit role first
    if let Some(role) = get_html_role(element, source) {
        return Some(role);
    }
    // Check implicit role from tag name
    if let Some(tag) = get_html_tag_name(element, source)
        && let Some(implicit) = IMPLICIT_PARENT_ROLES.get(tag.as_str())
    {
        return Some(implicit.to_string());
    }
    None
}

fn has_required_html_parent(node: &Node, source: &str, required_parents: &[&str]) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "element"
            && let Some(role) = get_ancestor_role_html(&parent, source)
            && required_parents.iter().any(|r| *r == role)
        {
            return true;
        }
        current = parent.parent();
    }
    false
}

fn extract_html_attribute(attr_node: &Node, source: &str) -> (Option<String>, Option<String>) {
    let mut name = None;
    let mut value = None;

    let mut cursor = attr_node.walk();
    for child in attr_node.children(&mut cursor) {
        if child.kind() == "attribute_name" {
            name = Some(source[child.byte_range()].to_string());
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

    (name, value)
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
    let role = match get_jsx_role_from_attrs(node, source) {
        Some(r) => r,
        None => return,
    };

    let required_parents = match REQUIRED_PARENTS_BY_ROLE.get(role.as_str()) {
        Some(parents) => parents,
        None => return,
    };

    if !has_required_jsx_parent(node, source, required_parents) {
        diagnostics.push(make_diagnostic(node, &role, required_parents));
    }
}

fn check_jsx_element(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let role = match get_jsx_element_role(node, source) {
        Some(r) => r,
        None => return,
    };

    let required_parents = match REQUIRED_PARENTS_BY_ROLE.get(role.as_str()) {
        Some(parents) => parents,
        None => return,
    };

    if !has_required_jsx_parent(node, source, required_parents) {
        diagnostics.push(make_diagnostic(node, &role, required_parents));
    }
}

fn get_jsx_element_role(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            return get_jsx_role_from_attrs(&child, source);
        }
    }
    None
}

fn get_jsx_role_from_attrs(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_attribute" {
            let (name, value) = extract_jsx_attribute(&child, source);
            if let Some(ref n) = name
                && n == "role"
            {
                return value;
            }
        }
    }
    None
}

fn get_ancestor_role_jsx(node: &Node, source: &str) -> Option<String> {
    match node.kind() {
        "jsx_self_closing_element" => get_jsx_role_from_attrs(node, source),
        "jsx_element" => get_jsx_element_role(node, source),
        _ => None,
    }
}

fn has_required_jsx_parent(node: &Node, source: &str, required_parents: &[&str]) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if (parent.kind() == "jsx_element" || parent.kind() == "jsx_self_closing_element")
            && let Some(role) = get_ancestor_role_jsx(&parent, source)
            && required_parents.iter().any(|r| *r == role)
        {
            return true;
        }
        current = parent.parent();
    }
    false
}

fn extract_jsx_attribute(attr_node: &Node, source: &str) -> (Option<String>, Option<String>) {
    let mut name = None;
    let mut value = None;

    let mut cursor = attr_node.walk();
    for child in attr_node.children(&mut cursor) {
        if child.kind() == "property_identifier" {
            name = Some(source[child.byte_range()].to_string());
        }
        if child.kind() == "string" {
            let raw = &source[child.byte_range()];
            let trimmed = raw.trim_matches('"').trim_matches('\'');
            value = Some(trimmed.to_string());
        }
    }

    (name, value)
}

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

fn make_diagnostic(node: &Node, role: &str, required_parents: &[&str]) -> Diagnostic {
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
            "Role '{}' requires a parent with role: {}. {} [WCAG {} Level {:?}]",
            role,
            required_parents.join(", "),
            meta.description,
            meta.wcag_criterion,
            meta.wcag_level
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
        let rule = AriaRequiredParent;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AriaRequiredParent;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_listitem_in_list_passes() {
        let diags = check_html(r#"<div role="list"><div role="listitem">item</div></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_listitem_without_list_parent_fails() {
        let diags = check_html(r#"<div><div role="listitem">item</div></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-required-parent".to_string()))
        );
    }

    #[test]
    fn test_tab_in_tablist_passes() {
        let diags = check_html(r#"<div role="tablist"><div role="tab">Tab</div></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tab_without_tablist_parent_fails() {
        let diags = check_html(r#"<div><div role="tab">Tab</div></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-required-parent".to_string()))
        );
    }

    #[test]
    fn test_cell_in_row_passes() {
        let diags = check_html(
            r#"<div role="table"><div role="row"><div role="cell">Cell</div></div></div>"#,
        );
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_cell_without_row_parent_fails() {
        let diags = check_html(r#"<div><div role="cell">Cell</div></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-required-parent".to_string()))
        );
    }

    #[test]
    fn test_button_no_required_parent_passes() {
        let diags = check_html(r#"<div role="button">click</div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_listitem_in_list_passes() {
        let diags = check_tsx(
            r#"const App = () => <div role="list"><div role="listitem">item</div></div>;"#,
        );
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_listitem_without_list_parent_fails() {
        let diags = check_tsx(r#"const App = () => <div><div role="listitem">item</div></div>;"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-required-parent".to_string()))
        );
    }
}
