use crate::parser::FileType;
use tower_lsp_server::ls_types::Diagnostic;
use tree_sitter::Node;

pub mod anchor_content;
pub mod aria_props;
pub mod aria_role;
pub mod click_events;
pub mod form_label;
pub mod heading_order;
pub mod html_lang;
pub mod iframe_title;
pub mod img_alt;
pub mod media_captions;
pub mod meta_refresh;
pub mod no_redundant_alt;
pub mod tabindex;
pub mod table_header;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WcagLevel {
    A,
    AA,
    AAA,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

pub struct RuleMetadata {
    pub id: &'static str,
    pub description: &'static str,
    pub wcag_level: WcagLevel,
    pub wcag_criterion: &'static str,
    pub wcag_url: &'static str,
    pub default_severity: Severity,
}

pub trait Rule: Send + Sync {
    fn metadata(&self) -> &RuleMetadata;
    fn check(&self, root: &Node, source: &str, file_type: FileType) -> Vec<Diagnostic>;
}

pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(anchor_content::AnchorContent),
        Box::new(aria_props::AriaProps),
        Box::new(aria_role::AriaRole),
        Box::new(click_events::ClickEvents),
        Box::new(form_label::FormLabel),
        Box::new(heading_order::HeadingOrder),
        Box::new(html_lang::HtmlLang),
        Box::new(iframe_title::IframeTitle),
        Box::new(img_alt::ImgAlt),
        Box::new(media_captions::MediaCaptions),
        Box::new(meta_refresh::MetaRefresh),
        Box::new(no_redundant_alt::NoRedundantAlt),
        Box::new(tabindex::Tabindex),
        Box::new(table_header::TableHeader),
    ]
}
