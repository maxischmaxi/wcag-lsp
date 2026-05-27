use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct NestedInteractive;

static METADATA: RuleMetadata = RuleMetadata {
    id: "nested-interactive",
    description: "Interactive elements must not be nested inside other interactive elements",
    wcag_level: WcagLevel::A,
    wcag_criterion: "4.1.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html",
    default_severity: Severity::Error,
};

const INTERACTIVE_TAGS: &[&str] = &["a", "button", "select", "textarea"];

const INTERACTIVE_ROLES: &[&str] = &[
    "button",
    "link",
    "tab",
    "checkbox",
    "radio",
    "textbox",
    "combobox",
    "listbox",
    "menuitem",
    "menuitemcheckbox",
    "menuitemradio",
    "option",
    "switch",
    "searchbox",
    "spinbutton",
    "slider",
    "treeitem",
    "gridcell",
];

/// In JSX, components starting with an uppercase letter are custom React components.
fn is_custom_component(name: &str) -> bool {
    name.starts_with(char::is_uppercase)
}

impl Rule for NestedInteractive {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(&self, root: &Node, source: &str, file_type: FileType) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        if file_type.is_jsx_like() {
            visit_jsx(root, source, &mut diagnostics, None);
        } else {
            visit_html(root, source, &mut diagnostics, None);
        }
        diagnostics
    }
}

// ---------------------------------------------------------------------------
// Composite widgets
// ---------------------------------------------------------------------------

/// Roles that act as composite-widget containers, mapped to the interactive
/// child roles they are expected to own. A child with one of these roles nested
/// inside the matching container is a valid ARIA pattern (e.g. `option` inside
/// `listbox`) and must not be reported as nested-interactive.
fn composite_children(parent_role: &str) -> &'static [&'static str] {
    match parent_role {
        "listbox" | "combobox" => &["option"],
        "menu" | "menubar" => &["menuitem", "menuitemcheckbox", "menuitemradio"],
        "tablist" => &["tab"],
        "tree" => &["treeitem"],
        "treegrid" | "grid" => &["row", "gridcell", "rowheader", "columnheader"],
        "radiogroup" => &["radio"],
        "row" => &["gridcell", "columnheader", "rowheader", "cell"],
        _ => &[],
    }
}

/// Whether `child_role` is an expected composite child of `parent_role`.
fn composite_allows(parent_role: &str, child_role: Option<&str>) -> bool {
    match child_role {
        Some(cr) => composite_children(parent_role).contains(&cr),
        None => false,
    }
}

// ---------------------------------------------------------------------------
// HTML helpers
// ---------------------------------------------------------------------------

/// Extract the tag name from an HTML element's start_tag or self_closing_tag.
fn get_html_tag_name<'a>(element: &Node, source: &'a str) -> Option<&'a str> {
    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" || child.kind() == "self_closing_tag" {
            let mut tag_cursor = child.walk();
            for tag_child in child.children(&mut tag_cursor) {
                if tag_child.kind() == "tag_name" {
                    return Some(&source[tag_child.byte_range()]);
                }
            }
        }
    }
    None
}

