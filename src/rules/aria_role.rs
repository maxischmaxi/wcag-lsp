use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use std::collections::HashSet;
use std::sync::LazyLock;
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct AriaRole;

static METADATA: RuleMetadata = RuleMetadata {
    id: "aria-role",
    description: "ARIA role must be a valid role value",
    wcag_level: WcagLevel::A,
    wcag_criterion: "4.1.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/name-role-value.html",
    default_severity: Severity::Error,
};

static VALID_ROLES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let roles = [
        "alert",
        "alertdialog",
        "application",
        "article",
        "banner",
        "blockquote",
        "button",
        "caption",
        "cell",
        "checkbox",
        "code",
        "columnheader",
        "combobox",
        "complementary",
        "contentinfo",
        "definition",
        "deletion",
        "dialog",
        "directory",
        "document",
        "emphasis",
        "feed",
        "figure",
        "form",
        "generic",
        "grid",
        "gridcell",
        "group",
        "heading",
        "img",
        "insertion",
        "link",
        "list",
        "listbox",
        "listitem",
        "log",
        "main",
        "marquee",
        "math",
        "menu",
        "menubar",
        "menuitem",
        "menuitemcheckbox",
        "menuitemradio",
        "meter",
        "navigation",
        "none",
        "note",
        "option",
        "paragraph",
        "presentation",
        "progressbar",
        "radio",
        "radiogroup",
        "region",
        "row",
        "rowgroup",
        "rowheader",
        "scrollbar",
        "search",
        "searchbox",
        "separator",
        "slider",
        "spinbutton",
        "status",
        "strong",
        "subscript",
        "superscript",
        "switch",
        "tab",
        "table",
        "tablist",
        "tabpanel",
        "term",
        "textbox",
        "time",
        "timer",
        "toolbar",
        "tooltip",
        "tree",
        "treegrid",
        "treeitem",
    ];
    roles.into_iter().collect()
});

impl Rule for AriaRole {
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
    let mut is_role = false;
    let mut value: Option<(String, Node)> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "attribute_name" {
            let name = &source[child.byte_range()];
            if name.eq_ignore_ascii_case("role") {
                is_role = true;
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

    if is_role {
        if let Some((val, val_node)) = value {
            check_role_value(&val, &val_node, diagnostics);
        }
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
    let mut is_role = false;
    let mut value: Option<(String, Node)> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "property_identifier" {
            let name = &source[child.byte_range()];
            if name == "role" {
                is_role = true;
            }
        }
        if child.kind() == "string" {
            let raw = &source[child.byte_range()];
            let trimmed = raw.trim_matches('"').trim_matches('\'');
            value = Some((trimmed.to_string(), child));
        }
    }

    if is_role {
        if let Some((val, val_node)) = value {
            check_role_value(&val, &val_node, diagnostics);
        }
    }
}

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

fn check_role_value(value: &str, node: &Node, diagnostics: &mut Vec<Diagnostic>) {
    // Roles can be space-separated
    for role in value.split_whitespace() {
        if !VALID_ROLES.contains(role) {
            diagnostics.push(make_diagnostic(node, role));
        }
    }
}

fn make_diagnostic(node: &Node, invalid_role: &str) -> Diagnostic {
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
            "Invalid ARIA role '{}'. {} [WCAG {} Level {:?}]",
            invalid_role, meta.description, meta.wcag_criterion, meta.wcag_level
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
        let rule = AriaRole;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = AriaRole;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_valid_role_passes() {
        let diags = check_html(r#"<div role="button"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_invalid_role_fails() {
        let diags = check_html(r#"<div role="invalid"></div>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("aria-role".to_string()))
        );
    }

    #[test]
    fn test_multiple_valid_roles_passes() {
        let diags = check_html(r#"<div role="button link"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_multiple_roles_one_invalid_fails() {
        let diags = check_html(r#"<div role="button foobar"></div>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_no_role_attribute_passes() {
        let diags = check_html(r#"<div class="container"></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_valid_role_passes() {
        let diags = check_tsx(r#"const App = () => <div role="button" />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_invalid_role_fails() {
        let diags = check_tsx(r#"const App = () => <div role="invalid" />;"#);
        assert_eq!(diags.len(), 1);
    }
}
