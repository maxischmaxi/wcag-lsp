use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use std::collections::HashSet;
use std::sync::LazyLock;
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct AutocompleteValid;

static METADATA: RuleMetadata = RuleMetadata {
    id: "autocomplete-valid",
    description: "autocomplete attribute must have a valid value",
    wcag_level: WcagLevel::AA,
    wcag_criterion: "1.3.5",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/identify-input-purpose.html",
    default_severity: Severity::Warning,
};

static VALID_AUTOCOMPLETE_TOKENS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let tokens = [
        "off",
        "on",
        "name",
        "honorific-prefix",
        "given-name",
        "additional-name",
        "family-name",
        "honorific-suffix",
        "nickname",
        "email",
        "username",
        "new-password",
        "current-password",
        "one-time-code",
        "organization-title",
        "organization",
        "street-address",
        "address-line1",
        "address-line2",
        "address-line3",
        "address-level4",
        "address-level3",
        "address-level2",
        "address-level1",
        "country",
        "country-name",
        "postal-code",
        "cc-name",
        "cc-given-name",
        "cc-additional-name",
        "cc-family-name",
        "cc-number",
        "cc-exp",
        "cc-exp-month",
        "cc-exp-year",
        "cc-csc",
        "cc-type",
        "transaction-currency",
        "transaction-amount",
        "language",
        "bday",
        "bday-day",
        "bday-month",
        "bday-year",
        "sex",
        "tel",
        "tel-country-code",
        "tel-national",
        "tel-area-code",
        "tel-local",
        "tel-extension",
        "impp",
        "url",
        "photo",
    ];
    tokens.into_iter().collect()
});

impl Rule for AutocompleteValid {
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
    if node.kind() == "attribute" {
        check_html_attribute(node, source, diagnostics);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diagnostics);
    }
}

fn check_html_attribute(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut is_autocomplete = false;
    let mut value: Option<(String, Node)> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute_name" {
            let name = &source[child.byte_range()];
            if name.eq_ignore_ascii_case("autocomplete") {
                is_autocomplete = true;
            }
        }
        if child.kind() == "quoted_attribute_value" {
            let mut val_cursor = child.walk();
            for val_child in child.children(&mut val_cursor) {
                if val_child.kind() == "attribute_value" {
                    value = Some((source[val_child.byte_range()].to_string(), val_child));
                }
            }
        }
    }

    if is_autocomplete && let Some((val, val_node)) = value {
        check_autocomplete_value(&val, &val_node, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// JSX / TSX
// ---------------------------------------------------------------------------

fn visit_jsx(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "jsx_attribute" {
        check_jsx_attribute(node, source, diagnostics);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_jsx(&child, source, diagnostics);
    }
}

fn check_jsx_attribute(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut is_autocomplete = false;
    let mut value: Option<(String, Node)> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "property_identifier" {
            let name = &source[child.byte_range()];
            if name == "autoComplete" || name == "autocomplete" {
                is_autocomplete = true;
            }
        }
        if child.kind() == "string" {
            let raw = &source[child.byte_range()];
            let trimmed = raw.trim_matches('"').trim_matches('\'');
            value = Some((trimmed.to_string(), child));
        }
    }

    if is_autocomplete && let Some((val, val_node)) = value {
        check_autocomplete_value(&val, &val_node, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

fn check_autocomplete_value(value: &str, node: &Node, diagnostics: &mut Vec<Diagnostic>) {
    // Autocomplete values can have optional section- and shipping/billing prefixes.
    // Validate the last token of space-separated values against the set.
    let last_token = value.split_whitespace().last().unwrap_or("");
    if last_token.is_empty() {
        return;
    }
    let lower = last_token.to_ascii_lowercase();
    if !VALID_AUTOCOMPLETE_TOKENS.contains(lower.as_str()) {
        diagnostics.push(make_diagnostic(node, value));
    }
}

fn make_diagnostic(node: &Node, invalid_value: &str) -> Diagnostic {
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
            "Invalid autocomplete value '{}'. {} [WCAG {} Level {:?}]",
            invalid_value, meta.description, meta.wcag_criterion, meta.wcag_level
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
        let rule = AutocompleteValid;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AutocompleteValid;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_valid_autocomplete_name_passes() {
        let diags = check_html(r#"<input autocomplete="name">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_invalid_autocomplete_value_fails() {
        let diags = check_html(r#"<input autocomplete="invalid-value">"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("autocomplete-valid".to_string()))
        );
    }

    #[test]
    fn test_shipping_prefix_with_valid_token_passes() {
        let diags = check_html(r#"<input autocomplete="shipping name">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_autocomplete_passes() {
        let diags = check_html(r#"<input type="text">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_valid_autocomplete_email_passes() {
        let diags = check_html(r#"<input autocomplete="email">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_valid_autocomplete_passes() {
        let diags = check_tsx(r#"const App = () => <input autoComplete="name" />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_invalid_autocomplete_fails() {
        let diags = check_tsx(r#"const App = () => <input autoComplete="invalid-value" />;"#);
        assert_eq!(diags.len(), 1);
    }
}
