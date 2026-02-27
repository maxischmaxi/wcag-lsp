use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct NoDistractingElements;

static METADATA: RuleMetadata = RuleMetadata {
    id: "no-distracting-elements",
    description: "<blink> and <marquee> elements must not be used",
    wcag_level: WcagLevel::A,
    wcag_criterion: "2.2.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/pause-stop-hide.html",
    default_severity: Severity::Error,
};

impl Rule for NoDistractingElements {
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
    let mut cursor = start_tag.walk();
    for child in start_tag.children(&mut cursor) {
        if child.kind() == "tag_name" {
            let name = &source[child.byte_range()];
            if name.eq_ignore_ascii_case("blink") || name.eq_ignore_ascii_case("marquee") {
                diagnostics.push(make_diagnostic(element_node));
            }
        }
    }
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

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_jsx(&child, source, diagnostics);
    }
}

fn check_jsx_self_closing(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = &source[child.byte_range()];
            if name == "blink" || name == "marquee" {
                diagnostics.push(make_diagnostic(node));
            }
        }
    }
}

fn check_jsx_element(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "identifier" {
                    let name = &source[inner_child.byte_range()];
                    if name == "blink" || name == "marquee" {
                        diagnostics.push(make_diagnostic(node));
                    }
                }
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
        let rule = NoDistractingElements;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = NoDistractingElements;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_blink_fails() {
        let diags = check_html(r#"<blink>Text</blink>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String(
                "no-distracting-elements".to_string()
            ))
        );
    }

    #[test]
    fn test_marquee_fails() {
        let diags = check_html(r#"<marquee>Scrolling text</marquee>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_div_passes() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_blink_fails() {
        let diags = check_tsx(r#"const App = () => <blink>Text</blink>;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_marquee_fails() {
        let diags = check_tsx(r#"const App = () => <marquee>Scrolling</marquee>;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_div_passes() {
        let diags = check_tsx(r#"const App = () => <div>Hello</div>;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_self_closing_blink_fails() {
        let diags = check_tsx(r#"const App = () => <blink />;"#);
        assert_eq!(diags.len(), 1);
    }
}
