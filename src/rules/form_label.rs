use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use std::collections::HashSet;
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
/// Note: `id` is intentionally excluded — an `id` alone is not a label.
/// A matching `<label for="…">` is required for id-based association.
const LABEL_ATTRS_HTML: &[&str] = &["aria-label", "aria-labelledby", "title"];

/// JSX attribute names that count as a label association (id excluded).
const LABEL_ATTRS_JSX: &[&str] = &[
    "aria-label",
    "aria-labelledby",
    "ariaLabel",
    "ariaLabelledby",
    "title",
];

/// Collected `for` / `htmlFor` values from `<label>` elements.
struct LabelForValues {
    /// String literal values, e.g. from `for="name"` or `htmlFor="name"` or `htmlFor={"name"}`.
    literals: HashSet<String>,
    /// Expression texts (identifiers / member expressions), e.g. from `htmlFor={inputId}`.
    expressions: HashSet<String>,
}

impl LabelForValues {
    fn new() -> Self {
        Self {
            literals: HashSet::new(),
            expressions: HashSet::new(),
        }
    }
}

impl Rule for FormLabel {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(&self, root: &Node, source: &str, file_type: FileType) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        if file_type.is_jsx_like() {
            let label_fors = collect_jsx_label_for_values(root, source);
            visit_jsx(root, source, &mut diagnostics, &label_fors);
        } else {
            let label_fors = collect_html_label_for_values(root, source);
            visit_html(root, source, &mut diagnostics, &label_fors);
        }
        diagnostics
    }
}

// ---------------------------------------------------------------------------
// HTML — Pass 1: Collect <label for="…"> values
// ---------------------------------------------------------------------------

fn collect_html_label_for_values(root: &Node, source: &str) -> LabelForValues {
    let mut values = LabelForValues::new();
    collect_html_labels(root, source, &mut values);
    values
}

fn collect_html_labels(node: &Node, source: &str, values: &mut LabelForValues) {
    if node.kind() == "element" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "start_tag" {
                let mut is_label = false;
                let mut for_value = None;

                let mut tag_cursor = child.walk();
                for tag_child in child.children(&mut tag_cursor) {
                    if tag_child.kind() == "tag_name" {
                        let name = &source[tag_child.byte_range()];
                        if name.eq_ignore_ascii_case("label") {
                            is_label = true;
                        }
                    }
                    if tag_child.kind() == "attribute" {
                        let (attr_name, attr_val) = extract_html_attribute(&tag_child, source);
                        if let Some(ref name) = attr_name
                            && name.eq_ignore_ascii_case("for")
                        {
                            for_value = attr_val;
                        }
                    }
                }

                if is_label && let Some(val) = for_value {
                    values.literals.insert(val);
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_html_labels(&child, source, values);
    }
}

// ---------------------------------------------------------------------------
// HTML — Pass 2: Check form elements
// ---------------------------------------------------------------------------

fn visit_html(node: &Node, source: &str, diags: &mut Vec<Diagnostic>, label_fors: &LabelForValues) {
    if node.kind() == "element" {
        check_html_element(node, source, diags, label_fors);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diags, label_fors);
    }
}

fn check_html_element(
    element: &Node,
    source: &str,
    diags: &mut Vec<Diagnostic>,
    label_fors: &LabelForValues,
) {
    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" {
            check_html_start_tag(&child, source, diags, element, label_fors);
        }
    }
}

