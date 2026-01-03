use crate::analysis::{
    DocumentManager, ModuleResolver, QueryEngine, WorkspaceIndex, WorkspaceIndexer,
};
use crate::languages::LanguageRegistry;
use abundantis::Abundantis;
use std::path::PathBuf;
use std::sync::Arc;

use crate::server::config::ConfigManager;

#[derive(Clone)]
pub struct ServerState {
    pub document_manager: Arc<DocumentManager>,
    pub languages: Arc<LanguageRegistry>,
    pub core: Arc<Abundantis>,
    pub config: Arc<ConfigManager>,
    pub workspace_index: Arc<WorkspaceIndex>,
    pub indexer: Arc<WorkspaceIndexer>,
    pub module_resolver: Arc<ModuleResolver>,
}

impl ServerState {
    pub fn new(
        document_manager: Arc<DocumentManager>,
        languages: Arc<LanguageRegistry>,
        core: Arc<Abundantis>,
        config: Arc<ConfigManager>,
        workspace_index: Arc<WorkspaceIndex>,
        indexer: Arc<WorkspaceIndexer>,
        module_resolver: Arc<ModuleResolver>,
    ) -> Self {
        Self {
            document_manager,
            languages,
            core,
            config,
            workspace_index,
            indexer,
            module_resolver,
        }
    }

    /// Get the environment context for a file URI.
    /// This provides a convenient method to get the WorkspaceContext needed for env var resolution.
    pub fn get_env_context(&self, uri: &tower_lsp::lsp_types::Url) -> Option<abundantis::WorkspaceContext> {
        let file_path = uri.to_file_path().ok()?;
        let workspace = self.core.workspace.read();
        workspace.context_for_file(&file_path)
    }

    /// Create a new ServerState with workspace indexing support.
    pub fn with_indexing(
        document_manager: Arc<DocumentManager>,
        languages: Arc<LanguageRegistry>,
        core: Arc<Abundantis>,
        config: Arc<ConfigManager>,
        query_engine: Arc<QueryEngine>,
        workspace_root: PathBuf,
    ) -> Self {
        let workspace_index = Arc::new(WorkspaceIndex::new());
        let module_resolver = Arc::new(ModuleResolver::new(workspace_root.clone()));
        let indexer = Arc::new(WorkspaceIndexer::new(
            Arc::clone(&workspace_index),
            query_engine,
            Arc::clone(&languages),
            workspace_root,
        ));

        Self {
            document_manager,
            languages,
            core,
            config,
            workspace_index,
            indexer,
            module_resolver,
        }
    }
}
