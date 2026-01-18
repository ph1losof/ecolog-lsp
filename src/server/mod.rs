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

        // Get workspace root for indexer
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
        // Build list of watchers
        let mut watchers = vec![
            // Config file watcher
            FileSystemWatcher {
                glob_pattern: GlobPattern::String("**/ecolog.toml".to_string()),
                kind: None,
            },
        ];

        // Add watchers for env files from config
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

        // Add watchers for all supported code file extensions from LanguageRegistry
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
                serde_json::to_value(DidChangeWatchedFilesRegistrationOptions { watchers }).unwrap(),
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

    /// Update the workspace index for a document after analysis.
    async fn update_workspace_index_for_document(&self, uri: &Url) {
        use crate::analysis::{workspace_index::FileIndexEntry, BindingResolver};
        use compact_str::CompactString;
        use korni::ParseOptions;
        use rustc_hash::FxHashSet;
        use std::time::SystemTime;

        // Get file path for metadata
        let path = uri
            .to_file_path()
            .unwrap_or_else(|_| std::path::PathBuf::from(uri.path()));

        // Use current time as mtime for open documents
        let mtime = SystemTime::now();

        // Determine if this is an env file
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
            // For .env files, parse with korni to extract defined env vars
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

            // Refresh Abundantis to pick up new/renamed env vars
            if let Err(e) = self.state.core.refresh(abundantis::RefreshOptions::preserve_all()).await {
                tracing::warn!("Failed to refresh Abundantis after env file change: {}", e);
            }

            vars
        } else {
            // For code files, extract env var references from binding graph
            if let Some(graph_ref) = self.state.document_manager.get_binding_graph(uri) {
                let resolver = BindingResolver::new(&*graph_ref);
                resolver.all_env_vars().into_iter().collect()
            } else {
                FxHashSet::default()
            }
        };

        // Update the workspace index
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
}

