//! Workspace Indexer - Background indexing for environment variable references.
//!
//! This module provides the indexer that scans the workspace for files,
//! analyzes them, and populates the WorkspaceIndex.

use crate::analysis::workspace_index::{FileIndexEntry, WorkspaceIndex};
use crate::analysis::{AnalysisPipeline, BindingGraph, BindingResolver, QueryEngine};
use crate::languages::LanguageRegistry;
use crate::server::config::EcologConfig;
use crate::types::{ExportResolution, FileExportEntry, ImportContext, SymbolId, SymbolOrigin};
use anyhow::Result;
use compact_str::CompactString;
use korni::ParseOptions;
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tower_lsp::lsp_types::Url;
use tracing::{debug, info, warn};

/// Workspace indexer for background file scanning and analysis.
///
/// Responsible for:
/// - Discovering files in the workspace
/// - Parsing and analyzing files in parallel
/// - Populating the WorkspaceIndex
/// - Handling incremental updates
pub struct WorkspaceIndexer {
    /// The workspace index to populate
    workspace_index: Arc<WorkspaceIndex>,

    /// Query engine for parsing
    query_engine: Arc<QueryEngine>,

    /// Language registry for language detection
    languages: Arc<LanguageRegistry>,

    /// Workspace root path
    workspace_root: PathBuf,
}

impl WorkspaceIndexer {
    /// Create a new workspace indexer.
    pub fn new(
        workspace_index: Arc<WorkspaceIndex>,
        query_engine: Arc<QueryEngine>,
        languages: Arc<LanguageRegistry>,
        workspace_root: PathBuf,
    ) -> Self {
        Self {
            workspace_index,
            query_engine,
            languages,
            workspace_root,
        }
    }

    // =========================================================================
    // Full Workspace Indexing
    // =========================================================================

    /// Index the entire workspace.
    ///
    /// This discovers all relevant files and indexes them in parallel.
    /// Progress can be monitored via `WorkspaceIndex::indexing_progress()`.
    pub async fn index_workspace(&self, config: &EcologConfig) -> Result<()> {
        info!("Starting workspace indexing at {:?}", self.workspace_root);

        self.workspace_index.set_indexing(true);

        // Step 1: Discover files
        let files = self.discover_files(config).await;
        let file_count = files.len();
        info!("Discovered {} files to index", file_count);

        self.workspace_index.set_total_files(file_count);

        if file_count == 0 {
            self.workspace_index.set_indexing(false);
            return Ok(());
        }

        // Step 2: Index files in parallel
        let semaphore = Arc::new(Semaphore::new(num_cpus::get()));
        let mut handles = Vec::with_capacity(file_count);

        for file_path in files {
            let permit = semaphore.clone().acquire_owned().await?;
            let indexer = self.clone_for_task();
            let config_clone = config.clone();

            handles.push(tokio::spawn(async move {
                let result = indexer.index_file(&file_path, &config_clone).await;
                drop(permit);
                result
            }));
        }

        // Step 3: Await all and collect results
        let mut success_count = 0;
        let mut error_count = 0;

        for handle in handles {
            match handle.await {
                Ok(Ok(())) => {
                    success_count += 1;
                    self.workspace_index.increment_indexed();
                }
                Ok(Err(e)) => {
                    debug!("Failed to index file: {}", e);
                    error_count += 1;
                    self.workspace_index.increment_indexed();
                }
                Err(e) => {
                    warn!("Task panicked: {}", e);
                    error_count += 1;
                }
            }
        }

        self.workspace_index.set_indexing(false);

        info!(
            "Workspace indexing complete: {} succeeded, {} failed",
            success_count, error_count
        );

        Ok(())
    }

