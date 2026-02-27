# WCAG LSP Server Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Rust-based LSP server that statically checks HTML/JSX/TSX/Vue/Svelte for WCAG 2.1 violations using tree-sitter.

**Architecture:** LSP layer (tower-lsp-server) receives document events, passes content to a Document Manager that maintains tree-sitter parse trees per file. A Rule Engine traverses the AST and dispatches nodes to individual WCAG rule checks. Configuration is loaded from `.wcag-lsp.toml`.

**Tech Stack:** Rust (edition 2024), tower-lsp-server 0.23, tree-sitter 0.24+, tree-sitter-html, tree-sitter-javascript, tree-sitter-typescript, tokio, serde, toml

---

### Task 1: Project Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "wcag-lsp"
version = "0.1.0"
edition = "2024"

[dependencies]
tower-lsp-server = "0.23"
tree-sitter = "0.24"
tree-sitter-html = "0.23"
tree-sitter-javascript = "0.25"
tree-sitter-typescript = "0.23"
tokio = { version = "1", features = ["io-std", "macros", "rt-multi-thread"] }
serde = { version = "1", features = ["derive"] }
toml = "0.8"
glob-match = "0.2"
```

**Step 2: Create minimal main.rs**

```rust
fn main() {
    println!("wcag-lsp");
}
```

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully (dependencies download)

**Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs
git commit -m "feat: initialize project with dependencies"
```

---

### Task 2: Minimal LSP Server (stdio)

**Files:**
- Create: `src/server.rs`
- Modify: `src/main.rs`

**Step 1: Write a test that the server struct can be created**

Add to `src/server.rs`:

```rust
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct WcagLspServer {
    pub client: Client,
}

impl WcagLspServer {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl LanguageServer for WcagLspServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "wcag-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            ..Default::default()
        })
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "wcag-lsp initialized")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = params.text_document.version;
        // TODO: parse and diagnose
        self.client.publish_diagnostics(uri, vec![], Some(version)).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        if let Some(change) = params.content_changes.into_iter().last() {
            // TODO: parse and diagnose
            self.client.publish_diagnostics(uri, vec![], Some(version)).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.client
            .publish_diagnostics(params.text_document.uri, vec![], None)
            .await;
    }
}
```

**Step 2: Update main.rs to start server**

```rust
mod server;

use server::WcagLspServer;
use tower_lsp_server::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| WcagLspServer::new(client));
    Server::new(stdin, stdout, socket).serve(service).await;
}
```

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add src/main.rs src/server.rs
git commit -m "feat: add minimal LSP server with stdio transport"
```

---

### Task 3: Tree-sitter Parser Pool

**Files:**
- Create: `src/parser.rs`

**Step 1: Write tests for parser creation and language detection**

```rust
use tree_sitter::{Language, Parser};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            "astro" | "php" | "erb" | "hbs" | "twig" => FileType::Html, // fallback
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
            FileType::Vue => Some(tree_sitter_html::LANGUAGE.into()),     // fallback to HTML
            FileType::Svelte => Some(tree_sitter_html::LANGUAGE.into()),  // fallback to HTML for v1
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
```

**Step 2: Run tests**

Run: `cargo test --lib parser`
Expected: All tests pass

**Step 3: Commit**

```bash
git add src/parser.rs
git commit -m "feat: add tree-sitter parser pool with file type detection"
```

---

### Task 4: Document Manager

**Files:**
- Create: `src/document.rs`
- Modify: `src/server.rs`

**Step 1: Create document manager with tests**

```rust
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

