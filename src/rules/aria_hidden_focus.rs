use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct AriaHiddenFocus;

static METADATA: RuleMetadata = RuleMetadata {
    id: "aria-hidden-focus",
    description: "Elements with aria-hidden=\"true\" must not contain focusable elements",
    wcag_level: WcagLevel::A,
    wcag_criterion: "4.1.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html",
    default_severity: Severity::Error,
};

/// Natively focusable HTML tags (some require additional conditions).
const FOCUSABLE_TAGS: &[&str] = &["button", "select", "textarea", "iframe"];

/// In JSX, components starting with an uppercase letter are custom React components.
fn is_custom_component(name: &str) -> bool {
    name.starts_with(char::is_uppercase)
}

impl Rule for AriaHiddenFocus {
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

/// Get the value of an HTML attribute by name (case-insensitive) from an element node.
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

/// Determine whether an HTML element is focusable.
fn is_html_focusable(element: &Node, source: &str) -> bool {
    let tag_name = match get_html_tag_name(element, source) {
        Some(name) => name.to_ascii_lowercase(),
        None => return false,
    };

    // 1. Natively focusable tags (button, select, textarea, iframe)
    if FOCUSABLE_TAGS
        .iter()
        .any(|t| t.eq_ignore_ascii_case(&tag_name))
    {
        return true;
    }

    // 2. <a> with href attribute
    if tag_name == "a" && has_html_attr(element, source, "href") {
        return true;
    }

    // 3. <input> that is NOT type="hidden"
    if tag_name == "input" {
        let type_val = get_html_attr_value(element, source, "type");
        if let Some(ref val) = type_val
            && val.eq_ignore_ascii_case("hidden")
        {
            return false;
        }
        return true;
    }

    // 4. Has tabindex attribute with value NOT "-1"
    if has_html_attr(element, source, "tabindex") {
        let val = get_html_attr_value(element, source, "tabindex");
        match val {
            Some(v) if v.trim() == "-1" => return false,
            _ => return true,
        }
    }

    false
}

// ---------------------------------------------------------------------------
// HTML visitor
// ---------------------------------------------------------------------------

fn visit_html(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "element" {
        // Check if this element has aria-hidden="true"
        if let Some(val) = get_html_attr_value(node, source, "aria-hidden")
            && val == "true"
        {
            // Check all descendants for focusable elements
            check_html_descendants_for_focusable(node, source, diagnostics);
            return;
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diagnostics);
    }
}

/// Recursively check descendants of an aria-hidden="true" element for focusable elements.
/// Stops recursing into elements with aria-hidden="false" (which overrides the parent).
fn check_html_descendants_for_focusable(
    node: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "element" {
            // Check if this child has aria-hidden="false" (overrides parent)
            if let Some(val) = get_html_attr_value(&child, source, "aria-hidden")
                && val == "false"
            {
                // Skip this subtree entirely
                continue;
            }

            // Check if this child is focusable
            if is_html_focusable(&child, source) {
                diagnostics.push(make_diagnostic(&child));
            }

            // Continue checking deeper descendants
            check_html_descendants_for_focusable(&child, source, diagnostics);
        } else {
            check_html_descendants_for_focusable(&child, source, diagnostics);
        }
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

/// Determine whether a JSX element is focusable based on the opening/self-closing element node.
fn is_jsx_focusable(opening: &Node, source: &str) -> bool {
    let tag_name = match get_jsx_tag_name_from_opening(opening, source) {
        Some(name) => name,
        None => return false,
    };

    // Skip custom components
    if is_custom_component(tag_name) {
        return false;
    }

    // 1. Natively focusable tags
    if FOCUSABLE_TAGS.contains(&tag_name) {
        return true;
    }

    // 2. <a> with href attribute
    if tag_name == "a" && has_jsx_attr(opening, source, "href") {
        return true;
    }

    // 3. <input> that is NOT type="hidden"
    if tag_name == "input" {
        let type_val = get_jsx_attr_value(opening, source, "type");
        if let Some(ref val) = type_val
            && val == "hidden"
        {
            return false;
        }
        return true;
    }

    // 4. Has tabindex/tabIndex with value NOT "-1"
    let has_tabindex =
        has_jsx_attr(opening, source, "tabindex") || has_jsx_attr(opening, source, "tabIndex");
    if has_tabindex {
        let val = get_jsx_attr_value(opening, source, "tabindex")
            .or_else(|| get_jsx_attr_value(opening, source, "tabIndex"));
        match val {
            Some(v) if v.trim() == "-1" => return false,
            _ => return true,
        }
    }

    false
}

// ---------------------------------------------------------------------------
// JSX visitor
// ---------------------------------------------------------------------------

fn visit_jsx(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    match node.kind() {
        "jsx_element" => {
            // Find the jsx_opening_element
            let mut opening: Option<Node> = None;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "jsx_opening_element" {
                    opening = Some(child);
                    break;
                }
            }

            if let Some(ref op) = opening {
                // Check if this element has aria-hidden="true"
                if let Some(val) = get_jsx_attr_value(op, source, "aria-hidden")
                    && val == "true"
                {
                    check_jsx_descendants_for_focusable(node, source, diagnostics);
                    return;
                }
            }

            // Normal recursion
            let mut cursor2 = node.walk();
            for child in node.children(&mut cursor2) {
                visit_jsx(&child, source, diagnostics);
            }
        }
        "jsx_self_closing_element" => {
            // Self-closing elements with aria-hidden="true" have no children,
            // so nothing to check. Just return.
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                visit_jsx(&child, source, diagnostics);
            }
        }
    }
}