fn check_html_start_tag(
    start_tag: &Node,
    source: &str,
    diags: &mut Vec<Diagnostic>,
    element_node: &Node,
    label_fors: &LabelForValues,
) {
    let mut is_form_element = false;
    let mut is_hidden = false;
    let mut has_label = false;
    let mut id_value: Option<String> = None;

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
            if let Some(ref name) = attr_name {
                if LABEL_ATTRS_HTML
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(name))
                {
                    has_label = true;
                }
                if name.eq_ignore_ascii_case("type")
                    && let Some(ref val) = attr_value
                    && val.eq_ignore_ascii_case("hidden")
                {
                    is_hidden = true;
                }
                if name.eq_ignore_ascii_case("id") {
                    id_value = attr_value;
                }
            }
        }
    }

    // Check if the element is wrapped in a <label>
    if is_form_element && !has_label && is_inside_label(element_node, source) {
        has_label = true;
    }

    // Check if there is a <label for="…"> matching this element's id
    if is_form_element
        && !has_label
        && let Some(ref id) = id_value
        && label_fors.literals.contains(id)
    {
        has_label = true;
    }

    if is_form_element && !is_hidden && !has_label {
        diags.push(make_diagnostic(element_node));
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
// JSX / TSX — Pass 1: Collect <label htmlFor="…"> values
// ---------------------------------------------------------------------------

fn collect_jsx_label_for_values(root: &Node, source: &str) -> LabelForValues {
    let mut values = LabelForValues::new();
    collect_jsx_labels(root, source, &mut values);
    values
}

fn collect_jsx_labels(node: &Node, source: &str, values: &mut LabelForValues) {
    match node.kind() {
        "jsx_element" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "jsx_opening_element"
                    && jsx_tag_name(&child, source).as_deref() == Some("label")
                {
                    collect_htmlfor_attrs(&child, source, values);
                }
            }
        }
        "jsx_self_closing_element" => {
            if jsx_tag_name(node, source).as_deref() == Some("label") {
                collect_htmlfor_attrs(node, source, values);
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_jsx_labels(&child, source, values);
    }
}

/// Extract htmlFor values from the attributes of a JSX element.
fn collect_htmlfor_attrs(node: &Node, source: &str, values: &mut LabelForValues) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_attribute"
            && jsx_attr_name(&child, source).as_deref() == Some("htmlFor")
        {
            collect_jsx_attr_value(&child, source, values);
        }
    }
}

