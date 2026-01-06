use crate::analysis::{
    DocumentManager, ModuleResolver, QueryEngine, WorkspaceIndex, WorkspaceIndexer,
};
use crate::languages::LanguageRegistry;
use abundantis::Abundantis;
use shelter::Masker;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::server::config::ConfigManager;

#[derive(Clone)]
pub struct ServerState {
    pub document_manager: Arc<DocumentManager>,
    pub languages: Arc<LanguageRegistry>,
    pub core: Arc<Abundantis>,
    pub masker: Arc<Mutex<Masker>>,
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
        masker: Arc<Mutex<Masker>>,
        config: Arc<ConfigManager>,
        workspace_index: Arc<WorkspaceIndex>,
        indexer: Arc<WorkspaceIndexer>,
        module_resolver: Arc<ModuleResolver>,
    ) -> Self {
        Self {
            document_manager,
            languages,
            core,
            masker,
            config,
            workspace_index,
            indexer,
            module_resolver,
        }
    }

    /// Create a new ServerState with workspace indexing support.
    pub fn with_indexing(
        document_manager: Arc<DocumentManager>,
        languages: Arc<LanguageRegistry>,
        core: Arc<Abundantis>,
        masker: Arc<Mutex<Masker>>,
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
            masker,
            config,
            workspace_index,
            indexer,
            module_resolver,
        }
    }
}
