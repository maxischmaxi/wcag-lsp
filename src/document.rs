use crate::parser::{self, FileType};
use std::collections::HashMap;
use tree_sitter::{Parser, Tree};

#[derive(Debug)]
pub struct Document {
    pub uri: String,
    pub file_type: FileType,
    pub source: String,
    pub tree: Tree,
    pub version: i32,
}

#[derive(Default)]
pub struct DocumentManager {
    documents: HashMap<String, Document>,
    parsers: HashMap<FileType, Parser>,
}

impl std::fmt::Debug for DocumentManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentManager")
            .field("documents", &self.documents)
            .field("parsers", &format!("<{} parsers>", self.parsers.len()))
            .finish()
    }
}

impl DocumentManager {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_or_create_parser(&mut self, file_type: FileType) -> Option<&mut Parser> {
        if !self.parsers.contains_key(&file_type) {
            let parser = parser::create_parser(file_type)?;
            self.parsers.insert(file_type, parser);
        }
        self.parsers.get_mut(&file_type)
    }

    pub fn open(&mut self, uri: String, text: String, version: i32) -> Option<&Document> {
        let file_type = FileType::from_uri(&uri);
        let parser = self.get_or_create_parser(file_type)?;
        let tree = parser.parse(&text, None)?;
        let doc = Document {
            uri: uri.clone(),
            file_type,
            source: text,
            tree,
            version,
        };
        self.documents.insert(uri.clone(), doc);
        self.documents.get(&uri)
    }

    pub fn update(&mut self, uri: &str, text: String, version: i32) -> Option<&Document> {
        let file_type = self.documents.get(uri)?.file_type;

        // Inline parser creation to allow split borrows on self.parsers and self.documents
        if !self.parsers.contains_key(&file_type) {
            let p = parser::create_parser(file_type)?;
            self.parsers.insert(file_type, p);
        }

        let parser = self.parsers.get_mut(&file_type)?;
        let old_tree = self.documents.get(uri).map(|d| &d.tree);
        let tree = parser.parse(&text, old_tree)?;

        let doc = self.documents.get_mut(uri)?;
        doc.source = text;
        doc.tree = tree;
        doc.version = version;
        Some(doc)
    }

    pub fn close(&mut self, uri: &str) {
        self.documents.remove(uri);
    }

    pub fn get(&self, uri: &str) -> Option<&Document> {
        self.documents.get(uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_html_document() {
        let mut mgr = DocumentManager::new();
        let doc = mgr.open(
            "file:///test.html".to_string(),
            "<html><body></body></html>".to_string(),
            1,
        );
        assert!(doc.is_some());
        let doc = doc.unwrap();
        assert_eq!(doc.file_type, FileType::Html);
        assert_eq!(doc.version, 1);
    }

    #[test]
    fn test_open_unknown_file_returns_none() {
        let mut mgr = DocumentManager::new();
        let doc = mgr.open("file:///test.rs".to_string(), "fn main() {}".to_string(), 1);
        assert!(doc.is_none());
    }

    #[test]
    fn test_update_document() {
        let mut mgr = DocumentManager::new();
        mgr.open("file:///test.html".to_string(), "<img>".to_string(), 1);
        let doc = mgr.update("file:///test.html", "<img alt=\"hi\">".to_string(), 2);
        assert!(doc.is_some());
        let doc = doc.unwrap();
        assert_eq!(doc.version, 2);
        assert_eq!(doc.source, "<img alt=\"hi\">");
    }

    #[test]
    fn test_close_document() {
        let mut mgr = DocumentManager::new();
        mgr.open("file:///test.html".to_string(), "<img>".to_string(), 1);
        mgr.close("file:///test.html");
        assert!(mgr.get("file:///test.html").is_none());
    }
}