/// Get the value of an HTML attribute by name (case-insensitive).
fn get_html_attr_value(element: &Node, source: &str, attr_name: &str) -> Option<String> {
    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" || child.kind() == "self_closing_tag" {
            let mut tag_cursor = child.walk();
            for tag_child in child.children(&mut tag_cursor) {
                if tag_child.kind() == "attribute" {
                    let mut found_name = false;
                    let mut attr_cursor = tag_child.walk();
                    for attr_child in tag_child.children(&mut attr_cursor) {
                        if attr_child.kind() == "attribute_name" {
                            let name = &source[attr_child.byte_range()];
                            if name.eq_ignore_ascii_case(attr_name) {
                                found_name = true;
                            }
                        }
                        if found_name && attr_child.kind() == "quoted_attribute_value" {
                            let mut val_cursor = attr_child.walk();
                            for val_child in attr_child.children(&mut val_cursor) {
                                if val_child.kind() == "attribute_value" {
                                    return Some(source[val_child.byte_range()].to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Check if an HTML attribute exists on the element (case-insensitive).
fn has_html_attr(element: &Node, source: &str, attr_name: &str) -> bool {
    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" || child.kind() == "self_closing_tag" {
            let mut tag_cursor = child.walk();
            for tag_child in child.children(&mut tag_cursor) {
                if tag_child.kind() == "attribute" {
                    let mut attr_cursor = tag_child.walk();
                    for attr_child in tag_child.children(&mut attr_cursor) {
                        if attr_child.kind() == "attribute_name" {
                            let name = &source[attr_child.byte_range()];
                            if name.eq_ignore_ascii_case(attr_name) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

/// Determine whether an HTML element is interactive.
fn is_html_interactive(element: &Node, source: &str) -> bool {
    let tag_name = match get_html_tag_name(element, source) {
        Some(name) => name.to_ascii_lowercase(),
        None => return false,
    };

    // 1. Tag is in INTERACTIVE_TAGS
    if INTERACTIVE_TAGS
        .iter()
        .any(|t| t.eq_ignore_ascii_case(&tag_name))
    {
        return true;
    }

    // 2. <input> that is NOT type="hidden"
    if tag_name == "input" {
        let type_val = get_html_attr_value(element, source, "type");
        if let Some(ref val) = type_val
            && val.eq_ignore_ascii_case("hidden")
        {
            return false;
        }
        return true;
    }

    // 3. Has tabindex attribute with a value other than "-1"
    if has_html_attr(element, source, "tabindex") {
        let val = get_html_attr_value(element, source, "tabindex");
        match val {
            Some(v) if v.trim() == "-1" => {}
            _ => return true,
        }
    }

    // 4. Has a role attribute whose value is in INTERACTIVE_ROLES
    if let Some(role_val) = get_html_attr_value(element, source, "role") {
        let role = role_val.trim().to_ascii_lowercase();
        if INTERACTIVE_ROLES.iter().any(|r| *r == role) {
            return true;
        }
    }

    false
}

/// The explicit interactive ARIA role of an HTML element, if any.
fn html_interactive_role(element: &Node, source: &str) -> Option<String> {
    let role = get_html_attr_value(element, source, "role")?;
    let r = role.trim().to_ascii_lowercase();
    INTERACTIVE_ROLES.iter().any(|x| *x == r).then_some(r)
}

/// An identifier for an interactive element used as the parent context when
/// recursing: its interactive ARIA role if it has one, otherwise its tag name.
/// Returns `None` for non-interactive elements.
fn html_interactive_identity(element: &Node, source: &str) -> Option<String> {
    if !is_html_interactive(element, source) {
        return None;
    }
    html_interactive_role(element, source)
        .or_else(|| get_html_tag_name(element, source).map(|t| t.to_ascii_lowercase()))
}

// ---------------------------------------------------------------------------
// HTML visitor
// ---------------------------------------------------------------------------

fn visit_html(
    node: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    parent_role: Option<&str>,
) {
    if node.kind() == "element" {
        let identity = html_interactive_identity(node, source);
        let role = html_interactive_role(node, source);

        if let Some(parent) = parent_role
            && identity.is_some()
            && !composite_allows(parent, role.as_deref())
        {
            diagnostics.push(make_diagnostic(node));
        }

        // The current element becomes the parent context for its descendants
        // when it is interactive; otherwise the existing context carries on.
        let child_role = identity.as_deref().or(parent_role);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            visit_html(&child, source, diagnostics, child_role);
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diagnostics, parent_role);
    }
}

// ---------------------------------------------------------------------------
// JSX helpers
// ---------------------------------------------------------------------------

/// Get the tag name from a JSX opening element or self-closing element node.
fn get_jsx_tag_name_from_opening<'a>(opening: &Node, source: &'a str) -> Option<&'a str> {
    let mut cursor = opening.walk();
    for child in opening.children(&mut cursor) {
        if child.kind() == "identifier" {
            return Some(&source[child.byte_range()]);
        }
    }
    None
}

/// Get the value of a JSX attribute by name from an opening/self-closing element node.
fn get_jsx_attr_value(opening: &Node, source: &str, attr_name: &str) -> Option<String> {
    let mut cursor = opening.walk();
    for child in opening.children(&mut cursor) {
        if child.kind() == "jsx_attribute" {
            let mut found_name = false;
            let mut attr_cursor = child.walk();
            for attr_child in child.children(&mut attr_cursor) {
                if attr_child.kind() == "property_identifier" {
                    let name = &source[attr_child.byte_range()];
                    if name == attr_name {
                        found_name = true;
                    }
                }
                if found_name && attr_child.kind() == "string" {
                    let raw = &source[attr_child.byte_range()];
                    let trimmed = raw.trim_matches('"').trim_matches('\'');
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

/// Check if a JSX attribute exists on the opening/self-closing element node.
fn has_jsx_attr(opening: &Node, source: &str, attr_name: &str) -> bool {
    let mut cursor = opening.walk();
    for child in opening.children(&mut cursor) {
        if child.kind() == "jsx_attribute" {
            let mut attr_cursor = child.walk();
            for attr_child in child.children(&mut attr_cursor) {
                if attr_child.kind() == "property_identifier" {
                    let name = &source[attr_child.byte_range()];
                    if name == attr_name {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Determine whether a JSX element is interactive based on the opening/self-closing element node.
fn is_jsx_interactive(opening: &Node, source: &str) -> bool {
    let tag_name = match get_jsx_tag_name_from_opening(opening, source) {
        Some(name) => name,
        None => return false,
    };

    // Skip custom components
    if is_custom_component(tag_name) {
        return false;
    }

    // 1. Tag is in INTERACTIVE_TAGS
    if INTERACTIVE_TAGS.contains(&tag_name) {
        return true;
    }

    // 2. <input> that is NOT type="hidden"
    if tag_name == "input" {
        let type_val = get_jsx_attr_value(opening, source, "type");
        if let Some(ref val) = type_val
            && val == "hidden"
        {
            return false;
        }
        return true;
    }

    // 3. Has tabindex/tabIndex attribute with a value other than "-1"
    let has_tabindex =
        has_jsx_attr(opening, source, "tabindex") || has_jsx_attr(opening, source, "tabIndex");
    if has_tabindex {
        let val = get_jsx_attr_value(opening, source, "tabindex")
            .or_else(|| get_jsx_attr_value(opening, source, "tabIndex"));
        match val {
            Some(v) if v.trim() == "-1" => {}
            _ => return true,
        }
    }

    // 4. Has a role attribute whose value is in INTERACTIVE_ROLES
    if let Some(role_val) = get_jsx_attr_value(opening, source, "role") {
        let role = role_val.trim().to_ascii_lowercase();
        if INTERACTIVE_ROLES.iter().any(|r| *r == role) {
            return true;
        }
    }

    false
}

/// The explicit interactive ARIA role declared on a JSX opening/self-closing
/// element node, if any.
fn jsx_interactive_role(opening: &Node, source: &str) -> Option<String> {
    let role = get_jsx_attr_value(opening, source, "role")?;
    let r = role.trim().to_ascii_lowercase();
    INTERACTIVE_ROLES.iter().any(|x| *x == r).then_some(r)
}

/// An identifier for an interactive JSX element used as the parent context:
/// its interactive ARIA role if it has one, otherwise its tag name. Returns
/// `None` for non-interactive elements (and custom components).
fn jsx_interactive_identity(opening: &Node, source: &str) -> Option<String> {
    if !is_jsx_interactive(opening, source) {
        return None;
    }
    jsx_interactive_role(opening, source)
        .or_else(|| get_jsx_tag_name_from_opening(opening, source).map(|t| t.to_ascii_lowercase()))
}

// ---------------------------------------------------------------------------
// JSX visitor
// ---------------------------------------------------------------------------

fn visit_jsx(
    node: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    parent_role: Option<&str>,
) {
    match node.kind() {
        "jsx_element" => {
            // Find the jsx_opening_element to determine interactivity
            let mut opening: Option<Node> = None;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "jsx_opening_element" {
                    opening = Some(child);
                    break;
                }
            }

            let identity = opening
                .as_ref()
                .and_then(|o| jsx_interactive_identity(o, source));
            let role = opening.as_ref().and_then(|o| jsx_interactive_role(o, source));

            if let Some(parent) = parent_role
                && identity.is_some()
                && !composite_allows(parent, role.as_deref())
            {
                diagnostics.push(make_diagnostic(node));
            }

            let child_role = identity.as_deref().or(parent_role);
            let mut cursor2 = node.walk();
            for child in node.children(&mut cursor2) {
                visit_jsx(&child, source, diagnostics, child_role);
            }
        }
        "jsx_self_closing_element" => {
            let identity = jsx_interactive_identity(node, source);
            let role = jsx_interactive_role(node, source);

            if let Some(parent) = parent_role
                && identity.is_some()
                && !composite_allows(parent, role.as_deref())
            {
                diagnostics.push(make_diagnostic(node));
            }

            // Self-closing elements have no children to recurse into
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                visit_jsx(&child, source, diagnostics, parent_role);
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
        let rule = NestedInteractive;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = NestedInteractive;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_button_containing_anchor_fails() {
        let diags = check_html(r#"<button><a href="/">link</a></button>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("nested-interactive".to_string()))
        );
    }

    #[test]
    fn test_anchor_containing_button_fails() {
        let diags = check_html(r#"<a href="/"><button>click</button></a>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("nested-interactive".to_string()))
        );
    }

    #[test]
    fn test_button_alone_passes() {
        let diags = check_html(r#"<button>text</button>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_anchor_alone_passes() {
        let diags = check_html(r#"<a href="/">link</a>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_div_containing_anchor_passes() {
        let diags = check_html(r#"<div><a href="/">link</a></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_button_with_hidden_input_passes() {
        let diags = check_html(r#"<button><input type="hidden"></button>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_button_with_focusable_div_fails() {
        let diags = check_html(r#"<button><div tabindex="0">focusable</div></button>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("nested-interactive".to_string()))
        );
    }

    #[test]
    fn test_tsx_button_containing_anchor_fails() {
        let diags = check_tsx(r#"const App = () => <button><a href="/">link</a></button>;"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("nested-interactive".to_string()))
        );
    }

    #[test]
    fn test_tsx_button_alone_passes() {
        let diags = check_tsx(r#"const App = () => <button>text</button>;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_listbox_with_option_passes() {
        let diags =
            check_html(r#"<div role="listbox"><div role="option">a</div></div>"#);
        assert_eq!(diags.len(), 0, "option is a valid child of listbox, got: {diags:?}");
    }

    #[test]
    fn test_tsx_listbox_with_mapped_options_passes() {
        let diags = check_tsx(
            r#"const App = () => <div role="listbox">{items.map((p, i) => (<div role="option" key={i} onClick={f}>{p.label}</div>))}</div>;"#,
        );
        assert_eq!(diags.len(), 0, "mapped options are valid listbox children, got: {diags:?}");
    }

    #[test]
    fn test_tablist_with_tab_passes() {
        let diags = check_html(r#"<div role="tablist"><div role="tab">Tab</div></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_menu_with_menuitem_passes() {
        let diags = check_html(r#"<div role="menu"><div role="menuitem">Item</div></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_listbox_with_button_still_fails() {
        // A button is not a valid composite child of listbox.
        let diags = check_html(r#"<div role="listbox"><button>x</button></div>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_listbox_with_nested_tab_still_fails() {
        // role="tab" is interactive but not an expected child of listbox.
        let diags =
            check_html(r#"<div role="listbox"><div role="tab">x</div></div>"#);
        assert_eq!(diags.len(), 1);
    }
}
