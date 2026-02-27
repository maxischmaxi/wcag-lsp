use crate::config::Config;
use crate::document::DocumentManager;
use crate::engine;
use crate::rules::{self, Rule};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer};

pub struct WcagLspServer {
    pub client: Client,
    pub documents: Arc<RwLock<DocumentManager>>,
    pub config: Arc<RwLock<Config>>,
    pub rules: Vec<Box<dyn Rule>>,
}

impl WcagLspServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(DocumentManager::new())),
            config: Arc::new(RwLock::new(Config::default())),
            rules: rules::all_rules(),
        }
    }

    async fn diagnose(&self, uri: Uri, version: Option<i32>) {
        let docs = self.documents.read().await;
        let config = self.config.read().await;
        let uri_str = uri.to_string();
        let diagnostics = if let Some(doc) = docs.get(&uri_str) {
            engine::run_diagnostics(doc, &self.rules, &config)
        } else {
            vec![]
        };
        drop(docs);
        drop(config);
        self.client
            .publish_diagnostics(uri, diagnostics, version)
            .await;
    }
}

impl LanguageServer for WcagLspServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Try to load config from workspace root
        if let Some(folders) = &params.workspace_folders {
            if let Some(folder) = folders.first() {
                if let Some(path) = folder.uri.to_file_path() {
                    let config_path = path.join(".wcag-lsp.toml");
                    let config = Config::from_file(&config_path);
                    *self.config.write().await = config;
                }
            }
        }

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
        let uri_str = uri.to_string();
        let text = params.text_document.text;
        let version = params.text_document.version;

        let mut docs = self.documents.write().await;
        docs.open(uri_str, text, version);
        drop(docs);

        self.diagnose(uri, Some(version)).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        if let Some(change) = params.content_changes.into_iter().last() {
            let mut docs = self.documents.write().await;
            docs.update(&uri.to_string(), change.text, version);
            drop(docs);

            self.diagnose(uri, Some(version)).await;
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
}