#[derive(Debug, Default)]
pub struct DocumentManager {
    documents: HashMap<String, Document>,
    parsers: HashMap<FileType, Parser>,
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
        let parser = self.get_or_create_parser(file_type)?;
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
        let doc = mgr.open(
            "file:///test.rs".to_string(),
            "fn main() {}".to_string(),
            1,
        );
        assert!(doc.is_none());
    }

    #[test]
    fn test_update_document() {
        let mut mgr = DocumentManager::new();
        mgr.open(
            "file:///test.html".to_string(),
            "<img>".to_string(),
            1,
        );
        let doc = mgr.update("file:///test.html", "<img alt=\"hi\">".to_string(), 2);
        assert!(doc.is_some());
        let doc = doc.unwrap();
        assert_eq!(doc.version, 2);
        assert_eq!(doc.source, "<img alt=\"hi\">");
    }

    #[test]
    fn test_close_document() {
        let mut mgr = DocumentManager::new();
        mgr.open(
            "file:///test.html".to_string(),
            "<img>".to_string(),
            1,
        );
        mgr.close("file:///test.html");
        assert!(mgr.get("file:///test.html").is_none());
    }
}
```

**Step 2: Add `mod document;` to main.rs**

Add `mod document;` after `mod parser;` in `src/main.rs`.

**Step 3: Integrate DocumentManager into server.rs**

Wrap `DocumentManager` in `Arc<RwLock<DocumentManager>>` in the `WcagLspServer` struct and use it in `did_open`, `did_change`, `did_close`.

```rust
use crate::document::DocumentManager;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct WcagLspServer {
    pub client: Client,
    pub documents: Arc<RwLock<DocumentManager>>,
}

impl WcagLspServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(DocumentManager::new())),
        }
    }
}
```

Update handlers to use `self.documents`:

```rust
async fn did_open(&self, params: DidOpenTextDocumentParams) {
    let uri_str = params.text_document.uri.to_string();
    let text = params.text_document.text;
    let version = params.text_document.version;

    let mut docs = self.documents.write().await;
    docs.open(uri_str, text, version);
    // TODO: run diagnostics
    drop(docs);

    self.client
        .publish_diagnostics(params.text_document.uri, vec![], Some(version))
        .await;
}

async fn did_change(&self, params: DidChangeTextDocumentParams) {
    let uri = params.text_document.uri;
    let version = params.text_document.version;
    if let Some(change) = params.content_changes.into_iter().last() {
        let mut docs = self.documents.write().await;
        docs.update(&uri.to_string(), change.text, version);
        drop(docs);

        self.client
            .publish_diagnostics(uri, vec![], Some(version))
            .await;
    }
}

async fn did_close(&self, params: DidCloseTextDocumentParams) {
    let mut docs = self.documents.write().await;
    docs.close(&params.text_document.uri.to_string());
    drop(docs);

    self.client
        .publish_diagnostics(params.text_document.uri, vec![], None)
        .await;
}
```

**Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 5: Commit**

```bash
git add src/document.rs src/server.rs src/main.rs
git commit -m "feat: add document manager with tree-sitter integration"
```

---

### Task 5: Rule Engine Skeleton

**Files:**
- Create: `src/engine.rs`
- Create: `src/rules/mod.rs`

**Step 1: Define the rule trait and engine**

`src/engine.rs`:

```rust
use crate::document::Document;
use crate::rules::Rule;
use tower_lsp_server::ls_types::*;

pub fn run_diagnostics(doc: &Document, rules: &[Box<dyn Rule>]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let root = doc.tree.root_node();
    let source = doc.source.as_bytes();

    for rule in rules {
        let mut cursor = tree_sitter::QueryCursor::new();
        let rule_diagnostics = rule.check(&root, &doc.source, doc.file_type);
        diagnostics.extend(rule_diagnostics);
    }

    diagnostics
}

