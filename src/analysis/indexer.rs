




use crate::analysis::workspace_index::{FileIndexEntry, WorkspaceIndex};
use crate::analysis::{AnalysisMode, AnalysisPipeline, BindingGraph, BindingResolver, QueryEngine};
use crate::languages::LanguageRegistry;
use crate::server::config::{CompiledEnvPatterns, IndexingConfig};
use crate::types::{
    ExportResolution, FileExportEntry, ImportContext, KorniEntryExt, SymbolId, SymbolOrigin,
};
use anyhow::Result;
use compact_str::CompactString;
use futures::stream::StreamExt;
use korni::ParseOptions;
use rustc_hash::FxHashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::SystemTime;
use tower_lsp::lsp_types::Url;
use tracing::{debug, info, warn};

/// A file found by the workspace walk, together with the mtime observed during
/// the walk so indexing does not have to `stat` it a second time.
struct DiscoveredFile {
    path: PathBuf,
    mtime: Option<SystemTime>,
}








pub struct WorkspaceIndexer {
    
    workspace_index: Arc<WorkspaceIndex>,

    
    query_engine: Arc<QueryEngine>,

    
    languages: Arc<LanguageRegistry>,

    
    workspace_root: PathBuf,
}

impl WorkspaceIndexer {
    
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

    
    
    

    
    
    
    
