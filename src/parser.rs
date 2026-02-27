use tree_sitter::{Language, Parser};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
    Html,
    Jsx,
    Tsx,
    Vue,
    Svelte,
    Unknown,
}

impl FileType {
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "html" | "htm" => FileType::Html,
            "jsx" => FileType::Jsx,
            "tsx" => FileType::Tsx,
            "vue" => FileType::Vue,
            "svelte" => FileType::Svelte,
            "astro" | "php" | "erb" | "hbs" | "twig" => FileType::Html,
            _ => FileType::Unknown,
        }
    }

    pub fn from_uri(uri: &str) -> Self {
        uri.rsplit('.')
            .next()
            .map(Self::from_extension)
            .unwrap_or(FileType::Unknown)
    }

    pub fn tree_sitter_language(&self) -> Option<Language> {
        match self {
            FileType::Html => Some(tree_sitter_html::LANGUAGE.into()),
            FileType::Jsx => Some(tree_sitter_javascript::LANGUAGE.into()),
            FileType::Tsx => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
            FileType::Vue => Some(tree_sitter_html::LANGUAGE.into()),
            FileType::Svelte => Some(tree_sitter_html::LANGUAGE.into()),
            FileType::Unknown => None,
        }
    }

    pub fn is_jsx_like(&self) -> bool {
        matches!(self, FileType::Jsx | FileType::Tsx)
    }
}

pub fn create_parser(file_type: FileType) -> Option<Parser> {
    let language = file_type.tree_sitter_language()?;
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    Some(parser)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_type_from_extension() {
        assert_eq!(FileType::from_extension("html"), FileType::Html);
        assert_eq!(FileType::from_extension("htm"), FileType::Html);
        assert_eq!(FileType::from_extension("jsx"), FileType::Jsx);
        assert_eq!(FileType::from_extension("tsx"), FileType::Tsx);
        assert_eq!(FileType::from_extension("vue"), FileType::Vue);
        assert_eq!(FileType::from_extension("svelte"), FileType::Svelte);
        assert_eq!(FileType::from_extension("rs"), FileType::Unknown);
    }

    #[test]
    fn test_file_type_from_uri() {
        assert_eq!(FileType::from_uri("file:///app/index.html"), FileType::Html);
        assert_eq!(FileType::from_uri("file:///app/App.tsx"), FileType::Tsx);
        assert_eq!(FileType::from_uri("file:///app/style.css"), FileType::Unknown);
    }

    #[test]
    fn test_create_parser_html() {
        let parser = create_parser(FileType::Html);
        assert!(parser.is_some());
    }

    #[test]
    fn test_create_parser_tsx() {
        let parser = create_parser(FileType::Tsx);
        assert!(parser.is_some());
    }

    #[test]
    fn test_create_parser_unknown_returns_none() {
        let parser = create_parser(FileType::Unknown);
        assert!(parser.is_none());
    }

    #[test]
    fn test_parse_html() {
        let mut parser = create_parser(FileType::Html).unwrap();
        let tree = parser.parse("<img src=\"photo.jpg\">", None).unwrap();
        let root = tree.root_node();
        assert_eq!(root.kind(), "document");
        assert!(!root.has_error());
    }

    #[test]
    fn test_parse_tsx() {
        let mut parser = create_parser(FileType::Tsx).unwrap();
        let source = "const App = () => <img src=\"photo.jpg\" />;";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        assert_eq!(root.kind(), "program");
    }

    #[test]
    fn test_is_jsx_like() {
        assert!(FileType::Jsx.is_jsx_like());
        assert!(FileType::Tsx.is_jsx_like());
        assert!(!FileType::Html.is_jsx_like());
        assert!(!FileType::Vue.is_jsx_like());
    }
}
