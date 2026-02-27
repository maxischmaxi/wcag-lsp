use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct MouseEvents;

static METADATA: RuleMetadata = RuleMetadata {
    id: "mouse-events-have-key-events",
    description: "Mouse event handlers must have corresponding keyboard event handlers",
    wcag_level: WcagLevel::A,
    wcag_criterion: "2.1.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/keyboard.html",
    default_severity: Severity::Error,
};

/// In JSX, components starting with an uppercase letter are custom React components.
/// They handle their own keyboard accessibility internally, so we skip them.
fn is_custom_component(name: &str) -> bool {
    name.starts_with(char::is_uppercase)
}

impl Rule for MouseEvents {
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
    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" || child.kind() == "self_closing_tag" {
            check_html_tag(&child, source, diagnostics, element);
        }
    }
}

fn check_html_tag(
    tag: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    element_node: &Node,
) {
    let mut has_mouseover = false;
    let mut has_mouseout = false;
    let mut has_focus = false;
    let mut has_blur = false;

    let mut cursor = tag.walk();
    for child in tag.children(&mut cursor) {
        if child.kind() == "attribute" {
            let attr_name = extract_html_attr_name(&child, source);
            if let Some(name) = attr_name {
                let lower = name.to_ascii_lowercase();
                if lower == "onmouseover" {
                    has_mouseover = true;
                }
                if lower == "onmouseout" {
                    has_mouseout = true;
                }
                if lower == "onfocus" {
                    has_focus = true;
                }
                if lower == "onblur" {
                    has_blur = true;
                }
            }
        }
    }

    if has_mouseover && !has_focus {
        diagnostics.push(make_diagnostic(element_node, "onMouseOver", "onFocus"));
    }
    if has_mouseout && !has_blur {
        diagnostics.push(make_diagnostic(element_node, "onMouseOut", "onBlur"));
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
            check_jsx_opening(node, source, diagnostics);
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_jsx(&child, source, diagnostics);
    }
}

fn check_jsx_self_closing(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut tag_name: Option<String> = None;
    let mut has_mouseover = false;
    let mut has_mouseout = false;
    let mut has_focus = false;
    let mut has_blur = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            tag_name = Some(source[child.byte_range()].to_string());
        }
        if child.kind() == "jsx_attribute" {
            let attr_name = extract_jsx_attr_name(&child, source);
            if let Some(name) = attr_name {
                if name == "onMouseOver" {
                    has_mouseover = true;
                }
                if name == "onMouseOut" {
                    has_mouseout = true;
                }
                if name == "onFocus" {
                    has_focus = true;
                }
                if name == "onBlur" {
                    has_blur = true;
                }
            }
        }
    }

    // Skip custom React components
    if let Some(ref name) = tag_name
        && is_custom_component(name)
    {
        return;
    }

    if has_mouseover && !has_focus {
        diagnostics.push(make_diagnostic(node, "onMouseOver", "onFocus"));
    }
    if has_mouseout && !has_blur {
        diagnostics.push(make_diagnostic(node, "onMouseOut", "onBlur"));
    }
}

fn check_jsx_opening(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            let mut tag_name: Option<String> = None;
            let mut has_mouseover = false;
            let mut has_mouseout = false;
            let mut has_focus = false;
            let mut has_blur = false;

            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "identifier" {
                    tag_name = Some(source[inner_child.byte_range()].to_string());
                }
                if inner_child.kind() == "jsx_attribute" {
                    let attr_name = extract_jsx_attr_name(&inner_child, source);
                    if let Some(name) = attr_name {
                        if name == "onMouseOver" {
                            has_mouseover = true;
                        }
                        if name == "onMouseOut" {
                            has_mouseout = true;
                        }
                        if name == "onFocus" {
                            has_focus = true;
                        }
                        if name == "onBlur" {
                            has_blur = true;
                        }
                    }
                }
            }

            // Skip custom React components
            if let Some(ref name) = tag_name
                && is_custom_component(name)
            {
                return;
            }

            if has_mouseover && !has_focus {
                diagnostics.push(make_diagnostic(node, "onMouseOver", "onFocus"));
            }
            if has_mouseout && !has_blur {
                diagnostics.push(make_diagnostic(node, "onMouseOut", "onBlur"));
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

fn make_diagnostic(node: &Node, mouse_event: &str, keyboard_event: &str) -> Diagnostic {
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
            "{} requires {}. {} [WCAG {} Level {:?}]",
            mouse_event, keyboard_event, meta.description, meta.wcag_criterion, meta.wcag_level
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
        let rule = MouseEvents;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = MouseEvents;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_mouseover_without_focus_fails() {
        let diags = check_html(r#"<div onmouseover="handler()"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String(
                "mouse-events-have-key-events".to_string()
            ))
        );
    }

    #[test]
    fn test_mouseover_with_focus_passes() {
        let diags = check_html(r#"<div onmouseover="handler()" onfocus="handler()"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_mouseout_without_blur_fails() {
        let diags = check_html(r#"<div onmouseout="handler()"></div>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_mouseout_with_blur_passes() {
        let diags = check_html(r#"<div onmouseout="handler()" onblur="handler()"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_mouse_events_passes() {
        let diags = check_html(r#"<div class="foo"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_both_mouse_events_without_keyboard_fails_twice() {
        let diags = check_html(r#"<div onmouseover="handler()" onmouseout="handler()"></div>"#);
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn test_tsx_mouseover_without_focus_fails() {
        let diags = check_tsx(r#"const App = () => <div onMouseOver={handler} />;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_mouseover_with_focus_passes() {
        let diags =
            check_tsx(r#"const App = () => <div onMouseOver={handler} onFocus={handler} />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_mouseout_without_blur_fails() {
        let diags = check_tsx(r#"const App = () => <div onMouseOut={handler} />;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_mouseout_with_blur_passes() {
        let diags =
            check_tsx(r#"const App = () => <div onMouseOut={handler} onBlur={handler} />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_element_mouseover_without_focus_fails() {
        let diags = check_tsx(r#"const App = () => <div onMouseOver={handler}>text</div>;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_element_mouseover_with_focus_passes() {
        let diags = check_tsx(
            r#"const App = () => <div onMouseOver={handler} onFocus={handler}>text</div>;"#,
        );
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_custom_component_with_mouseover_passes() {
        let diags = check_tsx(r#"const App = () => <Tooltip onMouseOver={handler} />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_custom_component_element_with_mouseout_passes() {
        let diags =
            check_tsx(r#"const App = () => <HoverCard onMouseOut={handler}>text</HoverCard>;"#);
        assert_eq!(diags.len(), 0);
    }
}
