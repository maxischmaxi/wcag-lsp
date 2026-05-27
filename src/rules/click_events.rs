use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct ClickEvents;

static METADATA: RuleMetadata = RuleMetadata {
    id: "click-events-have-key-events",
    description: "Elements with onClick must also have onKeyDown or onKeyUp",
    wcag_level: WcagLevel::A,
    wcag_criterion: "2.1.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/keyboard.html",
    default_severity: Severity::Error,
};

/// Elements that natively handle keyboard events and don't need explicit key handlers.
const INTERACTIVE_TAGS: &[&str] = &["button", "a", "input", "select", "textarea"];

/// In JSX, components starting with an uppercase letter are custom React components.
/// They handle their own keyboard accessibility internally, so we skip them.
fn is_custom_component(name: &str) -> bool {
    name.starts_with(char::is_uppercase)
}

impl Rule for ClickEvents {
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
/// child roles whose keyboard interaction is managed by the container (e.g. a
/// `listbox` handling arrow keys for its `option`s). A click handler on such a
/// managed child does not need its own key handler, so it is exempt.
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

/// Whether `role` is a managed child of the composite container described by
/// `composite` (the allowed child roles of the nearest composite ancestor).
fn is_managed_child(composite: Option<&[&str]>, role: Option<&str>) -> bool {
    match (composite, role) {
        (Some(allowed), Some(r)) => allowed.contains(&r),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------

fn visit_html(
    node: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    composite: Option<&[&str]>,
) {
    if node.kind() == "element" {
        check_html_element(node, source, diagnostics, composite);

        // Descend with the composite context of this element's role, if it is a
        // composite container; otherwise carry the existing context onward.
        let role = html_element_role(node, source);
        let child_ctx = role
            .as_deref()
            .map(composite_children)
            .filter(|c| !c.is_empty())
            .or(composite);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            visit_html(&child, source, diagnostics, child_ctx);
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diagnostics, composite);
    }
}

/// The lowercased `role` attribute value of an HTML element, if present.
fn html_element_role(element: &Node, source: &str) -> Option<String> {
    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" || child.kind() == "self_closing_tag" {
            let mut tag_cursor = child.walk();
            for tag_child in child.children(&mut tag_cursor) {
                if tag_child.kind() == "attribute" {
                    let mut found = false;
                    let mut attr_cursor = tag_child.walk();
                    for attr_child in tag_child.children(&mut attr_cursor) {
                        if attr_child.kind() == "attribute_name"
                            && source[attr_child.byte_range()].eq_ignore_ascii_case("role")
                        {
                            found = true;
                        }
                        if found && attr_child.kind() == "quoted_attribute_value" {
                            let mut val_cursor = attr_child.walk();
                            for val_child in attr_child.children(&mut val_cursor) {
                                if val_child.kind() == "attribute_value" {
                                    return Some(
                                        source[val_child.byte_range()]
                                            .trim()
                                            .to_ascii_lowercase(),
                                    );
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

fn check_html_element(
    element: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    composite: Option<&[&str]>,
) {
    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" || child.kind() == "self_closing_tag" {
            check_html_tag(&child, source, diagnostics, element, composite);
        }
    }
}

fn check_html_tag(
    tag: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    element_node: &Node,
    composite: Option<&[&str]>,
) {
    let mut tag_name: Option<String> = None;
    let mut role: Option<String> = None;
    let mut has_onclick = false;
    let mut has_key_event = false;

    let mut cursor = tag.walk();
    for child in tag.children(&mut cursor) {
        if child.kind() == "tag_name" {
            tag_name = Some(source[child.byte_range()].to_ascii_lowercase());
        }
        if child.kind() == "attribute" {
            let attr_name = extract_html_attr_name(&child, source);
            if let Some(name) = attr_name {
                let lower = name.to_ascii_lowercase();
                if lower == "onclick" {
                    has_onclick = true;
                }
                if lower == "onkeydown" || lower == "onkeyup" {
                    has_key_event = true;
                }
                if lower == "role" {
                    role = extract_html_attr_value(&child, source).map(|v| v.trim().to_ascii_lowercase());
                }
            }
        }
    }

    // Skip interactive elements that natively handle keyboard
    if let Some(ref name) = tag_name
        && INTERACTIVE_TAGS.iter().any(|t| t == name)
    {
        return;
    }

    // Skip managed children of a composite widget (keyboard handled by container)
    if is_managed_child(composite, role.as_deref()) {
        return;
    }

    if has_onclick && !has_key_event {
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

fn extract_html_attr_value(attr_node: &Node, source: &str) -> Option<String> {
    let mut cursor = attr_node.walk();
    for child in attr_node.children(&mut cursor) {
        if child.kind() == "quoted_attribute_value" {
            let mut val_cursor = child.walk();
            for val_child in child.children(&mut val_cursor) {
                if val_child.kind() == "attribute_value" {
                    return Some(source[val_child.byte_range()].to_string());
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// JSX / TSX
// ---------------------------------------------------------------------------

fn visit_jsx(
    node: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    composite: Option<&[&str]>,
) {
    match node.kind() {
        "jsx_self_closing_element" => {
            check_jsx_self_closing(node, source, diagnostics, composite);
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                visit_jsx(&child, source, diagnostics, composite);
            }
        }
        "jsx_element" => {
            check_jsx_opening(node, source, diagnostics, composite);

            // Descend with the composite context of this element's role, if it
            // is a composite container; otherwise carry the existing one onward.
            let role = jsx_element_role(node, source);
            let child_ctx = role
                .as_deref()
                .map(composite_children)
                .filter(|c| !c.is_empty())
                .or(composite);
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                visit_jsx(&child, source, diagnostics, child_ctx);
            }
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                visit_jsx(&child, source, diagnostics, composite);
            }
        }
    }
}

fn check_jsx_self_closing(
    node: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    composite: Option<&[&str]>,
) {
    let mut tag_name: Option<String> = None;
    let mut has_onclick = false;
    let mut has_key_event = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            tag_name = Some(source[child.byte_range()].to_string());
        }
        if child.kind() == "jsx_attribute" {
            let attr_name = extract_jsx_attr_name(&child, source);
            if let Some(name) = attr_name {
                if name == "onClick" {
                    has_onclick = true;
                }
                if name == "onKeyDown" || name == "onKeyUp" {
                    has_key_event = true;
                }
            }
        }
    }

    // Skip interactive elements and custom components
    if let Some(ref name) = tag_name
        && (INTERACTIVE_TAGS.contains(&name.as_str()) || is_custom_component(name))
    {
        return;
    }

    // Skip managed children of a composite widget (keyboard handled by container)
    if is_managed_child(composite, jsx_opening_role(node, source).as_deref()) {
        return;
    }

    if has_onclick && !has_key_event {
        diagnostics.push(make_diagnostic(node));
    }
}

fn check_jsx_opening(
    node: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    composite: Option<&[&str]>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            let mut tag_name: Option<String> = None;
            let mut has_onclick = false;
            let mut has_key_event = false;

            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "identifier" {
                    tag_name = Some(source[inner_child.byte_range()].to_string());
                }
                if inner_child.kind() == "jsx_attribute" {
                    let attr_name = extract_jsx_attr_name(&inner_child, source);
                    if let Some(name) = attr_name {
                        if name == "onClick" {
                            has_onclick = true;
                        }
                        if name == "onKeyDown" || name == "onKeyUp" {
                            has_key_event = true;
                        }
                    }
                }
            }

            // Skip interactive elements and custom components
            if let Some(ref name) = tag_name
                && (INTERACTIVE_TAGS.contains(&name.as_str()) || is_custom_component(name))
            {
                return;
            }

            // Skip managed children of a composite widget
            if is_managed_child(composite, jsx_opening_role(&child, source).as_deref()) {
                return;
            }

            if has_onclick && !has_key_event {
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

/// The lowercased `role` attribute value declared directly on a JSX
/// opening/self-closing element node, if present.
fn jsx_opening_role(opening: &Node, source: &str) -> Option<String> {
    let mut cursor = opening.walk();
    for child in opening.children(&mut cursor) {
        if child.kind() == "jsx_attribute" {
            let mut found = false;
            let mut attr_cursor = child.walk();
            for attr_child in child.children(&mut attr_cursor) {
                if attr_child.kind() == "property_identifier"
                    && &source[attr_child.byte_range()] == "role"
                {
                    found = true;
                }
                if found && attr_child.kind() == "string" {
                    let raw = &source[attr_child.byte_range()];
                    return Some(
                        raw.trim_matches('"')
                            .trim_matches('\'')
                            .trim()
                            .to_ascii_lowercase(),
                    );
                }
            }
        }
    }
    None
}

/// The lowercased `role` of a `jsx_element`, read from its opening element.
fn jsx_element_role(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            return jsx_opening_role(&child, source);
        }
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
        let rule = ClickEvents;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = ClickEvents;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_div_with_onclick_no_key_event_fails() {
        let diags = check_html(r#"<div onclick="handler()"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String(
                "click-events-have-key-events".to_string()
            ))
        );
    }

    #[test]
    fn test_div_with_onclick_and_onkeydown_passes() {
        let diags = check_html(r#"<div onclick="handler()" onkeydown="handler()"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_div_with_onclick_and_onkeyup_passes() {
        let diags = check_html(r#"<div onclick="handler()" onkeyup="handler()"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_button_with_onclick_passes() {
        let diags = check_html(r#"<button onclick="handler()"></button>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_anchor_with_onclick_passes() {
        let diags = check_html(r#"<a onclick="handler()" href="/">Link</a>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_input_with_onclick_passes() {
        let diags = check_html(r#"<input onclick="handler()">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_onclick_no_diagnostic() {
        let diags = check_html(r#"<div class="foo"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_div_with_onclick_no_key_fails() {
        let diags = check_tsx(r#"const App = () => <div onClick={handler} />;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_div_with_onclick_and_onkeydown_passes() {
        let diags =
            check_tsx(r#"const App = () => <div onClick={handler} onKeyDown={handler} />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_button_with_onclick_passes() {
        let diags = check_tsx(r#"const App = () => <button onClick={handler} />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_div_element_with_onclick_no_key_fails() {
        let diags = check_tsx(r#"const App = () => <div onClick={handler}>text</div>;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_div_element_with_onclick_and_onkeyup_passes() {
        let diags =
            check_tsx(r#"const App = () => <div onClick={handler} onKeyUp={handler}>text</div>;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_custom_component_with_onclick_passes() {
        let diags = check_tsx(r#"const App = () => <IconLabelButton onClick={handler} />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_custom_component_element_with_onclick_passes() {
        let diags = check_tsx(r#"const App = () => <MyButton onClick={handler}>Click</MyButton>;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_lowercase_div_with_onclick_still_fails() {
        let diags = check_tsx(r#"const App = () => <span onClick={handler} />;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_listbox_option_with_onclick_passes() {
        // role="option" inside role="listbox": keyboard handled by the container.
        let diags = check_tsx(
            r#"const App = () => <div role="listbox"><div role="option" onClick={handler}>x</div></div>;"#,
        );
        assert_eq!(diags.len(), 0, "managed option should be exempt, got: {diags:?}");
    }

    #[test]
    fn test_tsx_listbox_mapped_option_with_onclick_passes() {
        let diags = check_tsx(
            r#"const App = () => <div role="listbox">{items.map((p, i) => (<div role="option" key={i} onClick={f}>{p.label}</div>))}</div>;"#,
        );
        assert_eq!(diags.len(), 0, "mapped managed option should be exempt, got: {diags:?}");
    }

    #[test]
    fn test_tsx_option_without_listbox_still_fails() {
        // role="option" not inside a composite container: not exempt.
        let diags =
            check_tsx(r#"const App = () => <div role="option" onClick={handler}>x</div>;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_listbox_non_option_child_with_onclick_still_fails() {
        // A plain div (no managed role) inside a listbox is not exempt.
        let diags = check_tsx(
            r#"const App = () => <div role="listbox"><div onClick={handler}>x</div></div>;"#,
        );
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_html_listbox_option_with_onclick_passes() {
        let diags = check_html(
            r#"<div role="listbox"><div role="option" onclick="handler()">x</div></div>"#,
        );
        assert_eq!(diags.len(), 0);
    }
}
