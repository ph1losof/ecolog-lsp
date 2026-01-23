




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

    
    
    

    
    
    
    
    pub async fn index_workspace(&self, config: &EcologConfig) -> Result<()> {
        info!("Starting workspace indexing at {:?}", self.workspace_root);

        self.workspace_index.set_indexing(true);

        
        let files = self.discover_files(config).await;
        let file_count = files.len();
        info!("Discovered {} files to index", file_count);

        self.workspace_index.set_total_files(file_count);

        if file_count == 0 {
            self.workspace_index.set_indexing(false);
            return Ok(());
        }

        
        
        
        let parallelism = (num_cpus::get() / 2).max(1).min(4);
        let semaphore = Arc::new(Semaphore::new(parallelism));
        let mut handles = Vec::with_capacity(file_count);

        for (i, file_path) in files.into_iter().enumerate() {
            let permit = semaphore.clone().acquire_owned().await?;
            let indexer = self.clone_for_task();
            let config_clone = config.clone();

            handles.push(tokio::spawn(async move {
                let result = indexer.index_file(&file_path, &config_clone).await;
                drop(permit);
                result
            }));

            
            if (i + 1) % 10 == 0 {
                tokio::task::yield_now().await;
            }
        }

        
        let mut success_count = 0;
        let mut error_count = 0;

        for (i, handle) in handles.into_iter().enumerate() {
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

            
            if (i + 1) % 10 == 0 {
                tokio::task::yield_now().await;
            }
        }

        self.workspace_index.set_indexing(false);

        info!(
            "Workspace indexing complete: {} succeeded, {} failed",
            success_count, error_count
        );

        Ok(())
    }

    
    async fn discover_files(&self, config: &EcologConfig) -> Vec<PathBuf> {
        let mut files = Vec::new();

        
        let extensions: Vec<&str> = self
            .languages
            .all_languages()
            .iter()
            .flat_map(|l| l.extensions())
            .copied()
            .collect();

        
        let env_patterns: Vec<glob::Pattern> = config
            .workspace
            .env_files
            .iter()
            .filter_map(|p| glob::Pattern::new(p).ok())
            .collect();

        
        let walker = ignore::WalkBuilder::new(&self.workspace_root)
            .hidden(false) 
            .git_ignore(true) 
            .git_global(true) 
            .git_exclude(true) 
            .require_git(false) 
            .build();

        for entry in walker.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    files.push(path.to_path_buf());
                    continue;
                }
            }

            
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if env_patterns.iter().any(|p| p.matches(name)) {
                    files.push(path.to_path_buf());
                }
            }
        }

        files
    }

    
    
    

    
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

        
        if let Some(exports) = exports {
            self.workspace_index.update_exports(&uri, exports);
        }

        Ok(())
    }

    
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

        
        let binding_graph = AnalysisPipeline::analyze(
            &self.query_engine,
            lang.as_ref(),
            &tree,
            source,
            &ImportContext::default(),
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
                    
                    for symbol in graph.symbols() {
                        if symbol.name.as_str() == local_name && symbol.is_valid {
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

    
    
    

    
    pub async fn on_file_changed(&self, uri: &Url, config: &EcologConfig) {
        
        self.workspace_index.invalidate_resolution_cache(uri);

        if let Ok(path) = uri.to_file_path() {
            if let Err(e) = self.index_file(&path, config).await {
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

    fn default_config() -> EcologConfig {
        EcologConfig::default()
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
        indexer.index_workspace(&default_config()).await.unwrap();

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
        indexer.index_workspace(&default_config()).await.unwrap();

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
        indexer.index_workspace(&default_config()).await.unwrap();

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
        let config = default_config();
        indexer.index_workspace(&config).await.unwrap();

        
        assert!(!indexer.index().files_for_env_var("VAR1").is_empty());
        assert!(indexer.index().files_for_env_var("VAR2").is_empty());

        
        create_file(temp_dir.path(), "test.js", "const x = process.env.VAR2;");
        let uri = Url::from_file_path(temp_dir.path().join("test.js")).unwrap();
        indexer.on_file_changed(&uri, &config).await;

        
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
        indexer.index_workspace(&default_config()).await.unwrap();

        
        assert!(!indexer.index().files_for_env_var("INCLUDED").is_empty());
        assert!(indexer.index().files_for_env_var("IGNORED").is_empty());
        assert!(indexer.index().files_for_env_var("ALSO_IGNORED").is_empty());
    }
}
