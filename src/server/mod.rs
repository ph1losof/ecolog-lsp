pub mod config;
pub mod env_resolution;
pub mod error;
pub mod handlers;
pub mod state;
pub mod util;

pub use error::LspError;

use crate::analysis::{DocumentManager, QueryEngine};
use crate::languages::LanguageRegistry;
use crate::server::state::ServerState;
use std::sync::Arc;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use tracing::info;

pub struct LspServer {
    pub client: Client,
    pub state: ServerState,
}

impl LspServer {
    pub fn new(client: Client, core: abundantis::Abundantis) -> Self {
        let config_manager = crate::server::config::ConfigManager::new();
        let config = Arc::new(config_manager);

        Self::new_with_config(client, core, config)
    }

    pub fn new_with_config(
        client: Client,
        core: abundantis::Abundantis,
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
        let document_manager = Arc::new(DocumentManager::new(
            query_engine.clone(),
            languages.clone(),
        ));

        let workspace_root = core.workspace.read().root().to_path_buf();

        let state = ServerState::with_indexing(
            document_manager,
            languages,
            Arc::new(core),
            config,
            query_engine,
            workspace_root,
        );

        Self { client, state }
    }

    pub async fn register_watched_files(&self) {
        let mut watchers = vec![FileSystemWatcher {
            glob_pattern: GlobPattern::String("**/ecolog.toml".to_string()),
            kind: None,
        }];

        {
            let config = self.state.config.get_config();
            let config = config.read().await;
            for pattern in &config.workspace.env_files {
                watchers.push(FileSystemWatcher {
                    glob_pattern: GlobPattern::String(format!("**/{}", pattern)),
                    kind: None,
                });
            }
        }

        for lang in self.state.languages.all_languages() {
            for ext in lang.extensions() {
                watchers.push(FileSystemWatcher {
                    glob_pattern: GlobPattern::String(format!("**/*.{}", ext)),
                    kind: Some(WatchKind::Create | WatchKind::Delete),
                });
            }
        }

        let registration = Registration {
            id: "ecolog-file-watcher".to_string(),
            method: "workspace/didChangeWatchedFiles".to_string(),
            register_options: Some(
                serde_json::to_value(DidChangeWatchedFilesRegistrationOptions { watchers })
                    .unwrap(),
            ),
        };
        if let Err(e) = self.client.register_capability(vec![registration]).await {
            self.client
                .log_message(
                    MessageType::ERROR,
                    format!("Failed to register watcher: {}", e),
                )
                .await;
        }
    }

