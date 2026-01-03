pub mod state;
pub mod handlers;
pub mod config;
pub mod semantic_tokens;
pub mod util;

use std::sync::Arc;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use crate::analysis::{DocumentManager, QueryEngine};
use crate::languages::LanguageRegistry;
use crate::server::state::ServerState;
 use tokio::sync::Mutex;
use shelter::Masker;

pub struct LspServer {
    pub client: Client,
    pub state: ServerState,
}

impl LspServer {
    pub fn new(client: Client, core: abundantis::Abundantis) -> Self {
        let masker = Arc::new(Mutex::new(shelter::Masker::new(shelter::MaskingConfig::default())));
        let mut config_manager = crate::server::config::ConfigManager::new();
        config_manager.set_masker(masker.clone());
        let config = Arc::new(config_manager);

        Self::new_with_config(client, core, masker, config)
    }

    pub fn new_with_config(
        client: Client,
        core: abundantis::Abundantis,
        masker: Arc<Mutex<Masker>>,
        config: Arc<crate::server::config::ConfigManager>,
    ) -> Self {
        let mut registry = LanguageRegistry::new();
        
        registry.register(Arc::new(crate::languages::javascript::JavaScript));
        registry.register(Arc::new(crate::languages::typescript::TypeScript));
        registry.register(Arc::new(crate::languages::typescript::TypeScriptReact));
        registry.register(Arc::new(crate::languages::python::Python));
        registry.register(Arc::new(crate::languages::rust::Rust));
        registry.register(Arc::new(crate::languages::go::Go));
        
        let languages = Arc::new(registry);

        let query_engine = Arc::new(QueryEngine::new());
        let document_manager = Arc::new(DocumentManager::new(query_engine.clone(), languages.clone()));

        let state = ServerState::new(
            document_manager,
            languages,
            Arc::new(core),
            masker,
            config,
        );

        Self {
            client,
            state,
        }
    }

    pub async fn register_watched_files(&self) {
        let registration = Registration {
            id: "ecolog-config-watcher".to_string(),
            method: "workspace/didChangeWatchedFiles".to_string(),
            register_options: Some(serde_json::to_value(DidChangeWatchedFilesRegistrationOptions {
                watchers: vec![FileSystemWatcher {
                    glob_pattern: GlobPattern::String("**/ecolog.toml".to_string()),
                    kind: None,
                }],
            }).unwrap()),
        };
        if let Err(e) = self.client.register_capability(vec![registration]).await {
             self.client.log_message(MessageType::ERROR, format!("Failed to register watcher: {}", e)).await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for LspServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions::default()),
                definition_provider: Some(OneOf::Left(true)),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                    legend: SemanticTokensLegend {
                        token_types: semantic_tokens::SemanticTokenProvider::LEGEND_TYPES.to_vec(),
                        token_modifiers: semantic_tokens::SemanticTokenProvider::LEGEND_MODIFIERS.to_vec(),
                    },
                    full: Some(SemanticTokensFullOptions::Bool(true)),
                    range: None,
                    work_done_progress_options: WorkDoneProgressOptions { work_done_progress: None },
                })),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        "ecolog.file.setActive".to_string(),
                        "ecolog.listEnvVariables".to_string(),
                        "ecolog.file.list".to_string(),
                    ],
                    work_done_progress_options: WorkDoneProgressOptions { work_done_progress: None },
                }),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "ecolog-lsp initialized!")
            .await;
        
        let workspace_root = {
            let workspace = self.state.core.workspace.read();
            workspace.root().to_path_buf()
        };

        let _ = self.state.config.load_from_workspace(&workspace_root).await;
        self.client.log_message(MessageType::INFO, format!("Loaded configuration from {}", workspace_root.display())).await;
        
        self.register_watched_files().await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.state.document_manager.open(
            params.text_document.uri.clone(),
            params.text_document.language_id,
            params.text_document.text,
            params.text_document.version,
        ).await;

        let diagnostics = handlers::compute_diagnostics(&params.text_document.uri, &self.state).await;
        self.client.publish_diagnostics(params.text_document.uri, diagnostics, None).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.state.document_manager.change(
            &params.text_document.uri,
            params.content_changes,
            params.text_document.version,
        ).await;
        
        // Re-compute diagnostics
        let diagnostics = handlers::compute_diagnostics(&params.text_document.uri, &self.state).await;
        self.client.publish_diagnostics(params.text_document.uri, diagnostics, None).await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        Ok(handlers::handle_hover(params, &self.state).await)
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        for change in params.changes {
            let path = change.uri.to_file_path();
            if let Ok(path) = path {
                if path.ends_with("ecolog.toml") {
                    self.client.log_message(MessageType::INFO, "Reloading configuration...").await;
                    let workspace_root = {
                        let workspace = self.state.core.workspace.read();
                        workspace.root().to_path_buf()
                    };
                    let _ = self.state.config.load_from_workspace(&workspace_root).await;
                }
            }
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        match handlers::handle_completion(params, &self.state).await {
            Some(items) => Ok(Some(CompletionResponse::Array(items))),
            None => Ok(None),
        }
    }

    async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
        Ok(handlers::handle_definition(params, &self.state).await)
    }

    async fn semantic_tokens_full(&self, params: SemanticTokensParams) -> Result<Option<SemanticTokensResult>> {
        Ok(handlers::handle_semantic_tokens_full(params, &self.state).await)
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> Result<Option<serde_json::Value>> {
        Ok(handlers::handle_execute_command(params, &self.state).await)
    }
}