pub fn node_to_range(node: &tree_sitter::Node) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range {
        start: Position {
            line: start.row as u32,
            character: start.column as u32,
        },
        end: Position {
            line: end.row as u32,
            character: end.column as u32,
        },
    }
}
```

`src/rules/mod.rs`:

```rust
use crate::parser::FileType;
use tower_lsp_server::ls_types::Diagnostic;
use tree_sitter::Node;

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
        Box::new(img_alt::ImgAlt),
    ]
}
```

**Step 2: Add modules to main.rs**

```rust
mod document;
mod engine;
mod parser;
mod rules;
mod server;
```

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles (img_alt module will be created in next task)

**Step 4: Commit**

```bash
git add src/engine.rs src/rules/mod.rs src/main.rs
git commit -m "feat: add rule engine skeleton and rule trait"
```

---

### Task 6: First Rule — img-alt (TDD)

**Files:**
- Create: `src/rules/img_alt.rs`
- Create: `tests/fixtures/img_alt_fail.html`
- Create: `tests/fixtures/img_alt_pass.html`

**Step 1: Write the failing test**

`src/rules/img_alt.rs`:

```rust
use crate::engine::node_to_range;
use crate::parser::FileType;
use crate::rules::{Rule, RuleMetadata, Severity, WcagLevel};
use tower_lsp_server::ls_types::*;
use tree_sitter::Node;

pub struct ImgAlt;

static METADATA: RuleMetadata = RuleMetadata {
    id: "img-alt",
    description: "Images must have an alt attribute",
    wcag_level: WcagLevel::A,
    wcag_criterion: "1.1.1",
    wcag_url: "https://www.w3.org/WAI/WCAG21/Understanding/non-text-content.html",
    default_severity: Severity::Error,
};

impl Rule for ImgAlt {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn check(&self, root: &Node, source: &str, file_type: FileType) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        self.visit(root, source, file_type, &mut diagnostics);
        diagnostics
    }
}