#[tower_lsp::async_trait]
impl LanguageServer for LspServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Store initialization options for later merging with ecolog.toml
        self.state
            .config
            .set_init_settings(params.initialization_options)
            .await;

        // Collect completion trigger characters from all registered languages
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

        let config = self.state.config.load_from_workspace(&workspace_root).await;

        // Update the resolution config in Abundantis core with the merged config
        // This applies source defaults (e.g., shell disabled by default)
        if let Ok(ref cfg) = config {
            self.state.core.resolution.update_resolution_config(cfg.resolution.clone());
            self.state.core.resolution.update_interpolation_config(cfg.interpolation.clone());
        }

        self.client
            .log_message(
                MessageType::INFO,
                format!("Loaded configuration from {}", workspace_root.display()),
            )
            .await;

        self.register_watched_files().await;

        // Performance optimization: Pre-warm tree-sitter query caches in background.
        // Queries are lazily compiled on first access, which can cause jank when
        // opening the first document. By touching all queries now, we ensure
        // they're ready before the user opens any files.
        let languages = Arc::clone(&self.state.languages);
        tokio::spawn(async move {
            tokio::task::spawn_blocking(move || {
                for lang in languages.all_languages() {
                    // Touch all queries to trigger lazy compilation
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

        // Start background workspace indexing
        let indexer = Arc::clone(&self.state.indexer);
        let config = self.state.config.get_config();
        let client = self.client.clone();

        tokio::spawn(async move {
            let config = config.read().await;
            info!("Starting background workspace indexing...");
            client
                .log_message(MessageType::INFO, "Starting workspace indexing...")
                .await;

            if let Err(e) = indexer.index_workspace(&config).await {
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
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.state
            .document_manager
            .open(
                params.text_document.uri.clone(),
                params.text_document.language_id,
                params.text_document.text,
                params.text_document.version,
            )
            .await;

        // Update workspace index with env vars from this document
        self.update_workspace_index_for_document(&params.text_document.uri)
            .await;

        let diagnostics =
            handlers::compute_diagnostics(&params.text_document.uri, &self.state).await;
        self.client
            .publish_diagnostics(params.text_document.uri, diagnostics, None)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.state
            .document_manager
            .change(
                &params.text_document.uri,
                params.content_changes,
                params.text_document.version,
            )
            .await;

        // Update workspace index with env vars from this document
        self.update_workspace_index_for_document(&params.text_document.uri)
            .await;

        // Re-compute diagnostics
        let diagnostics =
            handlers::compute_diagnostics(&params.text_document.uri, &self.state).await;
        self.client
            .publish_diagnostics(params.text_document.uri, diagnostics, None)
            .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;

        // Clean up document from document manager
        self.state.document_manager.close(&uri);

        // Clean up workspace index references (clears env var associations and exports)
        self.state.workspace_index.remove_file(&uri);
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        Ok(handlers::handle_hover(params, &self.state).await)
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

            // Handle ecolog.toml config changes
            if path.ends_with("ecolog.toml") {
                self.client
                    .log_message(MessageType::INFO, "Reloading configuration...")
                    .await;
                let workspace_root = {
                    let workspace = self.state.core.workspace.read();
                    workspace.root().to_path_buf()
                };
                let _ = self.state.config.load_from_workspace(&workspace_root).await;
                continue;
            }

            // Check if this is an env file
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let is_env_file = config.workspace.env_files.iter().any(|pattern| {
                glob::Pattern::new(pattern.as_str())
                    .map(|p| p.matches(file_name))
                    .unwrap_or(false)
            });

            // Handle file changes for workspace index
            match change.typ {
                FileChangeType::CREATED | FileChangeType::CHANGED => {
                    // Re-index the file
                    self.state
                        .indexer
                        .on_file_changed(&change.uri, &config)
                        .await;

                    // Refresh Abundantis for env file changes so diagnostics update correctly
                    // This is important after rename operations where the .env file is modified
                    if is_env_file {
                        if let Err(e) = self.state.core.refresh(abundantis::RefreshOptions::preserve_all()).await {
                            tracing::warn!(
                                "Failed to refresh Abundantis after env file change: {}",
                                e
                            );
                        }

                        // Republish diagnostics for all open documents after env file change
                        // This ensures diagnostics are updated with the new env var definitions
                        for uri in self.state.document_manager.all_uris() {
                            let diagnostics =
                                handlers::compute_diagnostics(&uri, &self.state).await;
                            self.client.publish_diagnostics(uri, diagnostics, None).await;
                        }
                    }
                }
                FileChangeType::DELETED => {
                    // Remove from index
                    self.state.indexer.on_file_deleted(&change.uri);

                    // Refresh Abundantis if env file was deleted
                    if is_env_file {
                        if let Err(e) = self.state.core.refresh(abundantis::RefreshOptions::preserve_all()).await {
                            tracing::warn!(
                                "Failed to refresh Abundantis after env file deletion: {}",
                                e
                            );
                        }

                        // Republish diagnostics for all open documents after env file deletion
                        for uri in self.state.document_manager.all_uris() {
                            let diagnostics =
                                handlers::compute_diagnostics(&uri, &self.state).await;
                            self.client.publish_diagnostics(uri, diagnostics, None).await;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        match handlers::handle_completion(params, &self.state).await {
            Some(items) => Ok(Some(CompletionResponse::Array(items))),
            None => Ok(None),
        }
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        Ok(handlers::handle_definition(params, &self.state).await)
    }

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        let command = params.command.clone();
        let result = handlers::handle_execute_command(params, &self.state).await;

        // Republish diagnostics after configuration changes
        // This ensures hover/completion/diagnostics immediately reflect the new configuration
        if command == "ecolog.source.setPrecedence" || command == "ecolog.interpolation.set" {
            for uri in self.state.document_manager.all_uris() {
                let diagnostics = handlers::compute_diagnostics(&uri, &self.state).await;
                self.client.publish_diagnostics(uri, diagnostics, None).await;
            }
        }

        Ok(result)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        Ok(handlers::handle_references(params, &self.state).await)
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        Ok(handlers::handle_prepare_rename(params, &self.state).await)
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        Ok(handlers::handle_rename(params, &self.state).await)
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        Ok(handlers::handle_workspace_symbol(params, &self.state).await)
    }
}
