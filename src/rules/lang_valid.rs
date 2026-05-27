use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::html_attrs;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use std::collections::HashSet;
use std::sync::LazyLock;
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct LangValid;

static METADATA: RuleMetadata = RuleMetadata {
    id: "lang-valid",
    description: "lang attribute must have a valid BCP 47 primary language subtag",
    wcag_level: WcagLevel::A,
    wcag_criterion: "3.1.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/language-of-page.html",
    default_severity: Severity::Error,
};

static VALID_LANG_SUBTAGS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let subtags = [
        "aa", "ab", "af", "ak", "am", "an", "ar", "as", "av", "ay", "az", "ba", "be", "bg", "bh",
        "bi", "bm", "bn", "bo", "br", "bs", "ca", "ce", "ch", "co", "cr", "cs", "cu", "cv", "cy",
        "da", "de", "dv", "dz", "ee", "el", "en", "eo", "es", "et", "eu", "fa", "ff", "fi", "fj",
        "fo", "fr", "fy", "ga", "gd", "gl", "gn", "gu", "gv", "ha", "he", "hi", "ho", "hr", "ht",
        "hu", "hy", "hz", "ia", "id", "ie", "ig", "ii", "ik", "in", "io", "is", "it", "iu", "ja",
        "jv", "ka", "kg", "ki", "kj", "kk", "kl", "km", "kn", "ko", "kr", "ks", "ku", "kv", "kw",
        "ky", "la", "lb", "lg", "li", "ln", "lo", "lt", "lu", "lv", "mg", "mh", "mi", "mk", "ml",
        "mn", "mo", "mr", "ms", "mt", "my", "na", "nb", "nd", "ne", "ng", "nl", "nn", "no", "nr",
        "nv", "ny", "oc", "oj", "om", "or", "os", "pa", "pi", "pl", "ps", "pt", "qu", "rm", "rn",
        "ro", "ru", "rw", "sa", "sc", "sd", "se", "sg", "sh", "si", "sk", "sl", "sm", "sn", "so",
        "sq", "sr", "ss", "st", "su", "sv", "sw", "ta", "te", "tg", "th", "ti", "tk", "tl", "tn",
        "to", "tr", "ts", "tt", "tw", "ty", "ug", "uk", "ur", "uz", "ve", "vi", "vo", "wa", "wo",
        "xh", "yi", "yo", "za", "zh", "zu",
    ];
    subtags.into_iter().collect()
});

impl Rule for LangValid {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(&self, root: &Node, source: &str, file_type: FileType) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        if file_type.is_jsx_like() {
            return diagnostics;
        }
        visit_html(root, source, &mut diagnostics);
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
    let attr = match html_attrs::attr_from_node(node, source) {
        Some(a) => a,
        None => return,
    };

    if !attr.name_eq("lang") {
        return;
    }

    // A bound `:lang="x"` is a runtime expression — can't validate it.
    if attr.bound {
        return;
    }

    if let Some(val) = attr.value {
        // Report against the value node when available, else the attribute node.
        let val_node = value_node(node).unwrap_or(*node);
        check_lang_value(&val, &val_node, diagnostics);
    }
}

/// The `attribute_value` node inside an `attribute`, for precise diagnostics.
fn value_node<'a>(attr_node: &Node<'a>) -> Option<Node<'a>> {
    let mut cursor = attr_node.walk();
    for child in attr_node.children(&mut cursor) {
        if child.kind() == "quoted_attribute_value" {
            let mut vc = child.walk();
            for v in child.children(&mut vc) {
                if v.kind() == "attribute_value" {
                    return Some(v);
                }
            }
        }
        if child.kind() == "attribute_value" {
            return Some(child);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

fn check_lang_value(value: &str, node: &Node, diagnostics: &mut Vec<Diagnostic>) {
    let primary = value.split('-').next().unwrap_or("");
    let primary_lower = primary.to_ascii_lowercase();
    if !VALID_LANG_SUBTAGS.contains(primary_lower.as_str()) {
        diagnostics.push(make_diagnostic(node, value));
    }
}

fn make_diagnostic(node: &Node, invalid_lang: &str) -> Diagnostic {
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
            "Invalid language subtag '{}'. {} [WCAG {} Level {:?}]",
            invalid_lang, meta.description, meta.wcag_criterion, meta.wcag_level
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
        let rule = LangValid;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = LangValid;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    fn check_vue(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Vue).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = LangValid;
        rule.check(&tree.root_node(), source, FileType::Vue)
    }

    #[test]
    fn test_vue_bound_lang_skipped() {
        // Dynamic value: cannot validate the expression literally.
        let diags = check_vue(r#"<template><div :lang="locale"></div></template>"#);
        assert_eq!(diags.len(), 0, "bound :lang should be skipped, got: {diags:?}");
    }

    #[test]
    fn test_vue_static_invalid_lang_still_fails() {
        let diags = check_vue(r#"<template><div lang="xyz"></div></template>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_valid_lang_en_passes() {
        let diags = check_html(r#"<html lang="en"><body></body></html>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_valid_lang_en_us_passes() {
        let diags = check_html(r#"<html lang="en-US"><body></body></html>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_invalid_lang_xyz_fails() {
        let diags = check_html(r#"<html lang="xyz"><body></body></html>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("lang-valid".to_string()))
        );
    }

    #[test]
    fn test_invalid_lang_123_fails() {
        let diags = check_html(r#"<html lang="123"><body></body></html>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_no_lang_attribute_passes() {
        let diags = check_html(r#"<html><body></body></html>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_jsx_returns_empty() {
        let diags = check_tsx(r#"const App = () => <div lang="xyz" />;"#);
        assert_eq!(diags.len(), 0);
    }
}
