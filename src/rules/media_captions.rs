use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::html_attrs;
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
    let is_media = html_attrs::element_tag_name(element, source)
        .is_some_and(|n| n.eq_ignore_ascii_case("video") || n.eq_ignore_ascii_case("audio"));

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
    let tag = match html_attrs::element_tag(element) {
        Some(t) => t,
        None => return false,
    };

    let is_track =
        html_attrs::tag_name(&tag, source).is_some_and(|n| n.eq_ignore_ascii_case("track"));
    if !is_track {
        return false;
    }

    html_attrs::attrs(&tag, source).iter().any(|attr| {
        // A bound `:kind` is a runtime expression — skip the value check.
        attr.name_eq("kind")
            && !attr.bound
            && attr.value.as_deref().is_some_and(|val| {
                val.eq_ignore_ascii_case("captions") || val.eq_ignore_ascii_case("subtitles")
            })
    })
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

    fn check_vue(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Vue).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = MediaCaptions;
        rule.check(&tree.root_node(), source, FileType::Vue)
    }

    #[test]
    fn test_vue_static_captions_track_passes() {
        let diags = check_vue(
            r#"<template><video src="movie.mp4"><track kind="captions" src="caps.vtt" /></video></template>"#,
        );
        assert_eq!(diags.len(), 0, "static captions track satisfies the rule, got: {diags:?}");
    }

    #[test]
    fn test_vue_video_without_captions_fails() {
        let diags = check_vue(r#"<template><video src="movie.mp4"></video></template>"#);
        assert_eq!(diags.len(), 1);
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
