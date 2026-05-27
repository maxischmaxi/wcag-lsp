use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::html_attrs;
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
    // A bound role (`:role="x"`) is a runtime expression — can't validate it.
    if html_attrs::element_attrs(element, source)
        .iter()
        .any(|a| a.name_eq("role") && a.bound)
    {
        return;
    }

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
    html_attrs::element_attrs(element, source)
        .into_iter()
        .find(|a| a.name_eq("role"))
        .and_then(|a| a.value)
}

fn get_html_child_role(element: &Node, source: &str) -> Option<String> {
    let attrs = html_attrs::element_attrs(element, source);

    // An explicit role attribute wins. A bound `:role` is unknown at lint time.
    if let Some(role_attr) = attrs.iter().find(|a| a.name_eq("role")) {
        if role_attr.bound {
            return None;
        }
        return role_attr.value.clone();
    }

    // Otherwise fall back to the implicit role of the tag name.
    if let Some(tag) = html_attrs::element_tag_name(element, source)
        && let Some(implicit) = IMPLICIT_ROLES.get(tag.to_ascii_lowercase().as_str())
    {
        return Some(implicit.to_string());
    }
    None
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

    // Collect effective child elements. Besides literal child elements, JSX
    // renders elements produced inside expression containers such as
    // `{items.map(x => <Option/>)}`, `{cond && <Option/>}` or ternaries, so we
    // descend into jsx_expression children to find those too.
    let mut effective_children: Vec<Node> = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "jsx_element" | "jsx_self_closing_element" => effective_children.push(child),
            "jsx_expression" => collect_expression_jsx(&child, &mut effective_children),
            _ => {}
        }
    }

    let has_child_elements = !effective_children.is_empty();
    let mut found_required = false;
    for child in &effective_children {
        let child_role = get_jsx_child_role(child, source);
        if let Some(ref cr) = child_role
            && required_children.iter().any(|r| r == cr)
        {
            found_required = true;
            break;
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

/// Collect the "top-level" JSX elements produced inside an expression container.
///
/// Handles the common React patterns where children are generated dynamically,
/// e.g. `{items.map(x => <Option/>)}`, `{cond && <Option/>}` or ternaries. The
/// walk stops descending as soon as it reaches a concrete JSX element, so we
/// only collect the elements that would render as direct children of the parent
/// — not their own descendants.
fn collect_expression_jsx<'a>(node: &Node<'a>, out: &mut Vec<Node<'a>>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "jsx_element" | "jsx_self_closing_element" => out.push(child),
            _ => collect_expression_jsx(&child, out),
        }
    }
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

    fn check_vue(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Vue).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AriaRequiredChildren;
        rule.check(&tree.root_node(), source, FileType::Vue)
    }

    #[test]
    fn test_vue_bound_role_skips_check() {
        // Parent role is dynamic; we can't know what children it requires.
        let diags =
            check_vue(r#"<template><div :role="r"><div>x</div></div></template>"#);
        assert_eq!(diags.len(), 0, "bound role can't be validated, got: {diags:?}");
    }

    #[test]
    fn test_vue_static_listbox_with_option_passes() {
        let diags = check_vue(
            r#"<template><div role="listbox"><div role="option">x</div></div></template>"#,
        );
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_vue_static_listbox_without_option_fails() {
        let diags =
            check_vue(r#"<template><div role="listbox"><div>x</div></div></template>"#);
        assert_eq!(diags.len(), 1);
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
    fn test_tsx_listbox_with_mapped_options_passes() {
        let diags = check_tsx(
            r#"const App = () => <div role="listbox">{items.map((p, i) => (<div role="option" key={i}>{p.label}</div>))}</div>;"#,
        );
        assert_eq!(diags.len(), 0, "mapped options should satisfy listbox, got: {diags:?}");
    }

    #[test]
    fn test_tsx_listbox_with_conditional_option_passes() {
        let diags = check_tsx(
            r#"const App = () => <div role="listbox">{show && <div role="option">x</div>}</div>;"#,
        );
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_listbox_with_mapped_non_options_fails() {
        let diags = check_tsx(
            r#"const App = () => <div role="listbox">{items.map((p) => (<div>{p.label}</div>))}</div>;"#,
        );
        assert_eq!(diags.len(), 1);
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