    pub async fn index_workspace(
        &self,
        env_files: &[CompactString],
        indexing_config: &IndexingConfig,
    ) -> Result<()> {
        info!("Starting workspace indexing at {:?}", self.workspace_root);

        self.workspace_index.set_indexing(true);

        let discovery_start = std::time::Instant::now();
        let files = self.discover_files(env_files, indexing_config).await;
        let file_count = files.len();
        info!(
            "Discovered {} files to index in {:?}",
            file_count,
            discovery_start.elapsed()
        );

        self.workspace_index.set_total_files(file_count);

        if file_count == 0 {
            self.workspace_index.set_indexing(false);
            return Ok(());
        }

        // Compile the env-file globs once for the whole run instead of once per file.
        let env_patterns = Arc::new(CompiledEnvPatterns::compile(env_files));

        // Tree-sitter parsing is CPU-bound, so cap in-flight work below the core
        // count: the editor must stay responsive while indexing runs.
        let parallelism = std::env::var("ECOLOG_INDEX_PARALLELISM")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or_else(|| indexing_config.resolved_parallelism());

        let success_count = Arc::new(AtomicUsize::new(0));
        let error_count = Arc::new(AtomicUsize::new(0));

        // `buffer_unordered` keeps at most `parallelism` tasks alive at a time.
        // Spawning every file up front instead built a JoinHandle per file and
        // held them all until the run finished.
        futures::stream::iter(files)
            .map(|file| {
                let indexer = self.clone_for_task();
                let env_patterns = Arc::clone(&env_patterns);
                let success_count = Arc::clone(&success_count);
                let error_count = Arc::clone(&error_count);

                tokio::spawn(async move {
                    let result = indexer
                        .index_file_inner(&file.path, &env_patterns, file.mtime)
                        .await;

                    match result {
                        Ok(()) => {
                            success_count.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(e) => {
                            debug!("Failed to index file: {}", e);
                            error_count.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    indexer.workspace_index.increment_indexed();
                })
            })
            .buffer_unordered(parallelism)
            .for_each(|joined| async move {
                if let Err(e) = joined {
                    warn!("Task panicked: {}", e);
                }
            })
            .await;

        let success_count = success_count.load(Ordering::Relaxed);
        let error_count = error_count.load(Ordering::Relaxed);

        self.workspace_index.set_indexing(false);

        info!(
            "Workspace indexing complete: {} succeeded, {} failed",
            success_count, error_count
        );

        Ok(())
    }

    /// Walks the workspace and returns the files worth indexing.
    ///
    /// Runs on a blocking thread pool because `ignore`'s parallel walker is
    /// synchronous and directory traversal is IO-bound.
    async fn discover_files(
        &self,
        env_files: &[CompactString],
        indexing_config: &IndexingConfig,
    ) -> Vec<DiscoveredFile> {
        let extensions: FxHashSet<&'static str> = self
            .languages
            .all_languages()
            .iter()
            .flat_map(|l| l.extensions())
            .copied()
            .collect();

        let env_patterns = CompiledEnvPatterns::compile(env_files);

        let workspace_root = self.workspace_root.clone();
        let indexing_config = indexing_config.clone();

        tokio::task::spawn_blocking(move || {
            Self::walk_workspace(&workspace_root, &indexing_config, &extensions, &env_patterns)
        })
        .await
        .unwrap_or_default()
    }

    fn walk_workspace(
        workspace_root: &Path,
        indexing_config: &IndexingConfig,
        extensions: &FxHashSet<&'static str>,
        env_patterns: &CompiledEnvPatterns,
    ) -> Vec<DiscoveredFile> {
        let mut builder = ignore::WalkBuilder::new(workspace_root);
        builder
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .require_git(false);

        if indexing_config.max_depth > 0 {
            builder.max_depth(Some(indexing_config.max_depth));
        }

        if !indexing_config.exclude.is_empty() {
            let mut overrides = ignore::overrides::OverrideBuilder::new(workspace_root);
            for pattern in &indexing_config.exclude {
                // Exclusions are plain directory names (`node_modules`, `target`),
                // so match them at any depth rather than only at the workspace
                // root -- otherwise nested copies in a monorepo get walked.
                let _ = overrides.add(&format!("!**/{}/**", pattern));
                let _ = overrides.add(&format!("!**/{}", pattern));
            }
            if let Ok(built) = overrides.build() {
                builder.overrides(built);
            }
        }

        let max_files = indexing_config.max_files;
        let max_file_size = indexing_config.max_file_size;
        let mut files = Vec::new();

        for entry in builder.build().flatten() {
            if max_files > 0 && files.len() >= max_files {
                warn!(
                    "Reached max_files limit ({}). Increase [indexing].max_files or add exclusions in ecolog.toml",
                    max_files
                );
                break;
            }

            // `file_type()` comes from the directory read, so it is free;
            // `path.is_file()` cost an extra stat for every entry in the tree.
            // Symlinks are resolved below, since the walker does not follow them
            // and their entry type says nothing about the target.
            let is_symlink = match entry.file_type() {
                Some(ft) if ft.is_file() => false,
                Some(ft) if ft.is_symlink() => true,
                _ => continue,
            };

            let path = entry.path();

            // Cheap name checks first: only candidates are worth a stat.
            let is_candidate = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|ext| extensions.contains(ext))
                || path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|name| env_patterns.matches(name));

            if !is_candidate {
                continue;
            }

            let metadata = if is_symlink {
                // Follow the link to learn whether it points at a file at all.
                std::fs::metadata(path).ok()
            } else {
                entry.metadata().ok()
            };

            if is_symlink && !metadata.as_ref().is_some_and(|m| m.is_file()) {
                continue;
            }

            if max_file_size > 0 {
                if let Some(metadata) = &metadata {
                    if metadata.len() > max_file_size {
                        debug!("Skipping large file {:?} ({} bytes)", path, metadata.len());
                        continue;
                    }
                }
            }

            files.push(DiscoveredFile {
                path: path.to_path_buf(),
                mtime: metadata.and_then(|m| m.modified().ok()),
            });
        }

        files
    }

    
    pub async fn index_file(&self, path: &Path, env_files: &[CompactString]) -> Result<()> {
        let env_patterns = CompiledEnvPatterns::compile(env_files);
        self.index_file_inner(path, &env_patterns, None).await
    }

    /// Indexes a single file.
    ///
    /// `known_mtime` lets callers reuse the modification time observed during
    /// workspace discovery instead of paying a second `stat`.
    async fn index_file_inner(
        &self,
        path: &Path,
        env_patterns: &CompiledEnvPatterns,
        known_mtime: Option<SystemTime>,
    ) -> Result<()> {
        let uri = Url::from_file_path(path)
            .map_err(|_| anyhow::anyhow!("Invalid file path: {:?}", path))?;

        let content = tokio::fs::read_to_string(path).await?;
        let mtime = match known_mtime {
            Some(mtime) => mtime,
            None => tokio::fs::metadata(path).await?.modified()?,
        };

        let is_env_file = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|name| env_patterns.matches(name));

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

        
        // Only files that actually export something need an entry; storing empty
        // ones grows the export index by one record per file in the workspace.
        if let Some(exports) = exports {
            if !exports.is_empty() {
                self.workspace_index.update_exports(&uri, exports);
            }
        }

        Ok(())
    }


    
    fn extract_env_vars_from_env_file(&self, content: &str) -> FxHashSet<CompactString> {
        let entries = korni::parse_with_options(content, ParseOptions::full());

        entries
            .into_iter()
            .filter_map(|e| e.into_valid_pair())
            .map(|kv| CompactString::from(kv.key.as_ref()))
            .collect()
    }

    
    async fn extract_env_vars_and_exports_from_code_file(
        &self,
        uri: &Url,
        content: &str,
    ) -> Result<(FxHashSet<CompactString>, FileExportEntry)> {
        
        let lang = self
            .languages
            .get_for_uri(uri)
            .ok_or_else(|| anyhow::anyhow!("Unknown language for {:?}", uri))?;

        
        let tree = self
            .query_engine
            .parse(lang.as_ref(), content, None)
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to parse {:?}", uri))?;

        let source = content.as_bytes();

        // Indexing only needs env vars and export resolutions, so skip the
        // usage/property-access machinery and the positional interval trees
        // that interactive requests rely on.
        let binding_graph = AnalysisPipeline::analyze_with_mode(
            &self.query_engine,
            lang.as_ref(),
            &tree,
            source,
            &ImportContext::default(),
            AnalysisMode::Index,
        )
        .await;

        
        let env_vars = self.collect_env_vars(&binding_graph);

        
        let mut exports = self
            .query_engine
            .extract_exports(lang.as_ref(), &tree, source)
            .await;

        
        self.resolve_export_resolutions(&mut exports, &binding_graph);

        Ok((env_vars, exports))
    }

    
    fn collect_env_vars(&self, graph: &BindingGraph) -> FxHashSet<CompactString> {
        let resolver = BindingResolver::new(graph);
        resolver.all_env_vars().into_iter().collect()
    }


    
    
    
    
