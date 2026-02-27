use crate::parser::FileType;
use tower_lsp_server::ls_types::Diagnostic;
use tree_sitter::Node;

pub mod anchor_content;
pub mod area_alt;
pub mod aria_props;
pub mod aria_required_attr;
pub mod aria_role;
pub mod autocomplete_valid;
pub mod button_name;
pub mod click_events;
pub mod form_label;
pub mod heading_content;
pub mod heading_order;
pub mod html_lang;
pub mod iframe_title;
pub mod img_alt;
pub mod input_image_alt;
pub mod lang_valid;
pub mod list_structure;
pub mod media_captions;
pub mod meta_refresh;
pub mod mouse_events;
pub mod no_access_key;
pub mod no_autoplay;
pub mod no_distracting_elements;
pub mod no_duplicate_id;
pub mod no_redundant_alt;
pub mod no_redundant_roles;
pub mod object_alt;
pub mod page_title;
pub mod scope_attr;
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
        Box::new(area_alt::AreaAlt),
        Box::new(aria_props::AriaProps),
        Box::new(aria_required_attr::AriaRequiredAttr),
        Box::new(aria_role::AriaRole),
        Box::new(autocomplete_valid::AutocompleteValid),
        Box::new(button_name::ButtonName),
        Box::new(click_events::ClickEvents),
        Box::new(form_label::FormLabel),
        Box::new(heading_content::HeadingContent),
        Box::new(heading_order::HeadingOrder),
        Box::new(html_lang::HtmlLang),
        Box::new(iframe_title::IframeTitle),
        Box::new(img_alt::ImgAlt),
        Box::new(input_image_alt::InputImageAlt),
        Box::new(lang_valid::LangValid),
        Box::new(list_structure::ListStructure),
        Box::new(media_captions::MediaCaptions),
        Box::new(meta_refresh::MetaRefresh),
        Box::new(mouse_events::MouseEvents),
        Box::new(no_access_key::NoAccessKey),
        Box::new(no_autoplay::NoAutoplay),
        Box::new(no_distracting_elements::NoDistractingElements),
        Box::new(no_duplicate_id::NoDuplicateId),
        Box::new(no_redundant_alt::NoRedundantAlt),
        Box::new(no_redundant_roles::NoRedundantRoles),
        Box::new(object_alt::ObjectAlt),
        Box::new(page_title::PageTitle),
        Box::new(scope_attr::ScopeAttr),
        Box::new(tabindex::Tabindex),
        Box::new(table_header::TableHeader),
    ]
}
