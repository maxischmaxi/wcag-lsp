use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct FormLabel;

static METADATA: RuleMetadata = RuleMetadata {
    id: "form-label",
    description: "Form elements must have associated labels",
    wcag_level: WcagLevel::A,
    wcag_criterion: "1.3.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/info-and-relationships.html",
    default_severity: Severity::Error,
};

/// Tag names that require a label.
const FORM_TAGS: &[&str] = &["input", "select", "textarea"];

/// HTML attribute names that count as a label association.
const LABEL_ATTRS_HTML: &[&str] = &["aria-label", "aria-labelledby", "id", "title"];

/// JSX attribute names that count as a label association.
const LABEL_ATTRS_JSX: &[&str] = &[
    "aria-label",
    "aria-labelledby",
    "ariaLabel",
    "ariaLabelledby",
    "id",
    "title",
];

impl Rule for FormLabel {
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
        if child.kind() == "start_tag" {
            check_html_start_tag(&child, source, diagnostics, element);
        }
    }
}

fn check_html_start_tag(
    start_tag: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    element_node: &Node,
) {
    let mut is_form_element = false;
    let mut is_hidden = false;
    let mut has_label = false;

    let mut cursor = start_tag.walk();
    for child in start_tag.children(&mut cursor) {
        if child.kind() == "tag_name" {
            let name = &source[child.byte_range()];
            if FORM_TAGS.iter().any(|t| t.eq_ignore_ascii_case(name)) {
                is_form_element = true;
            }
        }
        if child.kind() == "attribute" {
            let (attr_name, attr_value) = extract_html_attribute(&child, source);
            if let Some(name) = attr_name {
                if LABEL_ATTRS_HTML
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(&name))
                {
                    has_label = true;
                }
                if name.eq_ignore_ascii_case("type")
                    && let Some(val) = &attr_value
                    && val.eq_ignore_ascii_case("hidden")
                {
                    is_hidden = true;
                }
            }
        }
    }

    // Check if the element is wrapped in a <label>
    if is_form_element && !has_label && is_inside_label(element_node, source) {
        has_label = true;
    }

    if is_form_element && !is_hidden && !has_label {
        diagnostics.push(make_diagnostic(element_node));
    }
}

/// Walk up ancestors to see if this element is inside a <label>.
fn is_inside_label(node: &Node, source: &str) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "element" {
            let mut cursor = parent.walk();
            for child in parent.children(&mut cursor) {
                if child.kind() == "start_tag" {
                    let mut tag_cursor = child.walk();
                    for tag_child in child.children(&mut tag_cursor) {
                        if tag_child.kind() == "tag_name" {
                            let name = &source[tag_child.byte_range()];
                            if name.eq_ignore_ascii_case("label") {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        current = parent.parent();
    }
    false
}

/// Extract (attribute_name, Option<attribute_value>) from an HTML attribute node.
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
    let mut is_form_element = false;
    let mut is_hidden = false;
    let mut has_label = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = &source[child.byte_range()];
            if FORM_TAGS.contains(&name) {
                is_form_element = true;
            }
        }
        if child.kind() == "jsx_attribute" {
            let (attr_name, attr_value) = extract_jsx_attribute(&child, source);
            if let Some(name) = attr_name {
                if LABEL_ATTRS_JSX.iter().any(|a| *a == name) {
                    has_label = true;
                }
                if name == "type"
                    && let Some(val) = &attr_value
                    && val == "hidden"
                {
                    is_hidden = true;
                }
            }
        }
    }

    // Check if inside a JSX <label> element
    if is_form_element && !has_label && is_inside_jsx_label(node, source) {
        has_label = true;
    }

    if is_form_element && !is_hidden && !has_label {
        diagnostics.push(make_diagnostic(node));
    }
}

fn check_jsx_element(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    // A jsx_element has an opening_element child; check if it is a form element
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            let mut is_form_element = false;
            let mut is_hidden = false;
            let mut has_label = false;

            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "identifier" {
                    let name = &source[inner_child.byte_range()];
                    if FORM_TAGS.contains(&name) {
                        is_form_element = true;
                    }
                }
                if inner_child.kind() == "jsx_attribute" {
                    let (attr_name, attr_value) = extract_jsx_attribute(&inner_child, source);
                    if let Some(name) = attr_name {
                        if LABEL_ATTRS_JSX.iter().any(|a| *a == name) {
                            has_label = true;
                        }
                        if name == "type"
                            && let Some(val) = &attr_value
                            && val == "hidden"
                        {
                            is_hidden = true;
                        }
                    }
                }
            }

            // Check if inside a JSX <label> element
            if is_form_element && !has_label && is_inside_jsx_label(node, source) {
                has_label = true;
            }

            if is_form_element && !is_hidden && !has_label {
                diagnostics.push(make_diagnostic(node));
            }
        }
    }
}

/// Walk up ancestors to see if this JSX element is inside a <label>.
fn is_inside_jsx_label(node: &Node, source: &str) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "jsx_element" {
            let mut cursor = parent.walk();
            for child in parent.children(&mut cursor) {
                if child.kind() == "jsx_opening_element" {
                    let mut inner_cursor = child.walk();
                    for inner_child in child.children(&mut inner_cursor) {
                        if inner_child.kind() == "identifier" {
                            let name = &source[inner_child.byte_range()];
                            if name == "label" {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        current = parent.parent();
    }
    false
}

/// Extract (attribute_name, Option<string_value>) from a JSX attribute node.
fn extract_jsx_attribute(attr_node: &Node, source: &str) -> (Option<String>, Option<String>) {
    let mut name = None;
    let mut value = None;

    let mut cursor = attr_node.walk();
    for child in attr_node.children(&mut cursor) {
        if child.kind() == "property_identifier" {
            name = Some(source[child.byte_range()].to_string());
        }
        if child.kind() == "string" {
            // JSX string values include the quotes; strip them
            let raw = &source[child.byte_range()];
            let trimmed = raw.trim_matches('"').trim_matches('\'');
            value = Some(trimmed.to_string());
        }
    }

    (name, value)
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
        let rule = FormLabel;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = FormLabel;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_input_without_label_fails() {
        let diags = check_html(r#"<input type="text">"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("form-label".to_string()))
        );
    }

    #[test]
    fn test_input_with_aria_label_passes() {
        let diags = check_html(r#"<input type="text" aria-label="Name">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_hidden_input_passes() {
        let diags = check_html(r#"<input type="hidden">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_select_without_label_fails() {
        let diags = check_html(r#"<select></select>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_textarea_without_label_fails() {
        let diags = check_html(r#"<textarea></textarea>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_input_with_id_passes() {
        let diags = check_html(r#"<input type="text" id="name">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_input_wrapped_in_label_passes() {
        let diags = check_html(r#"<label><input type="text"></label>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_input_with_title_passes() {
        let diags = check_html(r#"<input type="text" title="Enter name">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_input_with_aria_labelledby_passes() {
        let diags = check_html(r#"<input type="text" aria-labelledby="lbl">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_input_without_label_fails() {
        let diags = check_tsx(r#"const App = () => <input type="text" />;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_input_with_aria_label_passes() {
        let diags = check_tsx(r#"const App = () => <input type="text" ariaLabel="Name" />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_hidden_input_passes() {
        let diags = check_tsx(r#"const App = () => <input type="hidden" />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_input_wrapped_in_label_passes() {
        let diags = check_tsx(r#"const App = () => <label><input type="text" /></label>;"#);
        assert_eq!(diags.len(), 0);
    }
}
