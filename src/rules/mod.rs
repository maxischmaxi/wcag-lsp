use crate::parser::FileType;
use tower_lsp_server::ls_types::Diagnostic;
use tree_sitter::Node;

pub mod anchor_content;
pub mod click_events;
pub mod form_label;
pub mod heading_order;
pub mod html_lang;
pub mod img_alt;

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
        Box::new(click_events::ClickEvents),
        Box::new(form_label::FormLabel),
        Box::new(heading_order::HeadingOrder),
        Box::new(html_lang::HtmlLang),
        Box::new(img_alt::ImgAlt),
    ]
}
