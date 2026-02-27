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

impl Rule for ClickEvents {
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
    let mut tag_name: Option<String> = None;
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
            }
        }
    }

    // Skip interactive elements that natively handle keyboard
    if let Some(ref name) = tag_name
        && INTERACTIVE_TAGS.iter().any(|t| t == name)
    {
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

    // Skip interactive elements
    if let Some(ref name) = tag_name
        && INTERACTIVE_TAGS.contains(&name.as_str())
    {
        return;
    }

    if has_onclick && !has_key_event {
        diagnostics.push(make_diagnostic(node));
    }
}

fn check_jsx_opening(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
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

            // Skip interactive elements
            if let Some(ref name) = tag_name
                && INTERACTIVE_TAGS.contains(&name.as_str())
            {
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
}
