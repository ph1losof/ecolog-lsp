//! Workspace service for managing workspace index and indexing.
//!
//! This service wraps `WorkspaceIndex` and `WorkspaceIndexer` and provides
//! a focused interface for workspace-related operations with ~80% cohesion.

use crate::analysis::workspace_index::{FileIndexEntry, IndexStats, WorkspaceIndex};
use crate::analysis::{ModuleResolver, WorkspaceIndexer};
use crate::server::config::EcologConfig;
use crate::types::FileExportEntry;
use compact_str::CompactString;
use rustc_hash::FxHashSet;
use std::path::Path;
use std::sync::Arc;
use tower_lsp::lsp_types::Url;

/// Service for workspace management operations.
///
/// Provides a cohesive interface for all workspace-related operations including:
/// - Querying the workspace index for env var files
/// - Managing workspace indexing
/// - Module resolution caching
/// - File dependency tracking
pub struct WorkspaceService {
    index: Arc<WorkspaceIndex>,
    indexer: Arc<WorkspaceIndexer>,
    module_resolver: Arc<ModuleResolver>,
}

impl WorkspaceService {
    /// Creates a new WorkspaceService.
    pub fn new(
        index: Arc<WorkspaceIndex>,
        indexer: Arc<WorkspaceIndexer>,
        module_resolver: Arc<ModuleResolver>,
    ) -> Self {
        Self {
            index,
            indexer,
            module_resolver,
        }
    }

    /// Returns a reference to the underlying WorkspaceIndex.
    #[inline]
    pub fn index(&self) -> &Arc<WorkspaceIndex> {
        &self.index
    }

    /// Returns a reference to the underlying WorkspaceIndexer.
    #[inline]
    pub fn indexer(&self) -> &Arc<WorkspaceIndexer> {
        &self.indexer
    }

    /// Returns a reference to the module resolver.
    #[inline]
    pub fn module_resolver(&self) -> &Arc<ModuleResolver> {
        &self.module_resolver
    }

    // =========================================================================
    // Index Query Methods
    // =========================================================================

    /// Gets all files that reference a specific env var.
    pub fn files_for_env_var(&self, name: &str) -> Vec<Url> {
        self.index.files_for_env_var(name)
    }

    /// Gets all env vars known in the workspace.
    pub fn all_env_vars(&self) -> Vec<CompactString> {
        self.index.all_env_vars()
    }

    /// Gets all env vars exported by files in the workspace.
    pub fn all_exported_env_vars(&self) -> Vec<CompactString> {
        self.index.all_exported_env_vars()
    }

    /// Gets files that export a specific env var.
    pub fn files_exporting_env_var(&self, name: &str) -> Vec<Url> {
        self.index.files_exporting_env_var(name)
    }

    /// Checks if a file is indexed.
    pub fn is_file_indexed(&self, uri: &Url) -> bool {
        self.index.is_file_indexed(uri)
    }

    /// Gets env vars referenced in a specific file.
    pub fn env_vars_in_file(&self, uri: &Url) -> Option<FxHashSet<CompactString>> {
        self.index.env_vars_in_file(uri)
    }

    /// Gets exports for a file.
    pub fn get_exports(&self, uri: &Url) -> Option<FileExportEntry> {
        self.index.get_exports(uri)
    }

    /// Checks if a file has exports.
    pub fn has_exports(&self, uri: &Url) -> bool {
        self.index.has_exports(uri)
    }

    /// Gets index statistics.
    pub fn stats(&self) -> IndexStats {
        self.index.stats()
    }

    // =========================================================================
    // Index Mutation Methods
    // =========================================================================

    /// Updates a file entry in the index.
    pub fn update_file(&self, uri: &Url, entry: FileIndexEntry) {
        self.index.update_file(uri, entry);
    }

    /// Removes a file from the index.
    pub fn remove_file(&self, uri: &Url) {
        self.index.remove_file(uri);
    }