/// Recursively check descendants of an aria-hidden="true" JSX element for focusable elements.
fn check_jsx_descendants_for_focusable(
    node: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "jsx_element" => {
                // Find the opening element
                let mut opening: Option<Node> = None;
                let mut inner_cursor = child.walk();
                for inner_child in child.children(&mut inner_cursor) {
                    if inner_child.kind() == "jsx_opening_element" {
                        opening = Some(inner_child);
                        break;
                    }
                }

                if let Some(ref op) = opening {
                    // Check for aria-hidden="false" override
                    if let Some(val) = get_jsx_attr_value(op, source, "aria-hidden")
                        && val == "false"
                    {
                        continue;
                    }

                    // Check if focusable
                    if is_jsx_focusable(op, source) {
                        diagnostics.push(make_diagnostic(&child));
                    }
                }

                // Continue checking deeper descendants
                check_jsx_descendants_for_focusable(&child, source, diagnostics);
            }
            "jsx_self_closing_element" => {
                // Check for aria-hidden="false" override
                if let Some(val) = get_jsx_attr_value(&child, source, "aria-hidden")
                    && val == "false"
                {
                    continue;
                }

                // Check if focusable
                if is_jsx_focusable(&child, source) {
                    diagnostics.push(make_diagnostic(&child));
                }
            }
            _ => {
                check_jsx_descendants_for_focusable(&child, source, diagnostics);
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
        let rule = AriaHiddenFocus;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AriaHiddenFocus;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_aria_hidden_with_button_fails() {
        let diags = check_html(r#"<div aria-hidden="true"><button>Click</button></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-hidden-focus".to_string()))
        );
    }

    #[test]
    fn test_aria_hidden_with_anchor_fails() {
        let diags = check_html(r#"<div aria-hidden="true"><a href="/">link</a></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-hidden-focus".to_string()))
        );
    }

    #[test]
    fn test_aria_hidden_with_text_input_fails() {
        let diags = check_html(r#"<div aria-hidden="true"><input type="text"></div>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_aria_hidden_with_hidden_input_passes() {
        let diags = check_html(r#"<div aria-hidden="true"><input type="hidden"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_aria_hidden_with_tabindex_zero_fails() {
        let diags = check_html(r#"<div aria-hidden="true"><div tabindex="0">text</div></div>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_aria_hidden_with_tabindex_negative_passes() {
        let diags = check_html(r#"<div aria-hidden="true"><div tabindex="-1">text</div></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_aria_hidden_with_span_passes() {
        let diags = check_html(r#"<div aria-hidden="true"><span>text</span></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_aria_hidden_false_passes() {
        let diags = check_html(r#"<div aria-hidden="false"><button>Click</button></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_aria_hidden_passes() {
        let diags = check_html(r#"<div><button>Click</button></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_aria_hidden_with_button_fails() {
        let diags =
            check_tsx(r#"const App = () => <div aria-hidden="true"><button>Click</button></div>;"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-hidden-focus".to_string()))
        );
    }

    #[test]
    fn test_tsx_aria_hidden_with_span_passes() {
        let diags =
            check_tsx(r#"const App = () => <div aria-hidden="true"><span>text</span></div>;"#);
        assert_eq!(diags.len(), 0);
    }
}
