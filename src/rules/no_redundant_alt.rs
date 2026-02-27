use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct NoRedundantAlt;

static METADATA: RuleMetadata = RuleMetadata {
    id: "no-redundant-alt",
    description: "Image alt text should not contain redundant words",
    wcag_level: WcagLevel::A,
    wcag_criterion: "1.1.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/non-text-content.html",
    default_severity: Severity::Warning,
};

/// Words that are redundant in alt text because screen readers already
/// announce the element as an image.
const REDUNDANT_WORDS: &[&str] = &["image", "picture", "photo", "graphic", "icon"];

impl Rule for NoRedundantAlt {
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
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "start_tag" {
                check_html_start_tag(&child, source, diagnostics, node);
            }
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diagnostics);
    }
}

fn check_html_start_tag(
    start_tag: &Node,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
    element_node: &Node,
) {
    let mut is_img = false;
    let mut alt_value: Option<String> = None;

    let mut cursor = start_tag.walk();
    for child in start_tag.children(&mut cursor) {
        if child.kind() == "tag_name" {
            let name = &source[child.byte_range()];
            if name.eq_ignore_ascii_case("img") {
                is_img = true;
            }
        }
        if child.kind() == "attribute" {
            let (attr_name, attr_value) = extract_html_attribute(&child, source);
            if let Some(name) = attr_name
                && name.eq_ignore_ascii_case("alt")
            {
                alt_value = attr_value;
            }
        }
    }

    if is_img
        && let Some(ref alt) = alt_value
        && !alt.is_empty()
        && contains_redundant_word(alt)
    {
        diagnostics.push(make_diagnostic(element_node));
    }
    // No alt attribute or empty alt → handled by img-alt rule, not this one
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
            if value.is_none() {
                value = Some(String::new());
            }
        }
    }

    (name, value)
}

// ---------------------------------------------------------------------------
// JSX / TSX
// ---------------------------------------------------------------------------

fn visit_jsx(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "jsx_self_closing_element" {
        check_jsx_self_closing(node, source, diagnostics);
    } else if node.kind() == "jsx_element" {
        check_jsx_opening(node, source, diagnostics);
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_jsx(&child, source, diagnostics);
    }
}

fn check_jsx_self_closing(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut is_img = false;
    let mut alt_value: Option<String> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = &source[child.byte_range()];
            if name == "img" {
                is_img = true;
            }
        }
        if child.kind() == "jsx_attribute" {
            let (attr_name, attr_value) = extract_jsx_attribute(&child, source);
            if let Some(name) = attr_name
                && name == "alt"
            {
                alt_value = attr_value;
            }
        }
    }

    if is_img
        && let Some(ref alt) = alt_value
        && !alt.is_empty()
        && contains_redundant_word(alt)
    {
        diagnostics.push(make_diagnostic(node));
    }
}

fn check_jsx_opening(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            let mut is_img = false;
            let mut alt_value: Option<String> = None;

            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "identifier" {
                    let name = &source[inner_child.byte_range()];
                    if name == "img" {
                        is_img = true;
                    }
                }
                if inner_child.kind() == "jsx_attribute" {
                    let (attr_name, attr_value) = extract_jsx_attribute(&inner_child, source);
                    if let Some(name) = attr_name
                        && name == "alt"
                    {
                        alt_value = attr_value;
                    }
                }
            }

            if is_img
                && let Some(ref alt) = alt_value
                && !alt.is_empty()
                && contains_redundant_word(alt)
            {
                diagnostics.push(make_diagnostic(node));
            }
        }
    }
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
            let raw = &source[child.byte_range()];
            let trimmed = raw.trim_matches('"').trim_matches('\'');
            value = Some(trimmed.to_string());
        }
    }

    (name, value)
}

/// Check if the alt text contains any of the redundant words (case-insensitive).
/// The check is done on word boundaries — e.g., "image" in "image of a cat" matches,
/// but also "Photo" in "Photo of sunset" matches.
fn contains_redundant_word(alt: &str) -> bool {
    let alt_lower = alt.to_lowercase();
    for word in REDUNDANT_WORDS {
        // Split on non-alphabetic characters and check each word
        for token in alt_lower.split(|c: char| !c.is_alphabetic()) {
            if token == *word {
                return true;
            }
        }
    }
    false
}

fn make_diagnostic(node: &Node) -> Diagnostic {
    let meta = &METADATA;
    Diagnostic {
        range: node_to_range(node),
        severity: Some(DiagnosticSeverity::WARNING),
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
        let rule = NoRedundantAlt;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = NoRedundantAlt;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_img_alt_with_image_word_fails() {
        let diags = check_html(r#"<img alt="image of a cat" src="cat.jpg">"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("no-redundant-alt".to_string()))
        );
    }

    #[test]
    fn test_img_alt_without_redundant_words_passes() {
        let diags = check_html(r#"<img alt="a fluffy cat" src="cat.jpg">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_img_alt_with_photo_word_fails() {
        let diags = check_html(r#"<img alt="Photo of sunset" src="sun.jpg">"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_img_no_alt_passes() {
        // No alt attribute is handled by the img-alt rule, not this one
        let diags = check_html(r#"<img src="cat.jpg">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_img_empty_alt_passes() {
        // Decorative image with empty alt
        let diags = check_html(r#"<img alt="" src="spacer.gif">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_img_alt_with_graphic_fails() {
        let diags = check_html(r#"<img alt="graphic of a chart" src="chart.jpg">"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_img_alt_with_icon_fails() {
        let diags = check_html(r#"<img alt="icon for settings" src="settings.png">"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_img_alt_with_image_word_fails() {
        let diags = check_tsx(r#"const App = () => <img alt="image of a cat" src="cat.jpg" />;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_img_alt_without_redundant_words_passes() {
        let diags = check_tsx(r#"const App = () => <img alt="a fluffy cat" src="cat.jpg" />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_img_passes() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }
}
