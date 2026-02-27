use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct MediaCaptions;

static METADATA: RuleMetadata = RuleMetadata {
    id: "media-captions",
    description: "Media elements must have captions",
    wcag_level: WcagLevel::A,
    wcag_criterion: "1.2.2",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/captions-prerecorded.html",
    default_severity: Severity::Warning,
};

impl Rule for MediaCaptions {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(&self, root: &Node, source: &str, file_type: FileType) -> Vec<Diagnostic> {
        // Primarily HTML-only
        if file_type.is_jsx_like() {
            return Vec::new();
        }

        let mut diagnostics = Vec::new();
        visit_html(root, source, &mut diagnostics);
        diagnostics
    }
}

fn visit_html(node: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "element" {
        check_html_element(node, source, diagnostics);
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_html(&child, source, diagnostics);
    }
}

fn check_html_element(element: &Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut is_media = false;

    // Check the start_tag for tag name
    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" {
            let mut tag_cursor = child.walk();
            for tag_child in child.children(&mut tag_cursor) {
                if tag_child.kind() == "tag_name" {
                    let name = &source[tag_child.byte_range()];
                    if name.eq_ignore_ascii_case("video") || name.eq_ignore_ascii_case("audio") {
                        is_media = true;
                    }
                }
            }
        }
    }

    if !is_media {
        return;
    }

    // Check if any descendant is a <track kind="captions"> or <track kind="subtitles">
    if has_caption_track(element, source) {
        return;
    }

    diagnostics.push(make_diagnostic(element));
}

/// Recursively check whether the element contains a <track kind="captions"> or
/// <track kind="subtitles"> descendant.
fn has_caption_track(node: &Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "element" {
            // Check if this child element is a track with the right kind
            if is_caption_track_element(&child, source) {
                return true;
            }
            // Also recurse in case track is nested deeper
            if has_caption_track(&child, source) {
                return true;
            }
        }
    }
    false
}

/// Check if an element node is a <track kind="captions"> or <track kind="subtitles">
fn is_caption_track_element(element: &Node, source: &str) -> bool {
    let mut cursor = element.walk();
    for child in element.children(&mut cursor) {
        if child.kind() == "start_tag" {
            let mut is_track = false;
            let mut has_caption_kind = false;

            let mut tag_cursor = child.walk();
            for tag_child in child.children(&mut tag_cursor) {
                if tag_child.kind() == "tag_name" {
                    let name = &source[tag_child.byte_range()];
                    if name.eq_ignore_ascii_case("track") {
                        is_track = true;
                    }
                }
                if tag_child.kind() == "attribute" {
                    let (attr_name, attr_value) = extract_html_attribute(&tag_child, source);
                    if let Some(name) = attr_name {
                        if name.eq_ignore_ascii_case("kind") {
                            if let Some(ref val) = attr_value {
                                if val.eq_ignore_ascii_case("captions")
                                    || val.eq_ignore_ascii_case("subtitles")
                                {
                                    has_caption_kind = true;
                                }
                            }
                        }
                    }
                }
            }

            if is_track && has_caption_kind {
                return true;
            }
        }
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
            if value.is_none() {
                value = Some(String::new());
            }
        }
    }

    (name, value)
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
        let rule = MediaCaptions;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    #[test]
    fn test_video_without_captions_fails() {
        let diags = check_html(r#"<video src="movie.mp4"></video>"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("media-captions".to_string()))
        );
    }

    #[test]
    fn test_video_with_captions_passes() {
        let diags =
            check_html(r#"<video src="movie.mp4"><track kind="captions" src="caps.vtt"></video>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_audio_without_captions_fails() {
        let diags = check_html(r#"<audio src="song.mp3"></audio>"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_video_with_subtitles_passes() {
        let diags =
            check_html(r#"<video src="movie.mp4"><track kind="subtitles" src="subs.vtt"></video>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_media_passes() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_video_with_wrong_track_kind_fails() {
        let diags = check_html(
            r#"<video src="movie.mp4"><track kind="descriptions" src="desc.vtt"></video>"#,
        );
        assert_eq!(diags.len(), 1);
    }
}