    fn resolve_export_resolutions(&self, exports: &mut FileExportEntry, graph: &BindingGraph) {
        
        
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
                    
                    if let Some((_, Some(_canonical))) =
                        resolve_symbol_chain(graph, *source, depth + 1)
                    {
                        
                        
                        Some((Some(key.clone()), None))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }

        
        let resolve_symbol = |local_name: &str| -> ExportResolution {
            let resolver = BindingResolver::new(graph);

            
            if let Some(kind) = resolver.get_binding_kind(local_name) {
                if kind == crate::types::BindingKind::Object {
                    
                    for symbol in graph
                        .lookup_symbols_by_name(local_name)
                        .filter_map(|id| graph.get_symbol(id))
                    {
                        if symbol.is_valid {
                            if let SymbolOrigin::EnvObject { canonical_name } = &symbol.origin {
                                return ExportResolution::EnvObject {
                                    canonical_name: canonical_name.clone(),
                                };
                            }
                        }
                    }
                    
                    return ExportResolution::EnvObject {
                        canonical_name: local_name.into(),
                    };
                }
            }

            
            for symbol in graph
                .lookup_symbols_by_name(local_name)
                .filter_map(|id| graph.get_symbol(id))
            {
                if symbol.is_valid {
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
                            
                            if let Some((_, Some(_canonical))) =
                                resolve_symbol_chain(graph, *source, 0)
                            {
                                
                                
                                return ExportResolution::EnvVar { name: key.clone() };
                            }
                        }
                        SymbolOrigin::Unknown
                        | SymbolOrigin::UnresolvedSymbol { .. }
                        | SymbolOrigin::UnresolvedDestructure { .. }
                        | SymbolOrigin::Unresolvable => {
                            
                        }
                    }
                }
            }

