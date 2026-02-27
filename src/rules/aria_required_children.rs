use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use std::collections::HashMap;
use std::sync::LazyLock;
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct AriaRequiredChildren;

static METADATA: RuleMetadata = RuleMetadata {
    id: "aria-required-children",
    description: "Elements with ARIA roles must have required child roles",
    wcag_level: WcagLevel::A,
    wcag_criterion: "1.3.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html",
    default_severity: Severity::Error,
};

static REQUIRED_CHILDREN_BY_ROLE: LazyLock<HashMap<&'static str, Vec<&'static str>>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();
        map.insert("feed", vec!["article"]);
        map.insert("grid", vec!["row", "rowgroup"]);
        map.insert("list", vec!["listitem"]);
        map.insert("listbox", vec!["option"]);
        map.insert(
            "menu",
            vec!["menuitem", "menuitemcheckbox", "menuitemradio"],
        );
        map.insert(
            "menubar",
            vec!["menuitem", "menuitemcheckbox", "menuitemradio"],
        );
        map.insert("radiogroup", vec!["radio"]);
        map.insert("row", vec!["cell", "columnheader", "gridcell", "rowheader"]);
        map.insert("rowgroup", vec!["row"]);
        map.insert("tablist", vec!["tab"]);
        map.insert("table", vec!["row", "rowgroup"]);
        map.insert("tree", vec!["treeitem"]);
        map.insert("treegrid", vec!["row"]);
        map
    });

static IMPLICIT_ROLES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert("li", "listitem");
    map.insert("tr", "row");
    map.insert("td", "cell");
    map.insert("th", "columnheader");
    map.insert("option", "option");
    map.insert("article", "article");
    map.insert("thead", "rowgroup");
    map.insert("tbody", "rowgroup");
    map.insert("tfoot", "rowgroup");
    map
});

impl Rule for AriaRequiredChildren {
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

    let required_children = match REQUIRED_CHILDREN_BY_ROLE.get(role.as_str()) {
        Some(children) => children,
        None => return,
    };

    // Scan direct child elements
    let mut found_required = false;
    let mut has_child_elements = false;

    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "element" {
            has_child_elements = true;
            let child_role = get_html_child_role(&child, source);
            if let Some(ref cr) = child_role
                && required_children.iter().any(|r| r == cr)
            {
                found_required = true;
                break;
            }
        }
    }

    if !found_required {
        diagnostics.push(make_diagnostic(
            element,
            &role,
            required_children,
            has_child_elements,
        ));
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

fn get_html_child_role(element: &Node, source: &str) -> Option<String> {
    // First check for explicit role attribute
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
                if tag_child.kind() == "tag_name" {
                    let tag_name = source[tag_child.byte_range()].to_lowercase();
                    if let Some(implicit) = IMPLICIT_ROLES.get(tag_name.as_str()) {
                        // Check for explicit role first
                        let mut inner_cursor = child.walk();
                        for inner_child in child.children(&mut inner_cursor) {
                            if inner_child.kind() == "attribute" {
                                let (n, v) = extract_html_attribute(&inner_child, source);
                                if let Some(ref attr_name) = n
                                    && attr_name.eq_ignore_ascii_case("role")
                                {
                                    return v;
                                }
                            }
                        }
                        return Some(implicit.to_string());
                    }
                }
            }
        }
    }
    None
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
        "jsx_element" => {
            check_jsx_element(node, source, diagnostics);
        }
        "jsx_self_closing_element" => {
            check_jsx_self_closing(node, source, diagnostics);
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

    let required_children = match REQUIRED_CHILDREN_BY_ROLE.get(role.as_str()) {
        Some(children) => children,
        None => return,
    };

    // Self-closing elements have no children, so they always fail if a role requires children
    diagnostics.push(make_diagnostic(node, &role, required_children, false));
}

fn check_jsx_element(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let role = match get_jsx_element_role(node, source) {
        Some(r) => r,
        None => return,
    };

    let required_children = match REQUIRED_CHILDREN_BY_ROLE.get(role.as_str()) {
        Some(children) => children,
        None => return,
    };

    // Scan direct child jsx_element and jsx_self_closing_element nodes
    let mut found_required = false;
    let mut has_child_elements = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_element" || child.kind() == "jsx_self_closing_element" {
            has_child_elements = true;
            let child_role = get_jsx_child_role(&child, source);
            if let Some(ref cr) = child_role
                && required_children.iter().any(|r| r == cr)
            {
                found_required = true;
                break;
            }
        }
    }

    if !found_required {
        diagnostics.push(make_diagnostic(
            node,
            &role,
            required_children,
            has_child_elements,
        ));
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

fn get_jsx_child_role(node: &Node, source: &str) -> Option<String> {
    match node.kind() {
        "jsx_self_closing_element" => get_jsx_role_from_attrs(node, source),
        "jsx_element" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "jsx_opening_element" {
                    return get_jsx_role_from_attrs(&child, source);
                }
            }
            None
        }
        _ => None,
    }
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

fn make_diagnostic(
    node: &Node,
    role: &str,
    required_children: &[&str],
    _has_child_elements: bool,
) -> Diagnostic {
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
            "Role '{}' requires children with roles: {}. {} [WCAG {} Level {:?}]",
            role,
            required_children.join(", "),
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
        let rule = AriaRequiredChildren;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AriaRequiredChildren;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_list_with_listitem_passes() {
        let diags = check_html(r#"<div role="list"><div role="listitem">item</div></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_list_without_listitem_fails() {
        let diags = check_html(r#"<div role="list"><div>not a listitem</div></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-required-children".to_string()))
        );
    }

    #[test]
    fn test_list_with_implicit_listitem_passes() {
        let diags = check_html(r#"<ul role="list"><li>item</li></ul>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tablist_with_tab_passes() {
        let diags = check_html(r#"<div role="tablist"><div role="tab">Tab</div></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tablist_without_tab_fails() {
        let diags = check_html(r#"<div role="tablist"><div>not a tab</div></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-required-children".to_string()))
        );
    }

    #[test]
    fn test_grid_with_row_passes() {
        let diags = check_html(
            r#"<div role="grid"><div role="row"><div role="gridcell">cell</div></div></div>"#,
        );
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_button_no_required_children_passes() {
        let diags = check_html(r#"<div role="button">click</div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_role_passes() {
        let diags = check_html(r#"<div>no role</div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_list_with_listitem_passes() {
        let diags = check_tsx(
            r#"const App = () => <div role="list"><div role="listitem">item</div></div>;"#,
        );
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_list_without_listitem_fails() {
        let diags =
            check_tsx(r#"const App = () => <div role="list"><div>not a listitem</div></div>;"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-required-children".to_string()))
        );
    }
}