    /// Discover files to index in the workspace.
    async fn discover_files(&self, config: &EcologConfig) -> Vec<PathBuf> {
        let mut files = Vec::new();

        // Get all supported extensions from languages
        let extensions: Vec<&str> = self
            .languages
            .all_languages()
            .iter()
            .flat_map(|l| l.extensions())
            .copied()
            .collect();

        // Get env file patterns from config
        let env_patterns: Vec<glob::Pattern> = config
            .workspace
            .env_files
            .iter()
            .filter_map(|p| glob::Pattern::new(p).ok())
            .collect();

        // Walk directory respecting .gitignore
        let walker = ignore::WalkBuilder::new(&self.workspace_root)
            .hidden(false) // Include hidden files (like .env)
            .git_ignore(true) // Respect .gitignore
            .git_global(true) // Respect global gitignore
            .git_exclude(true) // Respect .git/info/exclude
            .require_git(false) // Respect .gitignore even without .git directory
            .build();

        for entry in walker.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            // Check if it's a code file (by extension)
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    files.push(path.to_path_buf());
                    continue;
                }
            }

            // Check if it's an env file (by pattern)
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if env_patterns.iter().any(|p| p.matches(name)) {
                    files.push(path.to_path_buf());
                }
            }
        }

        files
    }

    // =========================================================================
    // Single File Indexing
    // =========================================================================

    /// Index a single file.
    pub async fn index_file(&self, path: &Path, config: &EcologConfig) -> Result<()> {
        let uri = Url::from_file_path(path)
            .map_err(|_| anyhow::anyhow!("Invalid file path: {:?}", path))?;

        let content = tokio::fs::read_to_string(path).await?;
        let mtime = tokio::fs::metadata(path).await?.modified()?;

        let is_env_file = self.is_env_file(path, config);

        let (env_vars, exports) = if is_env_file {
            (self.extract_env_vars_from_env_file(&content), None)
        } else {
            let (vars, exports) = self
                .extract_env_vars_and_exports_from_code_file(&uri, &content)
                .await?;
            (vars, Some(exports))
        };

        debug!(
            "Indexed {:?}: {} env vars, {} exports, is_env_file={}",
            path,
            env_vars.len(),
            exports
                .as_ref()
                .map(|e| e.named_exports.len() + if e.default_export.is_some() { 1 } else { 0 })
                .unwrap_or(0),
            is_env_file
        );

        self.workspace_index.update_file(
            &uri,
            FileIndexEntry {
                mtime,
                env_vars,
                is_env_file,
                path: path.to_path_buf(),
            },
        );

        // Update exports index for code files
        if let Some(exports) = exports {
            self.workspace_index.update_exports(&uri, exports);
        }

        Ok(())
    }

    /// Check if a path is an env file based on config patterns.
    fn is_env_file(&self, path: &Path, config: &EcologConfig) -> bool {
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => return false,
        };

        config.workspace.env_files.iter().any(|pattern| {
            glob::Pattern::new(pattern)
                .map(|p| p.matches(name))
                .unwrap_or(false)
        })
    }

    /// Extract env var names from a .env file.
    fn extract_env_vars_from_env_file(&self, content: &str) -> FxHashSet<CompactString> {
        let entries = korni::parse_with_options(content, ParseOptions::full());

        entries
            .into_iter()
            .filter_map(|entry| match entry {
                korni::Entry::Pair(kv) => Some(CompactString::from(kv.key.as_ref())),
                _ => None,
            })
            .collect()
    }

    /// Extract env var names and exports from a code file.
    async fn extract_env_vars_and_exports_from_code_file(
        &self,
        uri: &Url,
        content: &str,
    ) -> Result<(FxHashSet<CompactString>, FileExportEntry)> {
        // Detect language
        let lang = self
            .languages
            .get_for_uri(uri)
            .ok_or_else(|| anyhow::anyhow!("Unknown language for {:?}", uri))?;

        // Parse
        let tree = self
            .query_engine
            .parse(lang.as_ref(), content, None)
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to parse {:?}", uri))?;

        let source = content.as_bytes();

        // Analyze for env vars
        let binding_graph = AnalysisPipeline::analyze(
            &self.query_engine,
            lang.as_ref(),
            &tree,
            source,
            &ImportContext::default(),
        )
        .await;

        // Extract env vars
        let env_vars = self.collect_env_vars(&binding_graph);

        // Extract exports
        let mut exports = self
            .query_engine
            .extract_exports(lang.as_ref(), &tree, source)
            .await;

        // Resolve export resolutions using the binding graph
        self.resolve_export_resolutions(&mut exports, &binding_graph);

        Ok((env_vars, exports))
    }

    /// Collect env var names from a binding graph.
    fn collect_env_vars(&self, graph: &BindingGraph) -> FxHashSet<CompactString> {
        let resolver = BindingResolver::new(graph);
        resolver.all_env_vars().into_iter().collect()
    }

    /// Resolve export resolutions using the binding graph.
    ///
    /// For exports with Unknown resolution, checks if the exported symbol
    /// resolves to an env var through the binding graph.
    fn resolve_export_resolutions(&self, exports: &mut FileExportEntry, graph: &BindingGraph) {
        // Helper to resolve a symbol to its final env var/object
        // Returns (Option<env_var_name>, Option<canonical_name>) for EnvVar/EnvObject
        fn resolve_symbol_chain(
            graph: &BindingGraph,
            symbol_id: SymbolId,
            depth: usize,
        ) -> Option<(Option<CompactString>, Option<CompactString>)> {
            const MAX_DEPTH: usize = 20;
            if depth >= MAX_DEPTH {
                return None;
            }

            let symbol = graph.get_symbol(symbol_id)?;
            match &symbol.origin {
                SymbolOrigin::EnvVar { name } => Some((Some(name.clone()), None)),
                SymbolOrigin::EnvObject { canonical_name } => {
                    Some((None, Some(canonical_name.clone())))
                }
                SymbolOrigin::Symbol { target } => {
                    resolve_symbol_chain(graph, *target, depth + 1)
                }
                SymbolOrigin::DestructuredProperty { source, key } => {
                    // Resolve the source to see if it's an env object
                    if let Some((_, Some(_canonical))) =
                        resolve_symbol_chain(graph, *source, depth + 1)
                    {
                        // Source is an env object, so this destructured property
                        // is the env var with the key name
                        Some((Some(key.clone()), None))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }

        // Helper to find a symbol by name and resolve it
        let resolve_symbol = |local_name: &str| -> ExportResolution {
            let resolver = BindingResolver::new(graph);

            // Check if it's an object binding (env object alias)
            if let Some(kind) = resolver.get_binding_kind(local_name) {
                if kind == crate::types::BindingKind::Object {
                    // It's an env object alias - get the canonical name
                    for symbol in graph.symbols() {
                        if symbol.name.as_str() == local_name && symbol.is_valid {
                            if let SymbolOrigin::EnvObject { canonical_name } = &symbol.origin {
                                return ExportResolution::EnvObject {
                                    canonical_name: canonical_name.clone(),
                                };
                            }
                        }
                    }
                    // Fallback: use the local name as canonical
                    return ExportResolution::EnvObject {
                        canonical_name: local_name.into(),
                    };
                }
            }

            // Check symbol origins directly in the graph
            for symbol in graph.symbols() {
                if symbol.name.as_str() == local_name && symbol.is_valid {
                    match &symbol.origin {
                        SymbolOrigin::EnvVar { name } => {
                            return ExportResolution::EnvVar { name: name.clone() };
                        }
                        SymbolOrigin::EnvObject { canonical_name } => {
                            return ExportResolution::EnvObject {
                                canonical_name: canonical_name.clone(),
                            };
                        }
                        SymbolOrigin::Symbol { target } => {
                            // Follow the chain to its final resolution
                            if let Some((env_var, env_obj)) =
                                resolve_symbol_chain(graph, *target, 0)
                            {
                                if let Some(name) = env_var {
                                    return ExportResolution::EnvVar { name };
                                }
                                if let Some(canonical_name) = env_obj {
                                    return ExportResolution::EnvObject { canonical_name };
                                }
                            }
                        }
                        SymbolOrigin::DestructuredProperty { source, key } => {
                            // Resolve the source to see if it's an env object
                            if let Some((_, Some(_canonical))) =
                                resolve_symbol_chain(graph, *source, 0)
                            {
                                // Source is an env object, so this destructured property
                                // is the env var with the key name
                                return ExportResolution::EnvVar { name: key.clone() };
                            }
                        }
                        SymbolOrigin::Unknown
                        | SymbolOrigin::UnresolvedSymbol { .. }
                        | SymbolOrigin::UnresolvedDestructure { .. }
                        | SymbolOrigin::Unresolvable => {
                            // Not env-related or not yet resolved
                        }
                    }
                }
            }

            ExportResolution::Unknown
        };

        // Resolve named exports
        for export in exports.named_exports.values_mut() {
            if matches!(export.resolution, ExportResolution::Unknown) {
                // For destructured exports with alias (e.g., `export const { DB_URL: something }`):
                // - exported_name = "something" (the variable name in binding graph)
                // - local_name = "DB_URL" (the destructure key)
                // We need to look up by exported_name (the actual variable name).
                //
                // For regular aliased exports (e.g., `export { foo as bar }`):
                // - exported_name = "bar"
                // - local_name = "foo" (the original variable name)
                // We need to look up by local_name.
                //
                // Try exported_name first (for destructured exports), then local_name.
                let resolution = resolve_symbol(export.exported_name.as_str());
                export.resolution = if matches!(resolution, ExportResolution::Unknown) {
                    if let Some(ref local_name) = export.local_name {
                        resolve_symbol(local_name.as_str())
                    } else {
                        resolution
                    }
                } else {
                    resolution
                };
            }
        }

        // Resolve default export
        if let Some(ref mut default) = exports.default_export {
            if matches!(default.resolution, ExportResolution::Unknown) {
                if let Some(ref local_name) = default.local_name {
                    default.resolution = resolve_symbol(local_name.as_str());
                } else if default.exported_name != "default" {
                    // The exported_name might be the actual local name for default exports
                    default.resolution = resolve_symbol(default.exported_name.as_str());
                }
            }
        }
    }

    // =========================================================================
    // Incremental Updates
    // =========================================================================

    /// Handle a file change notification.
    pub async fn on_file_changed(&self, uri: &Url, config: &EcologConfig) {
        // Invalidate module resolution cache entries that might reference this file
        self.workspace_index.invalidate_resolution_cache(uri);

        if let Ok(path) = uri.to_file_path() {
            if let Err(e) = self.index_file(&path, config).await {
                debug!("Failed to re-index {:?}: {}", uri, e);
            }
        }
    }

    /// Handle a file deletion notification.
    pub fn on_file_deleted(&self, uri: &Url) {
        debug!("Removing {:?} from index", uri);

        // Invalidate module resolution cache
        self.workspace_index.invalidate_resolution_cache(uri);

        // Remove file from index (this also removes exports)
        self.workspace_index.remove_file(uri);
    }

    /// Check if a file needs re-indexing (mtime changed).
    pub async fn needs_reindex(&self, uri: &Url) -> bool {
        if let Ok(path) = uri.to_file_path() {
            if let Ok(metadata) = tokio::fs::metadata(&path).await {
                if let Ok(mtime) = metadata.modified() {
                    return self.workspace_index.is_file_stale(uri, mtime);
                }
            }
        }
        true // Default to reindex if can't determine
    }

    // =========================================================================
    // Helpers
    // =========================================================================

    /// Clone self for use in spawned task.
    /// Creates a lightweight clone with Arc references.
    fn clone_for_task(&self) -> Self {
        Self {
            workspace_index: Arc::clone(&self.workspace_index),
            query_engine: Arc::clone(&self.query_engine),
            languages: Arc::clone(&self.languages),
            workspace_root: self.workspace_root.clone(),
        }
    }

    /// Get workspace root.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Get reference to the workspace index.
    pub fn index(&self) -> &Arc<WorkspaceIndex> {
        &self.workspace_index
    }
}

// =========================================================================
// Unit Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::javascript::JavaScript;
    use crate::languages::python::Python;
    use crate::languages::LanguageRegistry;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    async fn setup_test_indexer(temp_dir: &Path) -> WorkspaceIndexer {
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(JavaScript));
        registry.register(Arc::new(Python));

        WorkspaceIndexer::new(
            Arc::new(WorkspaceIndex::new()),
            Arc::new(QueryEngine::new()),
            Arc::new(registry),
            temp_dir.to_path_buf(),
        )
    }

    fn create_file(dir: &Path, name: &str, content: &str) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = File::create(&path).unwrap();
        write!(f, "{}", content).unwrap();
    }

    fn default_config() -> EcologConfig {
        EcologConfig::default()
    }

    #[tokio::test]
    async fn test_index_env_file() {
        let temp_dir = TempDir::new().unwrap();
        create_file(
            temp_dir.path(),
            ".env",
            "API_KEY=secret\nDB_URL=postgres://localhost",
        );

        let indexer = setup_test_indexer(temp_dir.path()).await;
        indexer.index_workspace(&default_config()).await.unwrap();

        let stats = indexer.index().stats();
        assert_eq!(stats.total_files, 1);
        assert_eq!(stats.env_files, 1);

        // Check env vars are indexed
        assert!(!indexer.index().files_for_env_var("API_KEY").is_empty());
        assert!(!indexer.index().files_for_env_var("DB_URL").is_empty());
    }

    #[tokio::test]
    async fn test_index_js_file() {
        let temp_dir = TempDir::new().unwrap();
        create_file(
            temp_dir.path(),
            "test.js",
            "const key = process.env.API_KEY;\nconst url = process.env.DB_URL;",
        );

        let indexer = setup_test_indexer(temp_dir.path()).await;
        indexer.index_workspace(&default_config()).await.unwrap();

        let stats = indexer.index().stats();
        assert_eq!(stats.total_files, 1);
        assert_eq!(stats.env_files, 0);

        // Check env vars are indexed
        let api_key_files = indexer.index().files_for_env_var("API_KEY");
        assert_eq!(api_key_files.len(), 1);
    }

    #[tokio::test]
    async fn test_index_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        create_file(temp_dir.path(), ".env", "API_KEY=secret");
        create_file(temp_dir.path(), "a.js", "const x = process.env.API_KEY;");
        create_file(temp_dir.path(), "b.js", "const y = process.env.API_KEY;");
        create_file(
            temp_dir.path(),
            "c.py",
            "import os\nkey = os.environ['API_KEY']",
        );

        let indexer = setup_test_indexer(temp_dir.path()).await;
        indexer.index_workspace(&default_config()).await.unwrap();

        let stats = indexer.index().stats();
        assert_eq!(stats.total_files, 4);
        assert_eq!(stats.env_files, 1);

        // All 4 files should reference API_KEY
        let api_key_files = indexer.index().files_for_env_var("API_KEY");
        assert_eq!(api_key_files.len(), 4);
    }

    #[tokio::test]
    async fn test_incremental_update() {
        let temp_dir = TempDir::new().unwrap();
        create_file(temp_dir.path(), "test.js", "const x = process.env.VAR1;");

        let indexer = setup_test_indexer(temp_dir.path()).await;
        let config = default_config();
        indexer.index_workspace(&config).await.unwrap();

        // VAR1 should be indexed
        assert!(!indexer.index().files_for_env_var("VAR1").is_empty());
        assert!(indexer.index().files_for_env_var("VAR2").is_empty());

        // Update file
        create_file(temp_dir.path(), "test.js", "const x = process.env.VAR2;");
        let uri = Url::from_file_path(temp_dir.path().join("test.js")).unwrap();
        indexer.on_file_changed(&uri, &config).await;

        // VAR1 should be gone, VAR2 should be indexed
        assert!(indexer.index().files_for_env_var("VAR1").is_empty());
        assert!(!indexer.index().files_for_env_var("VAR2").is_empty());
    }

    #[tokio::test]
    async fn test_file_deletion() {
        let temp_dir = TempDir::new().unwrap();
        create_file(temp_dir.path(), "test.js", "const x = process.env.VAR1;");

        let indexer = setup_test_indexer(temp_dir.path()).await;
        indexer.index_workspace(&default_config()).await.unwrap();

        assert!(!indexer.index().files_for_env_var("VAR1").is_empty());

        // Delete file from index
        let uri = Url::from_file_path(temp_dir.path().join("test.js")).unwrap();
        indexer.on_file_deleted(&uri);

        assert!(indexer.index().files_for_env_var("VAR1").is_empty());
    }

    #[tokio::test]
    async fn test_respects_gitignore() {
        let temp_dir = TempDir::new().unwrap();

        // Create .gitignore
        create_file(temp_dir.path(), ".gitignore", "ignored/\n*.ignored.js");

        // Create files
        create_file(temp_dir.path(), "included.js", "const x = process.env.INCLUDED;");
        create_file(
            temp_dir.path(),
            "ignored/test.js",
            "const x = process.env.IGNORED;",
        );
        create_file(
            temp_dir.path(),
            "also.ignored.js",
            "const x = process.env.ALSO_IGNORED;",
        );

        let indexer = setup_test_indexer(temp_dir.path()).await;
        indexer.index_workspace(&default_config()).await.unwrap();

        // Only INCLUDED should be indexed
        assert!(!indexer.index().files_for_env_var("INCLUDED").is_empty());
        assert!(indexer.index().files_for_env_var("IGNORED").is_empty());
        assert!(indexer.index().files_for_env_var("ALSO_IGNORED").is_empty());
    }
}