            ExportResolution::Unknown
        };

        
        for export in exports.named_exports.values_mut() {
            if matches!(export.resolution, ExportResolution::Unknown) {
                
                
                
                
                
                
                
                
                
                
                
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

        
        if let Some(ref mut default) = exports.default_export {
            if matches!(default.resolution, ExportResolution::Unknown) {
                if let Some(ref local_name) = default.local_name {
                    default.resolution = resolve_symbol(local_name.as_str());
                } else if default.exported_name != "default" {
                    
                    default.resolution = resolve_symbol(default.exported_name.as_str());
                }
            }
        }
    }

    
    
    

    
    pub async fn on_file_changed(&self, uri: &Url, env_files: &[CompactString]) {

        self.workspace_index.invalidate_resolution_cache(uri);

        if let Ok(path) = uri.to_file_path() {
            let env_patterns = CompiledEnvPatterns::compile(env_files);
            if let Err(e) = self.index_file_inner(&path, &env_patterns, None).await {
                debug!("Failed to re-index {:?}: {}", uri, e);
            }
        }
    }

    
    pub fn on_file_deleted(&self, uri: &Url) {
        debug!("Removing {:?} from index", uri);

        
        self.workspace_index.invalidate_resolution_cache(uri);

        
        self.workspace_index.remove_file(uri);
    }

    
    pub async fn needs_reindex(&self, uri: &Url) -> bool {
        if let Ok(path) = uri.to_file_path() {
            if let Ok(metadata) = tokio::fs::metadata(&path).await {
                if let Ok(mtime) = metadata.modified() {
                    return self.workspace_index.is_file_stale(uri, mtime);
                }
            }
        }
        true 
    }

    
    
    

    
    
    fn clone_for_task(&self) -> Self {
        Self {
            workspace_index: Arc::clone(&self.workspace_index),
            query_engine: Arc::clone(&self.query_engine),
            languages: Arc::clone(&self.languages),
            workspace_root: self.workspace_root.clone(),
        }
    }

    
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    
    pub fn index(&self) -> &Arc<WorkspaceIndex> {
        &self.workspace_index
    }
}





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

    fn default_env_files() -> Vec<CompactString> {
        vec![
            CompactString::new(".env"),
            CompactString::new(".env.*"),
        ]
    }

    fn default_indexing_config() -> IndexingConfig {
        IndexingConfig {
            exclude: vec![],
            ..IndexingConfig::default()
        }
    }

    #[tokio::test]
    async fn test_index_env_file() {
        let temp_dir = TempDir::new().unwrap();
        create_file(
            temp_dir.path(),
            ".env",
            "API_KEY=secret\nDB_URL=postgres://localhost/db",
        );

        let indexer = setup_test_indexer(temp_dir.path()).await;
        indexer.index_workspace(&default_env_files(), &default_indexing_config()).await.unwrap();

        let stats = indexer.index().stats();
        assert_eq!(stats.total_files, 1);
        assert_eq!(stats.env_files, 1);

        
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
        indexer.index_workspace(&default_env_files(), &default_indexing_config()).await.unwrap();

        let stats = indexer.index().stats();
        assert_eq!(stats.total_files, 1);
        assert_eq!(stats.env_files, 0);

        
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
        indexer.index_workspace(&default_env_files(), &default_indexing_config()).await.unwrap();

        let stats = indexer.index().stats();
        assert_eq!(stats.total_files, 4);
        assert_eq!(stats.env_files, 1);

        
        let api_key_files = indexer.index().files_for_env_var("API_KEY");
        assert_eq!(api_key_files.len(), 4);
    }

    #[tokio::test]
    async fn test_incremental_update() {
        let temp_dir = TempDir::new().unwrap();
        create_file(temp_dir.path(), "test.js", "const x = process.env.VAR1;");

        let indexer = setup_test_indexer(temp_dir.path()).await;
        let env_files = default_env_files();
        indexer.index_workspace(&env_files, &default_indexing_config()).await.unwrap();


        assert!(!indexer.index().files_for_env_var("VAR1").is_empty());
        assert!(indexer.index().files_for_env_var("VAR2").is_empty());


        create_file(temp_dir.path(), "test.js", "const x = process.env.VAR2;");
        let uri = Url::from_file_path(temp_dir.path().join("test.js")).unwrap();
        indexer.on_file_changed(&uri, &env_files).await;


        assert!(indexer.index().files_for_env_var("VAR1").is_empty());
        assert!(!indexer.index().files_for_env_var("VAR2").is_empty());
    }

    #[tokio::test]
    async fn test_file_deletion() {
        let temp_dir = TempDir::new().unwrap();
        create_file(temp_dir.path(), "test.js", "const x = process.env.VAR1;");

        let indexer = setup_test_indexer(temp_dir.path()).await;
        indexer.index_workspace(&default_env_files(), &default_indexing_config()).await.unwrap();

        assert!(!indexer.index().files_for_env_var("VAR1").is_empty());

        
        let uri = Url::from_file_path(temp_dir.path().join("test.js")).unwrap();
        indexer.on_file_deleted(&uri);

        assert!(indexer.index().files_for_env_var("VAR1").is_empty());
    }

    #[tokio::test]
    async fn test_respects_gitignore() {
        let temp_dir = TempDir::new().unwrap();

        
        create_file(temp_dir.path(), ".gitignore", "ignored/\n*.ignored.js");

        
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
        indexer.index_workspace(&default_env_files(), &default_indexing_config()).await.unwrap();

        
        assert!(!indexer.index().files_for_env_var("INCLUDED").is_empty());
        assert!(indexer.index().files_for_env_var("IGNORED").is_empty());
        assert!(indexer.index().files_for_env_var("ALSO_IGNORED").is_empty());
    }
}
