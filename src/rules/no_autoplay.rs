use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct NoAutoplay;

static METADATA: RuleMetadata = RuleMetadata {
    id: "no-autoplay",
    description: "<audio> and <video> elements must not autoplay without muted",
    wcag_level: WcagLevel::A,
    wcag_criterion: "1.4.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/audio-control.html",
    default_severity: Severity::Warning,
};

impl Rule for NoAutoplay {
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
    let mut is_media = false;
    let mut has_autoplay = false;
    let mut has_muted = false;

    let mut cursor = start_tag.walk();
    for child in start_tag.children(&mut cursor) {
        if child.kind() == "tag_name" {
            let name = &source[child.byte_range()];
            if name.eq_ignore_ascii_case("audio") || name.eq_ignore_ascii_case("video") {
                is_media = true;
            }
        }
        if child.kind() == "attribute" {
            let attr_name = extract_html_attr_name(&child, source);
            if let Some(name) = attr_name {
                if name.eq_ignore_ascii_case("autoplay") {
                    has_autoplay = true;
                }
                if name.eq_ignore_ascii_case("muted") {
                    has_muted = true;
                }
            }
        }
    }

    if is_media && has_autoplay && !has_muted {
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
    let mut is_media = false;
    let mut has_autoplay = false;
    let mut has_muted = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = &source[child.byte_range()];
            if name == "audio" || name == "video" {
                is_media = true;
            }
        }
        if child.kind() == "jsx_attribute" {
            let attr_name = extract_jsx_attr_name(&child, source);
            if let Some(name) = attr_name {
                if name == "autoPlay" || name == "autoplay" {
                    has_autoplay = true;
                }
                if name == "muted" {
                    has_muted = true;
                }
            }
        }
    }

    if is_media && has_autoplay && !has_muted {
        diagnostics.push(make_diagnostic(node));
    }
}

fn check_jsx_element(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "jsx_opening_element" {
            let mut is_media = false;
            let mut has_autoplay = false;
            let mut has_muted = false;

            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "identifier" {
                    let name = &source[inner_child.byte_range()];
                    if name == "audio" || name == "video" {
                        is_media = true;
                    }
                }
                if inner_child.kind() == "jsx_attribute" {
                    let attr_name = extract_jsx_attr_name(&inner_child, source);
                    if let Some(name) = attr_name {
                        if name == "autoPlay" || name == "autoplay" {
                            has_autoplay = true;
                        }
                        if name == "muted" {
                            has_muted = true;
                        }
                    }
                }
            }

            if is_media && has_autoplay && !has_muted {
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

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

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
        let rule = NoAutoplay;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = NoAutoplay;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_audio_with_autoplay_fails() {
        let diags = check_html(r#"<audio src="song.mp3" autoplay></audio>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("no-autoplay".to_string()))
        );
    }

    #[test]
    fn test_audio_with_autoplay_and_muted_passes() {
        let diags = check_html(r#"<audio src="song.mp3" autoplay muted></audio>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_video_with_autoplay_fails() {
        let diags = check_html(r#"<video src="movie.mp4" autoplay></video>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_video_without_autoplay_passes() {
        let diags = check_html(r#"<video src="movie.mp4"></video>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_video_with_autoplay_and_muted_passes() {
        let diags = check_html(r#"<video src="movie.mp4" autoplay muted></video>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_media_passes() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_audio_with_autoplay_fails() {
        let diags = check_tsx(r#"const App = () => <audio src="song.mp3" autoPlay />;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_audio_with_autoplay_and_muted_passes() {
        let diags = check_tsx(r#"const App = () => <audio src="song.mp3" autoPlay muted />;"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_tsx_video_with_autoplay_fails() {
        let diags =
            check_tsx(r#"const App = () => <video src="movie.mp4" autoPlay>content</video>;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_video_without_autoplay_passes() {
        let diags = check_tsx(r#"const App = () => <video src="movie.mp4">content</video>;"#);
        assert_eq!(diags.len(), 0);
    }
}
