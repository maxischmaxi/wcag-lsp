use crate::config::Config;
use crate::document::DocumentManager;
use crate::engine;
use crate::rules::{self, Rule};
use glob_match::glob_match;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer};

pub struct WcagLspServer {
    pub client: Client,
    pub documents: Arc<RwLock<DocumentManager>>,
    pub config: Arc<RwLock<Config>>,
    pub rules: Arc<Vec<Box<dyn Rule>>>,
    pub debounce_versions: Arc<RwLock<HashMap<String, i32>>>,
}

impl WcagLspServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(DocumentManager::new())),
            config: Arc::new(RwLock::new(Config::default())),
            rules: Arc::new(rules::all_rules()),
            debounce_versions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn diagnose(&self, uri: Uri, version: Option<i32>) {
        let config = self.config.read().await;

        // Check ignore patterns
        if let Some(file_path) = uri.to_file_path() {
            let path_str = file_path.to_string_lossy();
            for pattern in &config.ignore_patterns {
                if glob_match(pattern, &path_str) {
                    drop(config);
                    self.client.publish_diagnostics(uri, vec![], version).await;
                    return;
                }
            }
        }

        let docs = self.documents.read().await;
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
            let uri_str = uri.to_string();

            let mut docs = self.documents.write().await;
            docs.update(&uri_str, change.text, version);
            drop(docs);

            // Store current version for debounce
            {
                let mut versions = self.debounce_versions.write().await;
                versions.insert(uri_str.clone(), version);
            }

            // Clone Arcs for the spawned task
            let debounce_versions = self.debounce_versions.clone();
            let documents = self.documents.clone();
            let config = self.config.clone();
            let client = self.client.clone();
            let rules = self.rules.clone();

            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;

                // Check if this version is still current
                let current_version = {
                    let versions = debounce_versions.read().await;
                    versions.get(&uri_str).copied()
                };

                if current_version != Some(version) {
                    return; // A newer version came in, skip
                }

                // Check ignore patterns
                let cfg = config.read().await;
                if let Some(file_path) = uri.to_file_path() {
                    let path_str = file_path.to_string_lossy();
                    for pattern in &cfg.ignore_patterns {
                        if glob_match(pattern, &path_str) {
                            drop(cfg);
                            client.publish_diagnostics(uri, vec![], Some(version)).await;
                            return;
                        }
                    }
                }

                // Run diagnostics
                let docs = documents.read().await;
                let diagnostics = if let Some(doc) = docs.get(&uri_str) {
                    engine::run_diagnostics(doc, &rules, &cfg)
                } else {
                    vec![]
                };
                drop(docs);
                drop(cfg);
                client
                    .publish_diagnostics(uri, diagnostics, Some(version))
                    .await;
            });
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