impl ImgAlt {
    fn visit(
        &self,
        node: &Node,
        source: &str,
        file_type: FileType,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // For HTML: look for element nodes with tag_name "img"
        // For JSX/TSX: look for jsx_self_closing_element with name "img"
        if self.is_img_element(node, source, file_type) {
            if !self.has_alt_attribute(node, source, file_type) {
                let meta = self.metadata();
                diagnostics.push(Diagnostic {
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
                });
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit(&child, source, file_type, diagnostics);
        }
    }

    fn is_img_element(&self, node: &Node, source: &str, file_type: FileType) -> bool {
        if file_type.is_jsx_like() {
            // JSX: jsx_self_closing_element or jsx_opening_element
            if node.kind() == "jsx_self_closing_element" || node.kind() == "jsx_opening_element" {
                return self.get_jsx_tag_name(node, source) == Some("img");
            }
            return false;
        }

        // HTML: start_tag inside an element node
        if node.kind() == "element" || node.kind() == "self_closing_tag" {
            let tag_node = if node.kind() == "self_closing_tag" {
                Some(node.clone())
            } else {
                node.child_by_field_name("start_tag")
                    .or_else(|| {
                        let mut cursor = node.walk();
                        node.children(&mut cursor)
                            .find(|c| c.kind() == "start_tag")
                    })
            };
            if let Some(tag) = tag_node {
                let mut cursor = tag.walk();
                for child in tag.children(&mut cursor) {
                    if child.kind() == "tag_name" {
                        let name = &source[child.byte_range()];
                        return name.eq_ignore_ascii_case("img");
                    }
                }
            }
        }
        false
    }

    fn has_alt_attribute(&self, node: &Node, source: &str, file_type: FileType) -> bool {
        if file_type.is_jsx_like() {
            return self.has_jsx_attribute(node, source, "alt");
        }

        // HTML: look in start_tag or self_closing_tag for attribute with name "alt"
        let tag_node = if node.kind() == "self_closing_tag" {
            Some(node.clone())
        } else {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .find(|c| c.kind() == "start_tag")
        };

        if let Some(tag) = tag_node {
            let mut cursor = tag.walk();
            for child in tag.children(&mut cursor) {
                if child.kind() == "attribute" {
                    let mut attr_cursor = child.walk();
                    for attr_child in child.children(&mut attr_cursor) {
                        if attr_child.kind() == "attribute_name" {
                            let name = &source[attr_child.byte_range()];
                            if name.eq_ignore_ascii_case("alt") {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    fn get_jsx_tag_name<'a>(&self, node: &Node, source: &'a str) -> Option<&'a str> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "member_expression" {
                return Some(&source[child.byte_range()]);
            }
        }
        None
    }

    fn has_jsx_attribute(&self, node: &Node, source: &str, attr_name: &str) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "jsx_attribute" {
                let mut attr_cursor = child.walk();
                for attr_child in child.children(&mut attr_cursor) {
                    if attr_child.kind() == "property_identifier" {
                        let name = &source[attr_child.byte_range()];
                        if name == attr_name {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn check_html(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Html).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = ImgAlt;
        rule.check(&tree.root_node(), source, FileType::Html)
    }

    fn check_tsx(source: &str) -> Vec<Diagnostic> {
        let mut parser = parser::create_parser(FileType::Tsx).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let rule = ImgAlt;
        rule.check(&tree.root_node(), source, FileType::Tsx)
    }

    #[test]
    fn test_img_without_alt_fails() {
        let diags = check_html(r#"<img src="photo.jpg">"#);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("img-alt".to_string()))
        );
    }

    #[test]
    fn test_img_with_alt_passes() {
        let diags = check_html(r#"<img src="photo.jpg" alt="A photo">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_img_with_empty_alt_passes() {
        // Empty alt is valid (decorative image)
        let diags = check_html(r#"<img src="spacer.gif" alt="">"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_no_img_no_diagnostic() {
        let diags = check_html(r#"<div><p>Hello</p></div>"#);
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn test_multiple_imgs_mixed() {
        let diags = check_html(
            r#"<div><img src="a.jpg" alt="A"><img src="b.jpg"></div>"#,
        );
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_img_without_alt_fails() {
        let diags = check_tsx(r#"const App = () => <img src="photo.jpg" />;"#);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn test_tsx_img_with_alt_passes() {
        let diags = check_tsx(r#"const App = () => <img src="photo.jpg" alt="A photo" />;"#);
        assert_eq!(diags.len(), 0);
    }
}
```

**Step 2: Run tests to verify they pass**

Run: `cargo test --lib rules::img_alt`
Expected: All 7 tests pass

**Step 3: Commit**

```bash
git add src/rules/img_alt.rs
git commit -m "feat: add img-alt WCAG rule with HTML and JSX support"
```

---

### Task 7: Wire Rules into LSP Server

**Files:**
- Modify: `src/server.rs`
- Modify: `src/engine.rs`

**Step 1: Update engine to accept rules and return diagnostics**

Ensure `engine::run_diagnostics` works with the rule list:

```rust
use crate::document::Document;
use crate::rules::Rule;
use tower_lsp_server::ls_types::*;

pub fn run_diagnostics(doc: &Document, rules: &[Box<dyn Rule>]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for rule in rules {
        let rule_diags = rule.check(&doc.tree.root_node(), &doc.source, doc.file_type);
        diagnostics.extend(rule_diags);
    }
    diagnostics
}

pub fn node_to_range(node: &tree_sitter::Node) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range {
        start: Position {
            line: start.row as u32,
            character: start.column as u32,
        },
        end: Position {
            line: end.row as u32,
            character: end.column as u32,
        },
    }
}
```

**Step 2: Update server.rs to run diagnostics after open/change**

Add `rules: Vec<Box<dyn Rule>>` to `WcagLspServer` and call `engine::run_diagnostics`:

```rust
use crate::engine;
use crate::rules;

impl WcagLspServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(DocumentManager::new())),
            rules: rules::all_rules(),
        }
    }

    async fn diagnose(&self, uri: tower_lsp_server::ls_types::Uri, version: Option<i32>) {
        let docs = self.documents.read().await;
        let uri_str = uri.to_string();
        let diagnostics = if let Some(doc) = docs.get(&uri_str) {
            engine::run_diagnostics(doc, &self.rules)
        } else {
            vec![]
        };
        drop(docs);
        self.client.publish_diagnostics(uri, diagnostics, version).await;
    }
}
```

Update `did_open` and `did_change` to call `self.diagnose(...)`.

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add src/server.rs src/engine.rs
git commit -m "feat: wire rule engine into LSP server diagnostics pipeline"
```

---

### Task 8: Configuration Loader

**Files:**
- Create: `src/config.rs`

**Step 1: Create config module with tests**

```rust
use crate::rules::{Severity, WcagLevel};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Default)]
pub struct RawConfig {
    #[serde(default)]
    pub severity: HashMap<String, String>,
    #[serde(default)]
    pub rules: HashMap<String, String>,
    #[serde(default)]
    pub ignore: IgnoreConfig,
}

#[derive(Debug, Deserialize, Default)]
pub struct IgnoreConfig {
    #[serde(default)]
    pub patterns: Vec<String>,
}

#[derive(Debug)]
pub struct Config {
    pub severity_a: Severity,
    pub severity_aa: Severity,
    pub severity_aaa: Severity,
    pub rule_overrides: HashMap<String, RuleOverride>,
    pub ignore_patterns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuleOverride {
    Off,
    Severity(Severity),
}

impl Default for Config {
    fn default() -> Self {
        Self {
            severity_a: Severity::Error,
            severity_aa: Severity::Warning,
            severity_aaa: Severity::Warning,
            rule_overrides: HashMap::new(),
            ignore_patterns: vec![],
        }
    }
}

impl Config {
    pub fn from_file(path: &Path) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        Self::from_str(&content)
    }

    pub fn from_str(content: &str) -> Self {
        let raw: RawConfig = match toml::from_str(content) {
            Ok(r) => r,
            Err(_) => return Self::default(),
        };

        let parse_severity = |s: &str| -> Option<Severity> {
            match s.to_lowercase().as_str() {
                "error" => Some(Severity::Error),
                "warning" | "warn" => Some(Severity::Warning),
                _ => None,
            }
        };

        let severity_a = raw
            .severity
            .get("A")
            .and_then(|s| parse_severity(s))
            .unwrap_or(Severity::Error);
        let severity_aa = raw
            .severity
            .get("AA")
            .and_then(|s| parse_severity(s))
            .unwrap_or(Severity::Warning);
        let severity_aaa = raw
            .severity
            .get("AAA")
            .and_then(|s| parse_severity(s))
            .unwrap_or(Severity::Warning);

        let mut rule_overrides = HashMap::new();
        for (rule_id, value) in &raw.rules {
            let override_val = match value.to_lowercase().as_str() {
                "off" | "false" | "disable" => RuleOverride::Off,
                "error" => RuleOverride::Severity(Severity::Error),
                "warning" | "warn" => RuleOverride::Severity(Severity::Warning),
                _ => continue,
            };
            rule_overrides.insert(rule_id.clone(), override_val);
        }

        Config {
            severity_a,
            severity_aa,
            severity_aaa,
            rule_overrides,
            ignore_patterns: raw.ignore.patterns,
        }
    }

    pub fn severity_for_level(&self, level: WcagLevel) -> Severity {
        match level {
            WcagLevel::A => self.severity_a,
            WcagLevel::AA => self.severity_aa,
            WcagLevel::AAA => self.severity_aaa,
        }
    }

    pub fn is_rule_enabled(&self, rule_id: &str) -> bool {
        self.rule_overrides
            .get(rule_id)
            .map(|o| *o != RuleOverride::Off)
            .unwrap_or(true)
    }

    pub fn effective_severity(&self, rule_id: &str, level: WcagLevel) -> Severity {
        if let Some(RuleOverride::Severity(s)) = self.rule_overrides.get(rule_id) {
            return *s;
        }
        self.severity_for_level(level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.severity_a, Severity::Error);
        assert_eq!(config.severity_aa, Severity::Warning);
        assert_eq!(config.severity_aaa, Severity::Warning);
    }

    #[test]
    fn test_parse_config() {
        let config = Config::from_str(
            r#"
[severity]
A = "error"
AA = "error"
AAA = "warning"

[rules]
img-alt = "warning"
heading-order = "off"

[ignore]
patterns = ["node_modules/**", "dist/**"]
"#,
        );
        assert_eq!(config.severity_aa, Severity::Error);
        assert_eq!(
            config.rule_overrides.get("heading-order"),
            Some(&RuleOverride::Off)
        );
        assert!(!config.is_rule_enabled("heading-order"));
        assert!(config.is_rule_enabled("img-alt"));
        assert_eq!(
            config.effective_severity("img-alt", WcagLevel::A),
            Severity::Warning
        );
        assert_eq!(config.ignore_patterns.len(), 2);
    }

    #[test]
    fn test_invalid_toml_returns_defaults() {
        let config = Config::from_str("this is not valid toml {{{}}}");
        assert_eq!(config.severity_a, Severity::Error);
    }

    #[test]
    fn test_empty_config_returns_defaults() {
        let config = Config::from_str("");
        assert_eq!(config.severity_a, Severity::Error);
        assert!(config.rule_overrides.is_empty());
    }
}
```

**Step 2: Add `mod config;` to main.rs**

**Step 3: Run tests**

Run: `cargo test --lib config`
Expected: All tests pass

**Step 4: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: add TOML configuration loader with severity mapping"
```

---

### Task 9: Integrate Config into Rule Engine + Server

**Files:**
- Modify: `src/engine.rs`
- Modify: `src/server.rs`

**Step 1: Update engine to use config for filtering and severity**

Update `run_diagnostics` to accept a `Config` reference, filter disabled rules, and apply severity overrides:

```rust
use crate::config::Config;

pub fn run_diagnostics(doc: &Document, rules: &[Box<dyn Rule>], config: &Config) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for rule in rules {
        let meta = rule.metadata();
        if !config.is_rule_enabled(meta.id) {
            continue;
        }
        let severity = config.effective_severity(meta.id, meta.wcag_level);
        let lsp_severity = match severity {
            crate::rules::Severity::Error => DiagnosticSeverity::ERROR,
            crate::rules::Severity::Warning => DiagnosticSeverity::WARNING,
        };

        let mut rule_diags = rule.check(&doc.tree.root_node(), &doc.source, doc.file_type);
        for diag in &mut rule_diags {
            diag.severity = Some(lsp_severity);
        }
        diagnostics.extend(rule_diags);
    }
    diagnostics
}
```

**Step 2: Add Config to WcagLspServer**

```rust
pub struct WcagLspServer {
    pub client: Client,
    pub documents: Arc<RwLock<DocumentManager>>,
    pub config: Arc<RwLock<Config>>,
    pub rules: Vec<Box<dyn Rule>>,
}
```

Load config on `initialize` from workspace root. Update `diagnose` to pass config.

**Step 3: Run all tests**

Run: `cargo test`
Expected: All tests pass

**Step 4: Commit**

```bash
git add src/engine.rs src/server.rs
git commit -m "feat: integrate config into rule engine for severity and filtering"
```

---

### Task 10: Additional Rules — html-lang

**Files:**
- Create: `src/rules/html_lang.rs`
- Modify: `src/rules/mod.rs`

Follow the same pattern as img-alt. This rule checks that `<html>` elements have a `lang` attribute.

**Key logic:**
- Traverse AST for elements with tag name `html`
- Check for `lang` attribute
- WCAG 3.1.1, Level A

**Tests:**
- `<html>` without lang → 1 diagnostic
- `<html lang="en">` → 0 diagnostics
- `<html lang="">` → 1 diagnostic (empty lang is invalid)

After tests pass: add `Box::new(html_lang::HtmlLang)` to `all_rules()` in `mod.rs`.

Commit: `git commit -m "feat: add html-lang WCAG rule"`

---

### Task 11: Additional Rules — heading-order

**Files:**
- Create: `src/rules/heading_order.rs`
- Modify: `src/rules/mod.rs`

Checks that heading levels are not skipped (e.g., h1 → h3 without h2).

**Key logic:**
- Collect all heading elements (h1-h6) in document order
- Check that each heading level is at most one greater than the previous
- WCAG 1.3.1, Level A

**Tests:**
- `<h1>A</h1><h3>B</h3>` → 1 diagnostic (skipped h2)
- `<h1>A</h1><h2>B</h2><h3>C</h3>` → 0 diagnostics
- `<h2>A</h2>` → 1 diagnostic (starts at h2 without h1)

Commit: `git commit -m "feat: add heading-order WCAG rule"`

---

### Task 12: Additional Rules — form-label, anchor-content, click-events

**Files:**
- Create: `src/rules/form_label.rs` — form controls (input, select, textarea) need a label via `aria-label`, `aria-labelledby`, `id` matching a `<label for>`, or wrapping `<label>`. WCAG 1.3.1, Level A.
- Create: `src/rules/anchor_content.rs` — `<a>` elements must have text content or `aria-label`. WCAG 2.4.4, Level A.
- Create: `src/rules/click_events.rs` — elements with `onclick` must also have `onkeydown` or `onkeyup` (HTML) / `onClick` must have `onKeyDown`/`onKeyUp` (JSX). WCAG 2.1.1, Level A.
- Modify: `src/rules/mod.rs` — register all new rules

Each rule follows the same pattern: implement `Rule` trait, write unit tests for pass/fail cases, register in `all_rules()`.

Commit: `git commit -m "feat: add form-label, anchor-content, click-events rules"`

---

### Task 13: Additional Rules — aria-role, aria-props, tabindex, iframe-title

**Files:**
- Create: `src/rules/aria_role.rs` — validates `role` attribute values against WAI-ARIA spec. WCAG 4.1.2, Level A.
- Create: `src/rules/aria_props.rs` — validates `aria-*` attribute names exist in the spec. WCAG 4.1.2, Level A.
- Create: `src/rules/tabindex.rs` — warns on positive tabindex values (> 0). WCAG 2.4.3, Level A.
- Create: `src/rules/iframe_title.rs` — `<iframe>` must have a `title` attribute. WCAG 2.4.1, Level A.
- Modify: `src/rules/mod.rs` — register all new rules

Commit: `git commit -m "feat: add aria-role, aria-props, tabindex, iframe-title rules"`

---

### Task 14: Additional Rules — meta-refresh, media-captions, table-header, no-redundant-alt

**Files:**
- Create: `src/rules/meta_refresh.rs` — `<meta http-equiv="refresh">` with time > 0. WCAG 2.2.1, Level A.
- Create: `src/rules/media_captions.rs` — `<video>` and `<audio>` should have `<track kind="captions">`. WCAG 1.2.2, Level A.
- Create: `src/rules/table_header.rs` — `<table>` must contain `<th>` elements. WCAG 1.3.1, Level A.
- Create: `src/rules/no_redundant_alt.rs` — alt text should not contain "image", "picture", "photo". WCAG 1.1.1, Level A.
- Modify: `src/rules/mod.rs` — register all new rules

Commit: `git commit -m "feat: add meta-refresh, media-captions, table-header, no-redundant-alt rules"`

---

### Task 15: Debouncing

**Files:**
- Modify: `src/server.rs`

**Step 1: Add debounce logic**

Use a `tokio::time::sleep` approach: on `did_change`, cancel any pending diagnostic task and schedule a new one after 150ms.

```rust
use tokio::sync::Notify;
use std::sync::atomic::{AtomicI64, Ordering};

// In WcagLspServer:
// Add a debounce version counter per URI. On each change, increment the counter.
// Spawn a task that sleeps 150ms, then checks if the counter is still current.
// If yes, run diagnostics. If not, another change came in — skip.
```

**Step 2: Test manually by connecting to an editor**

Run: `cargo build && cargo run` (connect via Neovim or VS Code)

**Step 3: Commit**

```bash
git add src/server.rs
git commit -m "feat: add 150ms debounce for diagnostic publishing"
```

---

### Task 16: Ignore Patterns

**Files:**
- Modify: `src/server.rs`

**Step 1: Filter diagnostics based on ignore patterns in config**

Before running diagnostics in `diagnose()`, check if the file URI matches any ignore pattern using the `glob-match` crate. If it matches, publish empty diagnostics instead.

```rust
use glob_match::glob_match;

// In diagnose():
let config = self.config.read().await;
let uri_path = uri.to_string();
for pattern in &config.ignore_patterns {
    if glob_match(pattern, &uri_path) {
        self.client.publish_diagnostics(uri, vec![], version).await;
        return;
    }
}
```

**Step 2: Run tests**

Run: `cargo test`
Expected: All pass

**Step 3: Commit**

```bash
git add src/server.rs
git commit -m "feat: add file ignore pattern support from config"
```

---

### Task 17: Integration Test

**Files:**
- Create: `tests/integration.rs`

**Step 1: Write an integration test that creates a document manager, opens an HTML file, and verifies diagnostics**

```rust
use wcag_lsp::config::Config;
use wcag_lsp::document::DocumentManager;
use wcag_lsp::engine;
use wcag_lsp::rules;

#[test]
fn test_full_html_analysis() {
    let mut mgr = DocumentManager::new();
    let html = r#"<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>
  <img src="photo.jpg">
  <a href="/"></a>
  <iframe src="/embed"></iframe>
</body>
</html>"#;

    let doc = mgr.open("file:///test.html".to_string(), html.to_string(), 1).unwrap();
    let rules = rules::all_rules();
    let config = Config::default();
    let diagnostics = engine::run_diagnostics(doc, &rules, &config);

    // Should find: img-alt, html-lang, anchor-content, iframe-title
    assert!(diagnostics.len() >= 4, "Expected at least 4 diagnostics, got {}", diagnostics.len());

    let codes: Vec<String> = diagnostics
        .iter()
        .filter_map(|d| d.code.as_ref())
        .map(|c| match c {
            tower_lsp_server::ls_types::NumberOrString::String(s) => s.clone(),
            _ => String::new(),
        })
        .collect();
    assert!(codes.contains(&"img-alt".to_string()));
    assert!(codes.contains(&"html-lang".to_string()));
}

#[test]
fn test_tsx_analysis() {
    let mut mgr = DocumentManager::new();
    let tsx = r#"const App = () => (
  <div>
    <img src="photo.jpg" />
    <a href="/"></a>
  </div>
);"#;

    let doc = mgr.open("file:///App.tsx".to_string(), tsx.to_string(), 1).unwrap();
    let rules = rules::all_rules();
    let config = Config::default();
    let diagnostics = engine::run_diagnostics(doc, &rules, &config);

    assert!(diagnostics.len() >= 1, "Expected at least 1 diagnostic for missing alt");
}
```

**Step 2: Make module items public in lib.rs**

Create `src/lib.rs` that re-exports modules for integration tests:

```rust
pub mod config;
pub mod document;
pub mod engine;
pub mod parser;
pub mod rules;
```

**Step 3: Run integration tests**

Run: `cargo test --test integration`
Expected: All pass

**Step 4: Commit**

```bash
git add tests/integration.rs src/lib.rs
git commit -m "test: add integration tests for full HTML and TSX analysis"
```

---

### Summary

After all tasks, the project will have:
- A working LSP server connectable via stdio to Neovim/VS Code
- Tree-sitter-based parsing for HTML, JSX, TSX (Vue/Svelte via HTML fallback)
- ~14 WCAG rules covering the most common accessibility violations
- Configurable severity and rule toggles via `.wcag-lsp.toml`
- 150ms debounce for real-time diagnostics
- File ignore patterns
- Unit tests per rule and integration tests

**Adding more rules** follows the established pattern from Tasks 6/10-14: create `src/rules/<name>.rs`, implement `Rule` trait, write tests, register in `all_rules()`.
