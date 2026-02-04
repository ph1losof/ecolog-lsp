//! Server state management.
//!
//! This module provides the `ServerState` struct which acts as a thin coordinator
//! over focused service structs with single responsibilities.

use crate::analysis::{
    DocumentManager, ModuleResolver, QueryEngine, WorkspaceIndex, WorkspaceIndexer,
};
use crate::languages::LanguageRegistry;
use crate::server::config::ConfigManager;
use crate::server::services::{DocumentService, EnvService, WorkspaceService};
use abundantis::source::remote::ProviderManager;
use abundantis::Abundantis;
use std::path::PathBuf;
use std::sync::Arc;

/// Main server state that coordinates between focused services.
///
/// `ServerState` is a thin coordinator that provides access to specialized services:
/// - `DocumentService`: Document management and analysis
/// - `EnvService`: Environment variable resolution
/// - `WorkspaceService`: Workspace indexing and module resolution
///
/// For backward compatibility, the underlying components are also exposed directly
/// through public fields. New code should prefer using the service methods.
#[derive(Clone)]
pub struct ServerState {
    // Services (new pattern)
    /// Service for document management operations.
    pub documents: DocumentService,
    /// Service for environment variable resolution.
    pub env: EnvService,
    /// Service for workspace indexing and module resolution.
    pub workspace: WorkspaceService,

    // Direct access to underlying components (backward compatibility)
    /// Direct access to document manager (prefer using `documents` service).
    pub document_manager: Arc<DocumentManager>,
    /// Language registry for all supported languages.
    pub languages: Arc<LanguageRegistry>,
    /// Direct access to abundantis core (prefer using `env` service).
    pub core: Arc<Abundantis>,
    /// Configuration manager.
    pub config: Arc<ConfigManager>,
    /// Direct access to workspace index (prefer using `workspace` service).
    pub workspace_index: Arc<WorkspaceIndex>,
    /// Direct access to workspace indexer (prefer using `workspace` service).
    pub indexer: Arc<WorkspaceIndexer>,
    /// Direct access to module resolver (prefer using `workspace` service).
    pub module_resolver: Arc<ModuleResolver>,
    /// External provider manager for out-of-process providers.
    pub provider_manager: Arc<ProviderManager>,
}

impl ServerState {
    /// Creates a new ServerState with all components.
    pub fn new(
        document_manager: Arc<DocumentManager>,
        languages: Arc<LanguageRegistry>,
        core: Arc<Abundantis>,
        config: Arc<ConfigManager>,
        workspace_index: Arc<WorkspaceIndex>,
        indexer: Arc<WorkspaceIndexer>,
        module_resolver: Arc<ModuleResolver>,
        provider_manager: Arc<ProviderManager>,
    ) -> Self {
        // Create services wrapping the underlying components
        let documents = DocumentService::new(Arc::clone(&document_manager));
        let env = EnvService::new(Arc::clone(&core));
        let workspace = WorkspaceService::new(
            Arc::clone(&workspace_index),
            Arc::clone(&indexer),
            Arc::clone(&module_resolver),
        );

        Self {
            // Services
            documents,
            env,
            workspace,
            // Direct access (backward compatibility)
            document_manager,
            languages,
            core,
            config,
            workspace_index,
            indexer,
            module_resolver,
            provider_manager,
        }
    }

    /// Gets the workspace context for a file URI.
    pub fn get_env_context(
        &self,
        uri: &tower_lsp::lsp_types::Url,
    ) -> Option<abundantis::WorkspaceContext> {
        let file_path = uri.to_file_path().ok()?;
        self.env.get_context_for_file(&file_path)
    }

    /// Creates a ServerState with indexing support.
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

        // Create provider manager with default config
        let providers_config = abundantis::config::ProvidersConfig::default();
        let provider_manager = Arc::new(ProviderManager::new(providers_config));

        Self::new(
            document_manager,
            languages,
            core,
            config,
            workspace_index,
            indexer,
            module_resolver,
            provider_manager,
        )
    }
}