    async fn update_workspace_index_for_document(&self, uri: &Url) {
        use crate::analysis::{workspace_index::FileIndexEntry, BindingResolver};
        use compact_str::CompactString;
        use korni::ParseOptions;
        use rustc_hash::FxHashSet;
        use std::time::SystemTime;

        let path = uri
            .to_file_path()
            .unwrap_or_else(|_| std::path::PathBuf::from(uri.path()));

        let mtime = SystemTime::now();

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let is_env_file = {
            let config = self.state.config.get_config();
            let config = config.read().await;
            config.workspace.env_files.iter().any(|pattern| {
                glob::Pattern::new(pattern.as_str())
                    .map(|p| p.matches(file_name))
                    .unwrap_or(false)
            })
        };

        let env_vars: FxHashSet<CompactString> = if is_env_file {
            let vars = if let Some(doc) = self.state.document_manager.get(uri) {
                let content = &doc.content;
                let entries = korni::parse_with_options(content, ParseOptions::full());
                entries
                    .into_iter()
                    .filter_map(|entry| {
                        if let korni::Entry::Pair(kv) = entry {
                            Some(CompactString::from(kv.key.as_ref()))
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                FxHashSet::default()
            };

            util::safe_refresh(&self.state.core, abundantis::RefreshOptions::preserve_all()).await;

            vars
        } else if let Some(graph_ref) = self.state.document_manager.get_binding_graph(uri) {
            let resolver = BindingResolver::new(&graph_ref);
            resolver.all_env_vars().into_iter().collect()
        } else {
            FxHashSet::default()
        };

        self.state.workspace_index.update_file(
            uri,
            FileIndexEntry {
                mtime,
                env_vars,
                is_env_file,
                path,
            },
        );
    }

    /// Request the client to refresh all inlay hints
    async fn refresh_inlay_hints(&self) {
        // workspace/inlayHint/refresh is a server-to-client request
        // that tells the client to re-request inlay hints for all open documents
        if let Err(e) = self
            .client
            .send_request::<tower_lsp::lsp_types::request::InlayHintRefreshRequest>(())
            .await
        {
            tracing::debug!("Failed to refresh inlay hints: {}", e);
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for LspServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        self.state
            .config
            .set_init_settings(params.initialization_options)
            .await;

        let trigger_characters: Vec<String> = {
            let mut chars = std::collections::HashSet::new();
            for lang in self.state.languages.all_languages() {
                for ch in lang.completion_trigger_characters() {
                    chars.insert(ch.to_string());
                }
            }
            chars.into_iter().collect()
        };

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: if trigger_characters.is_empty() {
                        None
                    } else {
                        Some(trigger_characters)
                    },
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                })),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec![
                        "ecolog.file.setActive".to_string(),
                        "ecolog.file.list".to_string(),
                        "ecolog.listEnvVariables".to_string(),
                        "ecolog.generateEnvExample".to_string(),
                        "ecolog.variable.get".to_string(),
                        "ecolog.workspace.list".to_string(),
                        "ecolog.workspace.setRoot".to_string(),
                        "ecolog.interpolation.set".to_string(),
                        "ecolog.interpolation.get".to_string(),
                    ],
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                inlay_hint_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "ecolog-lsp initialized!")
            .await;

        let workspace_root = util::get_workspace_root(&self.state.core.workspace).await;

        let config = self.state.config.load_from_workspace(&workspace_root).await;

        if let Ok(ref cfg) = config {
            self.state
                .core
                .resolution
                .update_resolution_config(cfg.resolution.clone());
            self.state
                .core
                .resolution
                .update_interpolation_config(cfg.interpolation.clone());
        }

        self.client
            .log_message(
                MessageType::INFO,
                format!("Loaded configuration from {}", workspace_root.display()),
            )
            .await;

        self.register_watched_files().await;

        let languages = Arc::clone(&self.state.languages);
        tokio::spawn(async move {
            tokio::task::spawn_blocking(move || {
                for lang in languages.all_languages() {
                    let _ = lang.reference_query();
                    let _ = lang.binding_query();
                    let _ = lang.completion_query();
                    let _ = lang.reassignment_query();
                    let _ = lang.import_query();
                    let _ = lang.identifier_query();
                    let _ = lang.assignment_query();
                    let _ = lang.destructure_query();
                    let _ = lang.scope_query();
                    let _ = lang.export_query();
                }
                tracing::debug!("Query pre-warming complete for all languages");
            })
            .await
            .ok();
        });

        let indexer = Arc::clone(&self.state.indexer);
        let config = self.state.config.get_config();
        let client = self.client.clone();

        tokio::spawn(async move {
            let config_snapshot = {
                let config = config.read().await;
                config.clone()
            };
            info!("Starting background workspace indexing...");
            client
                .log_message(MessageType::INFO, "Starting workspace indexing...")
                .await;

            if let Err(e) = indexer.index_workspace(&config_snapshot).await {
                client
                    .log_message(
                        MessageType::WARNING,
                        format!("Workspace indexing failed: {}", e),
                    )
                    .await;
            } else {
                let stats = indexer.index().stats();
                client
                    .log_message(
                        MessageType::INFO,
                        format!(
                            "Workspace indexing complete: {} files, {} env vars",
                            stats.total_files, stats.total_env_vars
                        ),
                    )
                    .await;
            }
        });

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            let mut heartbeat_count = 0u64;
            loop {
                interval.tick().await;
                heartbeat_count += 1;
                tracing::info!(
                    "LSP heartbeat #{} - async runtime responsive",
                    heartbeat_count
                );
            }
        });
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = &params.text_document.uri;
        tracing::debug!("[HANDLER_ENTER] did_open uri={}", uri);
        let start = std::time::Instant::now();

        self.state
            .document_manager
            .open(
                params.text_document.uri.clone(),
                params.text_document.language_id,
                params.text_document.text,
                params.text_document.version,
            )
            .await;

        self.update_workspace_index_for_document(&params.text_document.uri)
            .await;

        let diagnostics =
            handlers::compute_diagnostics(&params.text_document.uri, &self.state).await;
        self.client
            .publish_diagnostics(params.text_document.uri, diagnostics, None)
            .await;

        tracing::debug!(
            "[HANDLER_EXIT] did_open elapsed_ms={}",
            start.elapsed().as_millis()
        );
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = &params.text_document.uri;
        tracing::debug!("[HANDLER_ENTER] did_change uri={}", uri);
        let start = std::time::Instant::now();

        self.state
            .document_manager
            .change(
                &params.text_document.uri,
                params.content_changes,
                params.text_document.version,
            )
            .await;

        self.update_workspace_index_for_document(&params.text_document.uri)
            .await;

        let diagnostics =
            handlers::compute_diagnostics(&params.text_document.uri, &self.state).await;
        self.client
            .publish_diagnostics(params.text_document.uri, diagnostics, None)
            .await;

        tracing::debug!(
            "[HANDLER_EXIT] did_change elapsed_ms={}",
            start.elapsed().as_millis()
        );
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        tracing::debug!("[HANDLER_ENTER] did_close uri={}", uri);
        let start = std::time::Instant::now();

        self.state.document_manager.close(&uri);

        self.state.workspace_index.remove_file(&uri);

        tracing::debug!(
            "[HANDLER_EXIT] did_close elapsed_ms={}",
            start.elapsed().as_millis()
        );
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        tracing::debug!("[HANDLER_ENTER] hover uri={}", uri);
        let start = std::time::Instant::now();
        let result = handlers::handle_hover(params, &self.state).await;
        tracing::debug!(
            "[HANDLER_EXIT] hover result={} elapsed_ms={}",
            if result.is_some() { "some" } else { "none" },
            start.elapsed().as_millis()
        );
        Ok(result)
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        let config = {
            let config_arc = self.state.config.get_config();
            let config = config_arc.read().await;
            config.clone()
        };

        for change in params.changes {
            let path = match change.uri.to_file_path() {
                Ok(p) => p,
                Err(_) => continue,
            };

            if path.ends_with("ecolog.toml") {
                self.client
                    .log_message(MessageType::INFO, "Reloading configuration...")
                    .await;
                let workspace_root = util::get_workspace_root(&self.state.core.workspace).await;
                let _ = self.state.config.load_from_workspace(&workspace_root).await;
                continue;
            }

            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let is_env_file = config.workspace.env_files.iter().any(|pattern| {
                glob::Pattern::new(pattern.as_str())
                    .map(|p| p.matches(file_name))
                    .unwrap_or(false)
            });

            match change.typ {
                FileChangeType::CREATED | FileChangeType::CHANGED => {
                    self.state
                        .indexer
                        .on_file_changed(&change.uri, &config)
                        .await;

                    if is_env_file {
                        util::safe_refresh(
                            &self.state.core,
                            abundantis::RefreshOptions::preserve_all(),
                        )
                        .await;

                        for uri in self.state.document_manager.all_uris() {
                            let diagnostics =
                                handlers::compute_diagnostics(&uri, &self.state).await;
                            self.client
                                .publish_diagnostics(uri, diagnostics, None)
                                .await;
                        }
                        // Refresh inlay hints when env files change
                        self.refresh_inlay_hints().await;
                    }
                }
                FileChangeType::DELETED => {
                    self.state.indexer.on_file_deleted(&change.uri);

                    if is_env_file {
                        util::safe_refresh(
                            &self.state.core,
                            abundantis::RefreshOptions::preserve_all(),
                        )
                        .await;

                        for uri in self.state.document_manager.all_uris() {
                            let diagnostics =
                                handlers::compute_diagnostics(&uri, &self.state).await;
                            self.client
                                .publish_diagnostics(uri, diagnostics, None)
                                .await;
                        }
                        // Refresh inlay hints when env files are deleted
                        self.refresh_inlay_hints().await;
                    }
                }
                _ => {}
            }
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        tracing::debug!("[HANDLER_ENTER] completion uri={}", uri);
        let start = std::time::Instant::now();
        let result = handlers::handle_completion(params, &self.state).await.map(CompletionResponse::Array);
        tracing::debug!(
            "[HANDLER_EXIT] completion result={} elapsed_ms={}",
            if result.is_some() { "some" } else { "none" },
            start.elapsed().as_millis()
        );
        Ok(result)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        tracing::debug!("[HANDLER_ENTER] goto_definition uri={}", uri);
        let start = std::time::Instant::now();
        let result = handlers::handle_definition(params, &self.state).await;
        tracing::debug!(
            "[HANDLER_EXIT] goto_definition result={} elapsed_ms={}",
            if result.is_some() { "some" } else { "none" },
            start.elapsed().as_millis()
        );
        Ok(result)
    }

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        let command = params.command.clone();
        tracing::debug!("[HANDLER_ENTER] execute_command cmd={}", command);
        let start = std::time::Instant::now();

        let result = handlers::handle_execute_command(params, &self.state).await;

        // Commands that affect env var resolution should refresh diagnostics and inlay hints
        let refresh_commands = [
            "ecolog.source.setPrecedence",
            "ecolog.interpolation.set",
            "ecolog.file.setActive",
            "ecolog.workspace.setRoot",
        ];
        if refresh_commands.contains(&command.as_str()) {
            // Refresh diagnostics for all open documents
            for uri in self.state.document_manager.all_uris() {
                let diagnostics = handlers::compute_diagnostics(&uri, &self.state).await;
                self.client
                    .publish_diagnostics(uri, diagnostics, None)
                    .await;
            }
            // Refresh inlay hints
            self.refresh_inlay_hints().await;
        }

        tracing::debug!(
            "[HANDLER_EXIT] execute_command cmd={} result={} elapsed_ms={}",
            command,
            if result.is_some() { "some" } else { "none" },
            start.elapsed().as_millis()
        );
        Ok(result)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        tracing::debug!("[HANDLER_ENTER] references uri={}", uri);
        let start = std::time::Instant::now();
        let result = handlers::handle_references(params, &self.state).await;
        tracing::debug!(
            "[HANDLER_EXIT] references result={} elapsed_ms={}",
            if result.is_some() { "some" } else { "none" },
            start.elapsed().as_millis()
        );
        Ok(result)
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = &params.text_document.uri;
        tracing::debug!("[HANDLER_ENTER] prepare_rename uri={}", uri);
        let start = std::time::Instant::now();
        let result = handlers::handle_prepare_rename(params, &self.state).await;
        tracing::debug!(
            "[HANDLER_EXIT] prepare_rename result={} elapsed_ms={}",
            if result.is_some() { "some" } else { "none" },
            start.elapsed().as_millis()
        );
        Ok(result)
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = &params.text_document_position.text_document.uri;
        tracing::debug!("[HANDLER_ENTER] rename uri={}", uri);
        let start = std::time::Instant::now();
        let result = handlers::handle_rename(params, &self.state).await;
        tracing::debug!(
            "[HANDLER_EXIT] rename result={} elapsed_ms={}",
            if result.is_some() { "some" } else { "none" },
            start.elapsed().as_millis()
        );
        Ok(result)
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        tracing::debug!("[HANDLER_ENTER] workspace_symbol query={}", params.query);
        let start = std::time::Instant::now();
        let result = handlers::handle_workspace_symbol(params, &self.state).await;
        tracing::debug!(
            "[HANDLER_EXIT] workspace_symbol result={} elapsed_ms={}",
            if result.is_some() { "some" } else { "none" },
            start.elapsed().as_millis()
        );
        Ok(result)
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = &params.text_document.uri;
        tracing::debug!("[HANDLER_ENTER] inlay_hint uri={}", uri);
        let start = std::time::Instant::now();
        let result = handlers::handle_inlay_hints(params, &self.state).await;
        tracing::debug!(
            "[HANDLER_EXIT] inlay_hint result={} elapsed_ms={}",
            if result.is_some() { "some" } else { "none" },
            start.elapsed().as_millis()
        );
        Ok(result)
    }
}