/// Extract the value of a JSX attribute into a LabelForValues.
fn collect_jsx_attr_value(attr_node: &Node, source: &str, values: &mut LabelForValues) {
    let mut cursor = attr_node.walk();
    for child in attr_node.children(&mut cursor) {
        if child.kind() == "string" {
            let raw = &source[child.byte_range()];
            let trimmed = raw.trim_matches('"').trim_matches('\'');
            values.literals.insert(trimmed.to_string());
        }
        if child.kind() == "jsx_expression" {
            let mut expr_cursor = child.walk();
            for expr_child in child.children(&mut expr_cursor) {
                match expr_child.kind() {
                    "string" => {
                        let raw = &source[expr_child.byte_range()];
                        let trimmed = raw.trim_matches('"').trim_matches('\'');
                        values.literals.insert(trimmed.to_string());
                    }
                    "identifier" | "member_expression" => {
                        values
                            .expressions
                            .insert(source[expr_child.byte_range()].to_string());
                    }
                    _ => {}
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// JSX / TSX — Pass 2: Check form elements
// ---------------------------------------------------------------------------

fn visit_jsx(node: &Node, source: &str, diags: &mut Vec<Diagnostic>, label_fors: &LabelForValues) {
    match node.kind() {
        "jsx_self_closing_element" => {
            check_jsx_self_closing(node, source, diags, label_fors);
        }
        "jsx_element" => {
            check_jsx_element(node, source, diags, label_fors);
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_jsx(&child, source, diags, label_fors);
    }
}

fn check_jsx_self_closing(
    node: &Node,
    source: &str,
    diags: &mut Vec<Diagnostic>,
    label_fors: &LabelForValues,
) {
    let tag = jsx_tag_name(node, source);
    if !tag.as_deref().is_some_and(|t| FORM_TAGS.contains(&t)) {
        return;
    }

    let mut is_hidden = false;
    let mut has_label = false;
    let mut id_literal: Option<String> = None;
    let mut id_expression: Option<String> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_attribute" {
            let attr_name = jsx_attr_name(&child, source);
            if let Some(ref name) = attr_name {
                if LABEL_ATTRS_JSX.contains(&name.as_str()) {
                    has_label = true;
                }
                if name == "type"
                    && jsx_attr_string_value(&child, source).as_deref() == Some("hidden")
                {
                    is_hidden = true;
                }
                if name == "id" {
                    let (lit, expr) = jsx_attr_id_value(&child, source);
                    id_literal = lit;
                    id_expression = expr;
                }
            }
        }
    }

    if !has_label && is_inside_jsx_label(node, source) {
        has_label = true;
    }

    if !has_label {
        has_label = id_matches_label(id_literal.as_deref(), id_expression.as_deref(), label_fors);
    }

    if !is_hidden && !has_label {
        diags.push(make_diagnostic(node));
    }
}

fn check_jsx_element(
    node: &Node,
    source: &str,
    diags: &mut Vec<Diagnostic>,
    label_fors: &LabelForValues,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            let tag = jsx_tag_name(&child, source);
            if !tag.as_deref().is_some_and(|t| FORM_TAGS.contains(&t)) {
                return;
            }

            let mut is_hidden = false;
            let mut has_label = false;
            let mut id_literal: Option<String> = None;
            let mut id_expression: Option<String> = None;

            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "jsx_attribute" {
                    let attr_name = jsx_attr_name(&inner_child, source);
                    if let Some(ref name) = attr_name {
                        if LABEL_ATTRS_JSX.contains(&name.as_str()) {
                            has_label = true;
                        }
                        if name == "type"
                            && jsx_attr_string_value(&inner_child, source).as_deref()
                                == Some("hidden")
                        {
                            is_hidden = true;
                        }
                        if name == "id" {
                            let (lit, expr) = jsx_attr_id_value(&inner_child, source);
                            id_literal = lit;
                            id_expression = expr;
                        }
                    }
                }
            }

            if !has_label && is_inside_jsx_label(node, source) {
                has_label = true;
            }

            if !has_label {
                has_label =
                    id_matches_label(id_literal.as_deref(), id_expression.as_deref(), label_fors);
            }

            if !is_hidden && !has_label {
                diags.push(make_diagnostic(node));
            }
        }
    }
}

/// Check if a form element's id matches any collected label `for`/`htmlFor` value.
fn id_matches_label(
    id_literal: Option<&str>,
    id_expression: Option<&str>,
    label_fors: &LabelForValues,
) -> bool {
    if let Some(lit) = id_literal
        && label_fors.literals.contains(lit)
    {
        return true;
    }
    if let Some(expr) = id_expression
        && label_fors.expressions.contains(expr)
    {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// JSX helpers
// ---------------------------------------------------------------------------

/// Get the tag name from a JSX opening or self-closing element.
fn jsx_tag_name(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            return Some(source[child.byte_range()].to_string());
        }
    }
    None
}

/// Get the attribute name from a jsx_attribute node.
fn jsx_attr_name(attr_node: &Node, source: &str) -> Option<String> {
    let mut cursor = attr_node.walk();
    for child in attr_node.children(&mut cursor) {
        if child.kind() == "property_identifier" {
            return Some(source[child.byte_range()].to_string());
        }
    }
    None
}

/// Get a string literal value from a jsx_attribute (ignores expressions).
fn jsx_attr_string_value(attr_node: &Node, source: &str) -> Option<String> {
    let mut cursor = attr_node.walk();
    for child in attr_node.children(&mut cursor) {
        if child.kind() == "string" {
            let raw = &source[child.byte_range()];
            return Some(raw.trim_matches('"').trim_matches('\'').to_string());
        }
    }
    None
}

/// Extract the id value from a JSX attribute as (literal, expression).
///
/// - `id="myId"` → `(Some("myId"), None)`
/// - `id={"myId"}` → `(Some("myId"), None)`
/// - `id={myVar}` → `(None, Some("myVar"))`
/// - `id={props.id}` → `(None, Some("props.id"))`
fn jsx_attr_id_value(attr_node: &Node, source: &str) -> (Option<String>, Option<String>) {
    let mut literal = None;
    let mut expression = None;

    let mut cursor = attr_node.walk();
    for child in attr_node.children(&mut cursor) {
        if child.kind() == "string" {
            let raw = &source[child.byte_range()];
            literal = Some(raw.trim_matches('"').trim_matches('\'').to_string());
        }
        if child.kind() == "jsx_expression" {
            let mut expr_cursor = child.walk();
            for expr_child in child.children(&mut expr_cursor) {
                match expr_child.kind() {
                    "string" => {
                        let raw = &source[expr_child.byte_range()];
                        literal = Some(raw.trim_matches('"').trim_matches('\'').to_string());
                    }
                    "identifier" | "member_expression" => {
                        expression = Some(source[expr_child.byte_range()].to_string());
                    }
                    _ => {}
                }
            }
        }
    }

    (literal, expression)
}

/// Walk up ancestors to see if this JSX element is inside a <label>.
fn is_inside_jsx_label(node: &Node, source: &str) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "jsx_element" {
            let mut cursor = parent.walk();
            for child in parent.children(&mut cursor) {
                if child.kind() == "jsx_opening_element"
                    && jsx_tag_name(&child, source).as_deref() == Some("label")
                {
                    return true;
                }
            }
        }
        current = parent.parent();
    }
    false
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
        let rule = FormLabel;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = FormLabel;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    // -----------------------------------------------------------------------
    // HTML
    // -----------------------------------------------------------------------

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
    fn test_input_with_id_alone_fails() {
        // An id alone is NOT a label — a matching <label for="…"> is required.
        let diags = check_html(r#"<input type="text" id="name">"#);
        assert_eq!(diags.len(), 1);
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
    fn test_label_for_matches_input_id() {
        let diags = check_html(r#"<label for="name">Name</label><input type="text" id="name">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_label_for_no_match() {
        let diags = check_html(r#"<label for="email">Email</label><input type="text" id="name">"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_label_after_input() {
        let diags = check_html(r#"<input type="text" id="name"><label for="name">Name</label>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_label_for_select() {
        let diags = check_html(r#"<label for="color">Color</label><select id="color"></select>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_label_for_textarea() {
        let diags = check_html(r#"<label for="bio">Bio</label><textarea id="bio"></textarea>"#);
        assert_eq!(diags.len(), 0);
    }

    // -----------------------------------------------------------------------
    // JSX / TSX
    // -----------------------------------------------------------------------

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

    #[test]
    fn test_tsx_input_with_id_alone_fails() {
        let diags = check_tsx(r#"const App = () => <input type="text" id="name" />;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_label_htmlfor_string_matches() {
        let diags = check_tsx(
            r#"const App = () => <><label htmlFor="name">Name</label><input type="text" id="name" /></>;"#,
        );
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_label_htmlfor_no_match() {
        let diags = check_tsx(
            r#"const App = () => <><label htmlFor="email">Email</label><input type="text" id="name" /></>;"#,
        );
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_label_htmlfor_variable_matches() {
        let diags = check_tsx(
            r#"const App = () => { const id = "x"; return <><label htmlFor={id}>Name</label><input type="text" id={id} /></>; };"#,
        );
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_label_htmlfor_different_variable_fails() {
        let diags = check_tsx(
            r#"const App = () => { const a = "x"; const b = "y"; return <><label htmlFor={a}>Name</label><input type="text" id={b} /></>; };"#,
        );
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_label_htmlfor_member_expression_matches() {
        let diags = check_tsx(
            r#"const App = () => <><label htmlFor={props.id}>Name</label><input type="text" id={props.id} /></>;"#,
        );
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_label_htmlfor_expression_string_matches() {
        // htmlFor={"name"} should match id="name"
        let diags = check_tsx(
            r#"const App = () => <><label htmlFor={"name"}>Name</label><input type="text" id="name" /></>;"#,
        );
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_label_after_input() {
        let diags = check_tsx(
            r#"const App = () => <><input type="text" id="name" /><label htmlFor="name">Name</label></>;"#,
        );
        assert_eq!(diags.len(), 0);
    }
}