    /// Updates exports for a file.
    pub fn update_exports(&self, uri: &Url, exports: FileExportEntry) {
        self.index.update_exports(uri, exports);
    }

    /// Clears the entire index.
    pub fn clear(&self) {
        self.index.clear();
    }

    // =========================================================================
    // Module Resolution Cache Methods
    // =========================================================================

    /// Gets a cached module resolution result.
    pub fn cached_module_resolution(
        &self,
        importer: &Url,
        specifier: &str,
    ) -> Option<Option<Url>> {
        self.index.cached_module_resolution(importer, specifier)
    }

    /// Caches a module resolution result.
    pub fn cache_module_resolution(
        &self,
        importer: &Url,
        specifier: &str,
        resolved: Option<Url>,
    ) {
        self.index.cache_module_resolution(importer, specifier, resolved);
    }

    /// Invalidates module resolution cache for a changed file.
    pub fn invalidate_resolution_cache(&self, changed_uri: &Url) {
        self.index.invalidate_resolution_cache(changed_uri);
    }

    /// Gets the module resolution cache size.
    pub fn module_cache_len(&self) -> usize {
        self.index.module_cache_len()
    }

    // =========================================================================
    // Dependency Graph Methods
    // =========================================================================

    /// Gets files that a given file imports from.
    pub fn get_dependencies(&self, uri: &Url) -> Vec<Url> {
        self.index.get_dependencies(uri)
    }

    /// Gets files that import a given file.
    pub fn get_dependents(&self, uri: &Url) -> Vec<Url> {
        self.index.get_dependents(uri)
    }

    /// Updates the dependency graph for a file.
    pub fn update_dependency_graph(&self, file_uri: &Url, dependencies: Vec<Url>) {
        self.index.update_dependency_graph(file_uri, dependencies);
    }

    /// Invalidates caches for a file change and marks dependents as dirty.
    pub fn invalidate_for_file_change(&self, changed_uri: &Url) {
        self.index.invalidate_for_file_change(changed_uri);
    }

    /// Gets all dirty files that need re-analysis.
    pub fn get_dirty_files(&self) -> Vec<Url> {
        self.index.get_dirty_files()
    }

    /// Clears the dirty flag for a file.
    pub fn clear_dirty(&self, uri: &Url) {
        self.index.clear_dirty(uri);
    }

    /// Checks if there are dirty files.
    pub fn has_dirty_files(&self) -> bool {
        self.index.has_dirty_files()
    }

    // =========================================================================
    // Indexer Methods
    // =========================================================================

    /// Starts indexing the workspace.
    pub async fn index_workspace(&self, config: &EcologConfig) -> anyhow::Result<()> {
        self.indexer.index_workspace(&config.workspace.env_files).await
    }

    /// Handles a file change event.
    pub async fn on_file_changed(&self, uri: &Url, config: &EcologConfig) {
        self.indexer.on_file_changed(uri, &config.workspace.env_files).await;
    }

    /// Handles a file deletion event.
    pub fn on_file_deleted(&self, uri: &Url) {
        self.indexer.on_file_deleted(uri);
    }

    /// Checks if a file needs re-indexing.
    pub async fn needs_reindex(&self, uri: &Url) -> bool {
        self.indexer.needs_reindex(uri).await
    }

    /// Gets the workspace root path.
    pub fn workspace_root(&self) -> &Path {
        self.indexer.workspace_root()
    }

    // =========================================================================
    // Indexing State Methods
    // =========================================================================

    /// Checks if indexing is in progress.
    pub fn is_indexing(&self) -> bool {
        self.index.is_indexing()
    }

    /// Gets the indexing progress percentage.
    pub fn indexing_progress(&self) -> u8 {
        self.index.indexing_progress()
    }
}

impl Clone for WorkspaceService {
    fn clone(&self) -> Self {
        Self {
            index: Arc::clone(&self.index),
            indexer: Arc::clone(&self.indexer),
            module_resolver: Arc::clone(&self.module_resolver),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::QueryEngine;
    use crate::languages::javascript::JavaScript;
    use crate::languages::LanguageRegistry;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn create_test_service() -> WorkspaceService {
        let index = Arc::new(WorkspaceIndex::new());
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(JavaScript));
        let languages = Arc::new(registry);
        let query_engine = Arc::new(QueryEngine::new());
        let workspace_root = PathBuf::from("/test");
        let indexer = Arc::new(WorkspaceIndexer::new(
            Arc::clone(&index),
            query_engine,
            languages,
            workspace_root.clone(),
        ));
        let module_resolver = Arc::new(ModuleResolver::new(workspace_root));

        WorkspaceService::new(index, indexer, module_resolver)
    }

    fn url(path: &str) -> Url {
        Url::parse(&format!("file://{}", path)).unwrap()
    }

    fn make_entry(env_vars: &[&str]) -> FileIndexEntry {
        FileIndexEntry {
            mtime: SystemTime::now(),
            env_vars: env_vars.iter().map(|s| CompactString::from(*s)).collect(),
            is_env_file: false,
            path: PathBuf::from("/test"),
        }
    }

    #[test]
    fn test_update_and_query_file() {
        let service = create_test_service();
        let uri = url("/test.js");

        service.update_file(&uri, make_entry(&["API_KEY", "DB_URL"]));

        let files = service.files_for_env_var("API_KEY");
        assert_eq!(files.len(), 1);
        assert!(files.contains(&uri));
    }

    #[test]
    fn test_remove_file() {
        let service = create_test_service();
        let uri = url("/test.js");

        service.update_file(&uri, make_entry(&["API_KEY"]));
        assert!(!service.files_for_env_var("API_KEY").is_empty());

        service.remove_file(&uri);
        assert!(service.files_for_env_var("API_KEY").is_empty());
    }

    #[test]
    fn test_all_env_vars() {
        let service = create_test_service();

        service.update_file(&url("/a.js"), make_entry(&["VAR1", "VAR2"]));
        service.update_file(&url("/b.js"), make_entry(&["VAR3"]));

        let vars = service.all_env_vars();
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn test_stats() {
        let service = create_test_service();

        service.update_file(&url("/a.js"), make_entry(&["VAR1"]));
        service.update_file(&url("/b.js"), make_entry(&["VAR2"]));

        let stats = service.stats();
        assert_eq!(stats.total_files, 2);
        assert_eq!(stats.total_env_vars, 2);
    }

    #[test]
    fn test_module_resolution_cache() {
        let service = create_test_service();
        let importer = url("/app.js");
        let resolved = url("/config.js");

        assert!(service.cached_module_resolution(&importer, "./config").is_none());

        service.cache_module_resolution(&importer, "./config", Some(resolved.clone()));

        let cached = service.cached_module_resolution(&importer, "./config");
        assert_eq!(cached, Some(Some(resolved)));
    }

    #[test]
    fn test_dependency_graph() {
        let service = create_test_service();
        let app = url("/app.js");
        let config = url("/config.js");

        service.update_dependency_graph(&app, vec![config.clone()]);

        let deps = service.get_dependencies(&app);
        assert!(deps.contains(&config));

        let dependents = service.get_dependents(&config);
        assert!(dependents.contains(&app));
    }

    #[test]
    fn test_dirty_files() {
        let service = create_test_service();
        let app = url("/app.js");
        let config = url("/config.js");

        service.update_dependency_graph(&app, vec![config.clone()]);
        service.invalidate_for_file_change(&config);

        assert!(service.has_dirty_files());
        let dirty = service.get_dirty_files();
        assert!(dirty.contains(&app));

        service.clear_dirty(&app);
        assert!(!service.has_dirty_files());
    }

    #[test]
    fn test_clone() {
        let service = create_test_service();
        let uri = url("/test.js");

        service.update_file(&uri, make_entry(&["VAR1"]));

        let cloned = service.clone();
        assert!(!cloned.files_for_env_var("VAR1").is_empty());
    }
}
