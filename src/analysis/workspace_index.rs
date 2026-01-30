





use crate::types::FileExportEntry;
use compact_str::CompactString;
use dashmap::{DashMap, DashSet};
use parking_lot::RwLock;
use quick_cache::sync::Cache;
use rustc_hash::FxHashSet;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::SystemTime;
use tower_lsp::lsp_types::{Range, Url};

/// Maximum number of module resolution cache entries.
/// This bounds memory growth from import resolution.
const MAX_MODULE_RESOLUTION_CACHE_SIZE: usize = 2000;

/// Key type for module resolution cache that implements proper hashing.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ModuleResolutionKey {
    importer: Url,
    specifier: CompactString,
}

impl Hash for ModuleResolutionKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.importer.as_str().hash(state);
        self.specifier.hash(state);
    }
}



#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationKind {
    
    DirectReference,
    
    BindingDeclaration,
    
    BindingUsage,
    
    PropertyAccess,
    
    DestructuredProperty,
    
    EnvFileDefinition,
}



#[derive(Debug, Clone)]
pub struct EnvVarLocation {
    
    pub range: Range,
    
    pub kind: LocationKind,
    
    pub binding_name: Option<CompactString>,
}



#[derive(Debug)]
pub struct FileIndexEntry {
    
    pub mtime: SystemTime,
    
    pub env_vars: FxHashSet<CompactString>,
    
    pub is_env_file: bool,
    
    pub path: PathBuf,
}


#[derive(Debug, Default)]
pub struct IndexState {
    
    pub total_files: usize,
    
    pub indexed_files: AtomicUsize,
    
    pub indexing_in_progress: bool,
    
    pub last_full_index: Option<SystemTime>,
}

impl IndexState {
    
    pub fn increment_indexed(&self) {
        self.indexed_files.fetch_add(1, Ordering::Relaxed);
    }

    
    pub fn indexed_count(&self) -> usize {
        self.indexed_files.load(Ordering::Relaxed)
    }

    
    pub fn progress_percent(&self) -> u8 {
        if self.total_files == 0 {
            return 100;
        }
        let indexed = self.indexed_count();
        ((indexed * 100) / self.total_files).min(100) as u8
    }
}
















pub struct WorkspaceIndex {

    env_to_files: DashMap<CompactString, FxHashSet<Url>>,


    file_entries: DashMap<Url, FileIndexEntry>,


    state: RwLock<IndexState>,






    export_index: DashMap<Url, FileExportEntry>,



    env_export_to_files: DashMap<CompactString, FxHashSet<Url>>,

    /// LRU cache for module resolution with bounded size.
    /// Key: (importer_url, specifier), Value: resolved URL or None for failed lookups.
    /// Bounded to MAX_MODULE_RESOLUTION_CACHE_SIZE entries to prevent unbounded memory growth.
    module_resolution_cache: Cache<ModuleResolutionKey, Option<Url>>,

    /// Files that this file imports from (dependencies)
    /// Key: importer file, Value: list of files it imports from
    file_dependencies: DashMap<Url, Vec<Url>>,

    /// Files that import this file (reverse dependency index)
    /// Key: imported file, Value: list of files that import it
    file_dependents: DashMap<Url, Vec<Url>>,

    /// Files that need re-analysis after a dependency change
    dirty_files: DashSet<Url>,
}

impl WorkspaceIndex {

    pub fn new() -> Self {
        Self {
            env_to_files: DashMap::new(),
            file_entries: DashMap::new(),
            state: RwLock::new(IndexState::default()),
            export_index: DashMap::new(),
            env_export_to_files: DashMap::new(),
            module_resolution_cache: Cache::new(MAX_MODULE_RESOLUTION_CACHE_SIZE),
            file_dependencies: DashMap::new(),
            file_dependents: DashMap::new(),
            dirty_files: DashSet::new(),
        }
    }

    
    
    

    
    
    
    
    pub fn files_for_env_var(&self, name: &str) -> Vec<Url> {
        self.env_to_files
            .get(name)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    
    pub fn is_file_indexed(&self, uri: &Url) -> bool {
        self.file_entries.contains_key(uri)
    }

    
    pub fn env_vars_in_file(&self, uri: &Url) -> Option<FxHashSet<CompactString>> {
        self.file_entries.get(uri).map(|e| e.env_vars.clone())
    }

    
    pub fn is_file_stale(&self, uri: &Url, current_mtime: SystemTime) -> bool {
        self.file_entries
            .get(uri)
            .map(|e| current_mtime > e.mtime)
            .unwrap_or(true) 
    }

    
    pub fn all_env_vars(&self) -> Vec<CompactString> {
        self.env_to_files
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            total_files: self.file_entries.len(),
            total_env_vars: self.env_to_files.len(),
            env_files: self
                .file_entries
                .iter()
                .filter(|e| e.is_env_file)
                .count(),
        }
    }

    
    
    

    
    
    pub fn get_exports(&self, uri: &Url) -> Option<FileExportEntry> {
        self.export_index.get(uri).map(|e| e.clone())
    }

    
    pub fn get_exports_ref(
        &self,
        uri: &Url,
    ) -> Option<dashmap::mapref::one::Ref<'_, Url, FileExportEntry>> {
        self.export_index.get(uri)
    }

    
    
    pub fn files_exporting_env_var(&self, name: &str) -> Vec<Url> {
        self.env_export_to_files
            .get(name)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    
    pub fn has_exports(&self, uri: &Url) -> bool {
        self.export_index
            .get(uri)
            .map(|e| !e.is_empty())
            .unwrap_or(false)
    }

    /// Get cached module resolution result.
    /// Returns Some(Some(url)) if resolved, Some(None) if confirmed not resolvable, None if not cached.
    pub fn cached_module_resolution(
        &self,
        importer: &Url,
        specifier: &str,
    ) -> Option<Option<Url>> {
        let key = ModuleResolutionKey {
            importer: importer.clone(),
            specifier: CompactString::from(specifier),
        };
        self.module_resolution_cache.get(&key)
    }

    
    pub fn all_exported_env_vars(&self) -> Vec<CompactString> {
        self.env_export_to_files
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    
    
    

    
    
    
    
    
    
    pub fn update_file(&self, uri: &Url, entry: FileIndexEntry) {
        
        if let Some(old_entry) = self.file_entries.get(uri) {
            for env_var in &old_entry.env_vars {
                if let Some(mut files) = self.env_to_files.get_mut(env_var) {
                    files.remove(uri);
                    
                    if files.is_empty() {
                        drop(files);
                        self.env_to_files.remove(env_var);
                    }
                }
            }
        }

        
        for env_var in &entry.env_vars {
            self.env_to_files
                .entry(env_var.clone())
                .or_default()
                .insert(uri.clone());
        }

        
        self.file_entries.insert(uri.clone(), entry);
    }

    
    
    
    pub fn remove_file(&self, uri: &Url) {
        
        if let Some((_, entry)) = self.file_entries.remove(uri) {
            for env_var in entry.env_vars {
                if let Some(mut files) = self.env_to_files.get_mut(&env_var) {
                    files.remove(uri);
                    if files.is_empty() {
                        drop(files);
                        self.env_to_files.remove(&env_var);
                    }
                }
            }
        }


        self.remove_exports(uri);


        self.invalidate_resolution_cache(uri);

        self.remove_from_dependency_graph(uri);
    }

    
    pub fn clear(&self) {
        self.env_to_files.clear();
        self.file_entries.clear();
        self.export_index.clear();
        self.env_export_to_files.clear();
        self.module_resolution_cache.clear();
        self.file_dependencies.clear();
        self.file_dependents.clear();
        self.dirty_files.clear();
    }

    
    
    

    
    
    
    
    
    
    pub fn update_exports(&self, uri: &Url, exports: FileExportEntry) {
        
        if let Some(old_exports) = self.export_index.get(uri) {
            for env_var in old_exports.exported_env_vars() {
                if let Some(mut files) = self.env_export_to_files.get_mut(&env_var) {
                    files.remove(uri);
                    if files.is_empty() {
                        drop(files);
                        self.env_export_to_files.remove(&env_var);
                    }
                }
            }
        }

        
        for env_var in exports.exported_env_vars() {
            self.env_export_to_files
                .entry(env_var)
                .or_default()
                .insert(uri.clone());
        }

        
        self.export_index.insert(uri.clone(), exports);
    }

    
    fn remove_exports(&self, uri: &Url) {
        if let Some((_, exports)) = self.export_index.remove(uri) {
            for env_var in exports.exported_env_vars() {
                if let Some(mut files) = self.env_export_to_files.get_mut(&env_var) {
                    files.remove(uri);
                    if files.is_empty() {
                        drop(files);
                        self.env_export_to_files.remove(&env_var);
                    }
                }
            }
        }
    }

    /// Cache a module resolution result.
    pub fn cache_module_resolution(
        &self,
        importer: &Url,
        specifier: &str,
        resolved: Option<Url>,
    ) {
        let key = ModuleResolutionKey {
            importer: importer.clone(),
            specifier: CompactString::from(specifier),
        };
        self.module_resolution_cache.insert(key, resolved);
    }

    /// Invalidate module resolution cache entries related to a changed file.
    /// This selectively removes only entries that:
    /// 1. Were resolved FROM the changed file (importer matches)
    /// 2. Were resolved TO the changed file (resolved URL matches)
    ///
    /// This is more efficient than clearing the entire cache.
    pub fn invalidate_resolution_cache(&self, changed_uri: &Url) {
        // Use retain to keep only entries NOT related to the changed file.
        // An entry is related if:
        // - The importer (source file) is the changed file
        // - The resolved target is the changed file
        self.module_resolution_cache.retain(|key, resolved| {
            // Remove if importer matches changed URI
            if &key.importer == changed_uri {
                return false;
            }
            // Remove if resolved target matches changed URI
            if let Some(target) = resolved {
                if target == changed_uri {
                    return false;
                }
            }
            // Keep all other entries
            true
        });
    }

    /// Clear all module resolution cache entries.
    pub fn clear_resolution_cache(&self) {
        self.module_resolution_cache.clear();
    }

    /// Get module resolution cache statistics for monitoring.
    pub fn module_cache_len(&self) -> usize {
        self.module_resolution_cache.len()
    }

    // =========================================================================
    // Dependency Graph Methods
    // =========================================================================

    /// Update the dependency graph for a file based on its imports.
    /// This should be called during indexing when imports are extracted.
    /// `dependencies` is the list of resolved file URIs that this file imports from.
    pub fn update_dependency_graph(&self, file_uri: &Url, dependencies: Vec<Url>) {
        // Remove old forward edges
        if let Some((_, old_deps)) = self.file_dependencies.remove(file_uri) {
            for dep in old_deps {
                if let Some(mut dependents) = self.file_dependents.get_mut(&dep) {
                    dependents.retain(|u| u != file_uri);
                }
            }
        }

        // Add new forward edges
        for dep in &dependencies {
            self.file_dependents
                .entry(dep.clone())
                .or_default()
                .push(file_uri.clone());
        }

        // Store forward edges
        if !dependencies.is_empty() {
            self.file_dependencies.insert(file_uri.clone(), dependencies);
        }
    }

    /// Invalidate caches for a file change and mark dependents as dirty.
    /// Call this when a file is modified.
    pub fn invalidate_for_file_change(&self, changed_uri: &Url) {
        // Mark all dependents (files that import this file) as dirty
        if let Some(dependents) = self.file_dependents.get(changed_uri) {
            for dep in dependents.iter() {
                self.dirty_files.insert(dep.clone());
            }
        }

        // Invalidate module resolution cache entries related to this file
        self.invalidate_resolution_cache(changed_uri);
    }

    /// Get all files that are marked as dirty and need re-analysis.
    pub fn get_dirty_files(&self) -> Vec<Url> {
        self.dirty_files.iter().map(|u| u.clone()).collect()
    }

    /// Clear the dirty flag for a file after it has been re-analyzed.
    pub fn clear_dirty(&self, uri: &Url) {
        self.dirty_files.remove(uri);
    }

    /// Check if any files are dirty.
    pub fn has_dirty_files(&self) -> bool {
        !self.dirty_files.is_empty()
    }

    /// Get the number of dirty files.
    pub fn dirty_count(&self) -> usize {
        self.dirty_files.len()
    }

    /// Remove a file from the dependency graph completely.
    /// Call this when a file is deleted.
    pub fn remove_from_dependency_graph(&self, uri: &Url) {
        // Remove forward edges (files this file imports from)
        if let Some((_, deps)) = self.file_dependencies.remove(uri) {
            for dep in deps {
                if let Some(mut dependents) = self.file_dependents.get_mut(&dep) {
                    dependents.retain(|u| u != uri);
                    if dependents.is_empty() {
                        drop(dependents);
                        self.file_dependents.remove(&dep);
                    }
                }
            }
        }

        // Remove reverse edges (files that import this file)
        if let Some((_, dependents)) = self.file_dependents.remove(uri) {
            // Mark all files that imported this file as dirty
            for dep in dependents {
                self.dirty_files.insert(dep.clone());
                // Remove this file from their dependencies list
                if let Some(mut deps) = self.file_dependencies.get_mut(&dep) {
                    deps.retain(|u| u != uri);
                }
            }
        }

        // Remove from dirty files
        self.dirty_files.remove(uri);
    }

    /// Get the files that a given file imports from (its dependencies).
    pub fn get_dependencies(&self, uri: &Url) -> Vec<Url> {
        self.file_dependencies
            .get(uri)
            .map(|deps| deps.clone())
            .unwrap_or_default()
    }

    /// Get the files that import a given file (its dependents).
    pub fn get_dependents(&self, uri: &Url) -> Vec<Url> {
        self.file_dependents
            .get(uri)
            .map(|deps| deps.clone())
            .unwrap_or_default()
    }

    pub fn set_indexing(&self, in_progress: bool) {
        let mut state = self.state.write();
        state.indexing_in_progress = in_progress;
        if in_progress {
            state.indexed_files.store(0, Ordering::Relaxed);
        } else {
            state.last_full_index = Some(SystemTime::now());
        }
    }

    
    pub fn set_total_files(&self, count: usize) {
        self.state.write().total_files = count;
    }

    
    pub fn increment_indexed(&self) {
        self.state.read().increment_indexed();
    }

    
    pub fn is_indexing(&self) -> bool {
        self.state.read().indexing_in_progress
    }

    
    pub fn indexing_progress(&self) -> u8 {
        self.state.read().progress_percent()
    }

    
    pub fn get_state(&self) -> IndexStateSnapshot {
        let state = self.state.read();
        IndexStateSnapshot {
            total_files: state.total_files,
            indexed_files: state.indexed_count(),
            indexing_in_progress: state.indexing_in_progress,
            last_full_index: state.last_full_index,
        }
    }
}

impl Default for WorkspaceIndex {
    fn default() -> Self {
        Self::new()
    }
}


#[derive(Debug, Clone)]
pub struct IndexStats {
    pub total_files: usize,
    pub total_env_vars: usize,
    pub env_files: usize,
}


#[derive(Debug, Clone)]
pub struct IndexStateSnapshot {
    pub total_files: usize,
    pub indexed_files: usize,
    pub indexing_in_progress: bool,
    pub last_full_index: Option<SystemTime>,
}





#[cfg(test)]
mod tests {
    use super::*;

    fn url(path: &str) -> Url {
        Url::parse(&format!("file://{}", path)).unwrap()
    }

    fn make_entry(env_vars: &[&str], is_env_file: bool) -> FileIndexEntry {
        FileIndexEntry {
            mtime: SystemTime::now(),
            env_vars: env_vars.iter().map(|s| CompactString::from(*s)).collect(),
            is_env_file,
            path: PathBuf::from("/test"),
        }
    }

    #[test]
    fn test_update_file_adds_reverse_index() {
        let index = WorkspaceIndex::new();
        let uri = url("/test.js");

        index.update_file(&uri, make_entry(&["API_KEY", "DB_URL"], false));

        assert_eq!(index.files_for_env_var("API_KEY"), vec![uri.clone()]);
        assert_eq!(index.files_for_env_var("DB_URL"), vec![uri.clone()]);
        assert!(index.files_for_env_var("NONEXISTENT").is_empty());
    }

    #[test]
    fn test_update_file_removes_old_associations() {
        let index = WorkspaceIndex::new();
        let uri = url("/test.js");

        
        index.update_file(&uri, make_entry(&["OLD_VAR"], false));
        assert!(!index.files_for_env_var("OLD_VAR").is_empty());

        
        index.update_file(&uri, make_entry(&["NEW_VAR"], false));

        
        assert!(index.files_for_env_var("OLD_VAR").is_empty());
        assert!(!index.files_for_env_var("NEW_VAR").is_empty());
    }

    #[test]
    fn test_remove_file_cleans_up() {
        let index = WorkspaceIndex::new();
        let uri = url("/test.js");

        index.update_file(&uri, make_entry(&["API_KEY"], false));
        assert!(!index.files_for_env_var("API_KEY").is_empty());

        index.remove_file(&uri);

        assert!(index.files_for_env_var("API_KEY").is_empty());
        assert!(!index.is_file_indexed(&uri));
    }

    #[test]
    fn test_multiple_files_same_env_var() {
        let index = WorkspaceIndex::new();
        let uri1 = url("/a.js");
        let uri2 = url("/b.ts");
        let uri3 = url("/c.py");

        index.update_file(&uri1, make_entry(&["API_KEY"], false));
        index.update_file(&uri2, make_entry(&["API_KEY", "DB_URL"], false));
        index.update_file(&uri3, make_entry(&["API_KEY"], false));

        let files = index.files_for_env_var("API_KEY");
        assert_eq!(files.len(), 3);
        assert!(files.contains(&uri1));
        assert!(files.contains(&uri2));
        assert!(files.contains(&uri3));

        
        let db_files = index.files_for_env_var("DB_URL");
        assert_eq!(db_files.len(), 1);
        assert!(db_files.contains(&uri2));
    }

    #[test]
    fn test_stats() {
        let index = WorkspaceIndex::new();

        index.update_file(&url("/a.js"), make_entry(&["VAR1", "VAR2"], false));
        index.update_file(&url("/b.ts"), make_entry(&["VAR1"], false));
        index.update_file(&url("/.env"), make_entry(&["VAR1", "VAR2", "VAR3"], true));

        let stats = index.stats();
        assert_eq!(stats.total_files, 3);
        assert_eq!(stats.total_env_vars, 3); 
        assert_eq!(stats.env_files, 1);
    }

    #[test]
    fn test_is_file_stale() {
        let index = WorkspaceIndex::new();
        let uri = url("/test.js");

        let old_time = SystemTime::UNIX_EPOCH;
        let entry = FileIndexEntry {
            mtime: old_time,
            env_vars: FxHashSet::default(),
            is_env_file: false,
            path: PathBuf::from("/test.js"),
        };
        index.update_file(&uri, entry);

        
        assert!(index.is_file_stale(&uri, SystemTime::now()));

        
        assert!(!index.is_file_stale(&uri, old_time));
    }

    #[test]
    fn test_all_env_vars() {
        let index = WorkspaceIndex::new();

        index.update_file(&url("/a.js"), make_entry(&["VAR1", "VAR2"], false));
        index.update_file(&url("/b.ts"), make_entry(&["VAR3"], false));

        let vars = index.all_env_vars();
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn test_indexing_state() {
        let index = WorkspaceIndex::new();

        assert!(!index.is_indexing());

        index.set_total_files(100);
        index.set_indexing(true);
        assert!(index.is_indexing());
        assert_eq!(index.indexing_progress(), 0);

        for _ in 0..50 {
            index.increment_indexed();
        }
        assert_eq!(index.indexing_progress(), 50);

        index.set_indexing(false);
        assert!(!index.is_indexing());

        let state = index.get_state();
        assert!(state.last_full_index.is_some());
    }

    #[test]
    fn test_clear() {
        let index = WorkspaceIndex::new();

        index.update_file(&url("/a.js"), make_entry(&["VAR1"], false));
        index.update_file(&url("/b.ts"), make_entry(&["VAR2"], false));

        assert_eq!(index.stats().total_files, 2);

        index.clear();

        assert_eq!(index.stats().total_files, 0);
        assert_eq!(index.stats().total_env_vars, 0);
    }

    #[test]
    fn test_env_vars_in_file() {
        let index = WorkspaceIndex::new();
        let uri = url("/test.js");

        index.update_file(&uri, make_entry(&["VAR1", "VAR2"], false));

        let vars = index.env_vars_in_file(&uri).unwrap();
        assert!(vars.contains("VAR1"));
        assert!(vars.contains("VAR2"));
        assert!(!vars.contains("VAR3"));

        
        assert!(index.env_vars_in_file(&url("/nonexistent.js")).is_none());
    }

    
    
    

    use crate::types::{ExportResolution, ModuleExport};
    use std::collections::HashMap;
    use tower_lsp::lsp_types::Range;

    fn make_export_entry(exports: &[(&str, &str)]) -> FileExportEntry {
        let mut named_exports = HashMap::new();
        for (name, env_var) in exports {
            named_exports.insert(
                CompactString::from(*name),
                ModuleExport {
                    exported_name: CompactString::from(*name),
                    local_name: None,
                    resolution: ExportResolution::EnvVar {
                        name: CompactString::from(*env_var),
                    },
                    declaration_range: Range::default(),
                    is_default: false,
                },
            );
        }
        FileExportEntry {
            named_exports,
            default_export: None,
            wildcard_reexports: vec![],
        }
    }

    #[test]
    fn test_update_exports() {
        let index = WorkspaceIndex::new();
        let uri = url("/config.js");

        let exports = make_export_entry(&[("dbUrl", "DATABASE_URL")]);
        index.update_exports(&uri, exports);

        assert!(index.has_exports(&uri));
        let retrieved = index.get_exports(&uri).unwrap();
        assert!(retrieved.named_exports.contains_key("dbUrl"));
    }

    #[test]
    fn test_files_exporting_env_var() {
        let index = WorkspaceIndex::new();
        let uri1 = url("/config.js");
        let uri2 = url("/utils.js");

        index.update_exports(&uri1, make_export_entry(&[("dbUrl", "DATABASE_URL")]));
        index.update_exports(
            &uri2,
            make_export_entry(&[("apiKey", "API_KEY"), ("dbConn", "DATABASE_URL")]),
        );

        let db_files = index.files_exporting_env_var("DATABASE_URL");
        assert_eq!(db_files.len(), 2);
        assert!(db_files.contains(&uri1));
        assert!(db_files.contains(&uri2));

        let api_files = index.files_exporting_env_var("API_KEY");
        assert_eq!(api_files.len(), 1);
        assert!(api_files.contains(&uri2));
    }

    #[test]
    fn test_update_exports_removes_old() {
        let index = WorkspaceIndex::new();
        let uri = url("/config.js");

        
        index.update_exports(&uri, make_export_entry(&[("oldVar", "OLD_VAR")]));
        assert!(!index.files_exporting_env_var("OLD_VAR").is_empty());

        
        index.update_exports(&uri, make_export_entry(&[("newVar", "NEW_VAR")]));

        
        assert!(index.files_exporting_env_var("OLD_VAR").is_empty());
        assert!(!index.files_exporting_env_var("NEW_VAR").is_empty());
    }

    #[test]
    fn test_remove_file_clears_exports() {
        let index = WorkspaceIndex::new();
        let uri = url("/config.js");

        index.update_exports(&uri, make_export_entry(&[("dbUrl", "DATABASE_URL")]));
        assert!(!index.files_exporting_env_var("DATABASE_URL").is_empty());

        index.remove_file(&uri);

        assert!(index.files_exporting_env_var("DATABASE_URL").is_empty());
        assert!(!index.has_exports(&uri));
    }

    #[test]
    fn test_module_resolution_cache() {
        let index = WorkspaceIndex::new();
        let importer = url("/app.js");
        let resolved = url("/config.js");

        
        assert!(index.cached_module_resolution(&importer, "./config").is_none());

        
        index.cache_module_resolution(&importer, "./config", Some(resolved.clone()));

        
        let cached = index.cached_module_resolution(&importer, "./config");
        assert_eq!(cached, Some(Some(resolved.clone())));

        
        index.cache_module_resolution(&importer, "./missing", None);
        let cached_none = index.cached_module_resolution(&importer, "./missing");
        assert_eq!(cached_none, Some(None));
    }

    #[test]
    fn test_invalidate_resolution_cache() {
        let index = WorkspaceIndex::new();
        let app = url("/app.js");
        let config = url("/config.js");
        let utils = url("/utils.js");
        let other = url("/other.js");

        // Cache some resolutions
        // app -> config (resolves TO config)
        index.cache_module_resolution(&app, "./config", Some(config.clone()));
        // app -> utils (resolves TO utils)
        index.cache_module_resolution(&app, "./utils", Some(utils.clone()));
        // config -> utils (resolves FROM config, TO utils)
        index.cache_module_resolution(&config, "./utils", Some(utils.clone()));
        // other -> app (unrelated to config)
        index.cache_module_resolution(&other, "./app", Some(app.clone()));

        // Verify all cached
        assert!(index.cached_module_resolution(&app, "./config").is_some());
        assert!(index.cached_module_resolution(&app, "./utils").is_some());
        assert!(index.cached_module_resolution(&config, "./utils").is_some());
        assert!(index.cached_module_resolution(&other, "./app").is_some());

        // Invalidate config: should remove entries FROM config and TO config
        index.invalidate_resolution_cache(&config);

        // Entries TO config should be invalidated (app -> config)
        assert!(index.cached_module_resolution(&app, "./config").is_none());
        // Entries FROM config should be invalidated (config -> utils)
        assert!(index.cached_module_resolution(&config, "./utils").is_none());
        // Unrelated entries should remain (app -> utils, other -> app)
        assert!(index.cached_module_resolution(&app, "./utils").is_some());
        assert!(index.cached_module_resolution(&other, "./app").is_some());
    }

    #[test]
    fn test_invalidate_resolution_cache_none_values() {
        let index = WorkspaceIndex::new();
        let app = url("/app.js");
        let config = url("/config.js");

        // Cache a failed resolution (None value)
        index.cache_module_resolution(&app, "./missing", None);
        // Cache a successful resolution
        index.cache_module_resolution(&config, "./missing", None);

        // Verify both cached
        assert!(index.cached_module_resolution(&app, "./missing").is_some());
        assert!(index.cached_module_resolution(&config, "./missing").is_some());

        // Invalidate app: should remove entries FROM app
        index.invalidate_resolution_cache(&app);

        // Entry FROM app should be invalidated
        assert!(index.cached_module_resolution(&app, "./missing").is_none());
        // Entry FROM config should remain
        assert!(index.cached_module_resolution(&config, "./missing").is_some());
    }

    #[test]
    fn test_clear_clears_exports() {
        let index = WorkspaceIndex::new();

        index.update_exports(
            &url("/config.js"),
            make_export_entry(&[("dbUrl", "DATABASE_URL")]),
        );
        index.cache_module_resolution(&url("/app.js"), "./config", Some(url("/config.js")));

        index.clear();

        assert!(index.files_exporting_env_var("DATABASE_URL").is_empty());
        assert!(index
            .cached_module_resolution(&url("/app.js"), "./config")
            .is_none());
    }

    #[test]
    fn test_all_exported_env_vars() {
        let index = WorkspaceIndex::new();

        index.update_exports(
            &url("/config.js"),
            make_export_entry(&[("db", "DATABASE_URL"), ("api", "API_KEY")]),
        );
        index.update_exports(&url("/utils.js"), make_export_entry(&[("secret", "SECRET")]));

        let vars = index.all_exported_env_vars();
        assert_eq!(vars.len(), 3);
    }

    // =========================================================================
    // Task 3: Module Dependency Graph Tests - update_dependency_graph
    // =========================================================================

    #[test]
    fn test_update_dependency_graph_single_dependency() {
        let index = WorkspaceIndex::new();
        let app = url("/app.js");
        let config = url("/config.js");

        index.update_dependency_graph(&app, vec![config.clone()]);

        // Forward edge: app depends on config
        let deps = index.get_dependencies(&app);
        assert_eq!(deps.len(), 1);
        assert!(deps.contains(&config));

        // Reverse edge: config is depended on by app
        let dependents = index.get_dependents(&config);
        assert_eq!(dependents.len(), 1);
        assert!(dependents.contains(&app));
    }

    #[test]
    fn test_update_dependency_graph_multiple_dependencies() {
        let index = WorkspaceIndex::new();
        let app = url("/app.js");
        let config = url("/config.js");
        let utils = url("/utils.js");
        let helpers = url("/helpers.js");

        index.update_dependency_graph(&app, vec![config.clone(), utils.clone(), helpers.clone()]);

        let deps = index.get_dependencies(&app);
        assert_eq!(deps.len(), 3);
        assert!(deps.contains(&config));
        assert!(deps.contains(&utils));
        assert!(deps.contains(&helpers));

        // Each dependency should have app as a dependent
        assert!(index.get_dependents(&config).contains(&app));
        assert!(index.get_dependents(&utils).contains(&app));
        assert!(index.get_dependents(&helpers).contains(&app));
    }

    #[test]
    fn test_update_dependency_graph_replaces_old_dependencies() {
        let index = WorkspaceIndex::new();
        let app = url("/app.js");
        let old_dep = url("/old.js");
        let new_dep = url("/new.js");

        // First update with old dependency
        index.update_dependency_graph(&app, vec![old_dep.clone()]);
        assert!(index.get_dependencies(&app).contains(&old_dep));
        assert!(index.get_dependents(&old_dep).contains(&app));

        // Update with new dependency (replaces old)
        index.update_dependency_graph(&app, vec![new_dep.clone()]);

        // Old dependency should be removed
        assert!(!index.get_dependencies(&app).contains(&old_dep));
        assert!(!index.get_dependents(&old_dep).contains(&app));

        // New dependency should be present
        assert!(index.get_dependencies(&app).contains(&new_dep));
        assert!(index.get_dependents(&new_dep).contains(&app));
    }

    #[test]
    fn test_update_dependency_graph_empty_dependencies() {
        let index = WorkspaceIndex::new();
        let app = url("/app.js");
        let config = url("/config.js");

        // First add some dependencies
        index.update_dependency_graph(&app, vec![config.clone()]);
        assert!(!index.get_dependencies(&app).is_empty());

        // Update with empty dependencies
        index.update_dependency_graph(&app, vec![]);

        // All dependencies should be cleared
        assert!(index.get_dependencies(&app).is_empty());
        assert!(!index.get_dependents(&config).contains(&app));
    }

    #[test]
    fn test_update_dependency_graph_bidirectional_consistency() {
        let index = WorkspaceIndex::new();
        let a = url("/a.js");
        let b = url("/b.js");
        let c = url("/c.js");

        // a depends on b and c
        index.update_dependency_graph(&a, vec![b.clone(), c.clone()]);
        // b depends on c
        index.update_dependency_graph(&b, vec![c.clone()]);

        // Check forward edges
        assert!(index.get_dependencies(&a).contains(&b));
        assert!(index.get_dependencies(&a).contains(&c));
        assert!(index.get_dependencies(&b).contains(&c));

        // Check reverse edges
        let c_dependents = index.get_dependents(&c);
        assert!(c_dependents.contains(&a));
        assert!(c_dependents.contains(&b));

        let b_dependents = index.get_dependents(&b);
        assert!(b_dependents.contains(&a));
    }

    #[test]
    fn test_update_dependency_graph_circular_dependencies() {
        let index = WorkspaceIndex::new();
        let a = url("/a.js");
        let b = url("/b.js");

        // Create circular dependency: a -> b -> a
        index.update_dependency_graph(&a, vec![b.clone()]);
        index.update_dependency_graph(&b, vec![a.clone()]);

        // Both should have each other as dependencies
        assert!(index.get_dependencies(&a).contains(&b));
        assert!(index.get_dependencies(&b).contains(&a));

        // Both should have each other as dependents
        assert!(index.get_dependents(&a).contains(&b));
        assert!(index.get_dependents(&b).contains(&a));
    }

    // =========================================================================
    // Task 3: Module Dependency Graph Tests - invalidate_for_file_change
    // =========================================================================

    #[test]
    fn test_invalidate_for_file_change_marks_dependents_dirty() {
        let index = WorkspaceIndex::new();
        let config = url("/config.js");
        let app1 = url("/app1.js");
        let app2 = url("/app2.js");

        // app1 and app2 both depend on config
        index.update_dependency_graph(&app1, vec![config.clone()]);
        index.update_dependency_graph(&app2, vec![config.clone()]);

        // Change config - should mark app1 and app2 as dirty
        index.invalidate_for_file_change(&config);

        let dirty = index.get_dirty_files();
        assert!(dirty.contains(&app1));
        assert!(dirty.contains(&app2));
    }

    #[test]
    fn test_invalidate_for_file_change_no_dependents() {
        let index = WorkspaceIndex::new();
        let config = url("/config.js");

        // config has no dependents
        index.invalidate_for_file_change(&config);

        // No dirty files
        assert!(index.get_dirty_files().is_empty());
    }

    #[test]
    fn test_invalidate_for_file_change_calls_invalidate_resolution_cache() {
        let index = WorkspaceIndex::new();
        let app = url("/app.js");
        let config = url("/config.js");

        // Cache a resolution
        index.cache_module_resolution(&app, "./config", Some(config.clone()));
        assert!(index.cached_module_resolution(&app, "./config").is_some());

        // Invalidate config
        index.invalidate_for_file_change(&config);

        // Cache entry that resolved TO config should be invalidated
        assert!(index.cached_module_resolution(&app, "./config").is_none());
    }

    #[test]
    fn test_invalidate_for_file_change_only_direct_dependents() {
        let index = WorkspaceIndex::new();
        let a = url("/a.js");
        let b = url("/b.js");
        let c = url("/c.js");

        // a -> b -> c (a depends on b, b depends on c)
        index.update_dependency_graph(&a, vec![b.clone()]);
        index.update_dependency_graph(&b, vec![c.clone()]);

        // Change c - should only mark b as dirty (direct dependent), not a
        index.invalidate_for_file_change(&c);

        let dirty = index.get_dirty_files();
        assert!(dirty.contains(&b));
        assert!(!dirty.contains(&a)); // a is transitive, not direct
    }

    // =========================================================================
    // Task 3: Module Dependency Graph Tests - Dirty Files Management
    // =========================================================================

    #[test]
    fn test_get_dirty_files_empty() {
        let index = WorkspaceIndex::new();
        assert!(index.get_dirty_files().is_empty());
    }

    #[test]
    fn test_get_dirty_files_after_invalidation() {
        let index = WorkspaceIndex::new();
        let config = url("/config.js");
        let app1 = url("/app1.js");
        let app2 = url("/app2.js");
        let app3 = url("/app3.js");

        // Set up dependencies
        index.update_dependency_graph(&app1, vec![config.clone()]);
        index.update_dependency_graph(&app2, vec![config.clone()]);
        index.update_dependency_graph(&app3, vec![config.clone()]);

        // Invalidate
        index.invalidate_for_file_change(&config);

        let dirty = index.get_dirty_files();
        assert_eq!(dirty.len(), 3);
        assert!(dirty.contains(&app1));
        assert!(dirty.contains(&app2));
        assert!(dirty.contains(&app3));
    }

    #[test]
    fn test_clear_dirty_single_file() {
        let index = WorkspaceIndex::new();
        let config = url("/config.js");
        let app1 = url("/app1.js");
        let app2 = url("/app2.js");

        index.update_dependency_graph(&app1, vec![config.clone()]);
        index.update_dependency_graph(&app2, vec![config.clone()]);
        index.invalidate_for_file_change(&config);

        assert!(index.get_dirty_files().contains(&app1));
        assert!(index.get_dirty_files().contains(&app2));

        // Clear only app1
        index.clear_dirty(&app1);

        assert!(!index.get_dirty_files().contains(&app1));
        assert!(index.get_dirty_files().contains(&app2));
    }

    #[test]
    fn test_has_dirty_files_true_and_false() {
        let index = WorkspaceIndex::new();
        let config = url("/config.js");
        let app = url("/app.js");

        // Initially no dirty files
        assert!(!index.has_dirty_files());

        index.update_dependency_graph(&app, vec![config.clone()]);
        index.invalidate_for_file_change(&config);

        // Now has dirty files
        assert!(index.has_dirty_files());

        // Clear all dirty files
        index.clear_dirty(&app);

        // No more dirty files
        assert!(!index.has_dirty_files());
    }

    #[test]
    fn test_dirty_count() {
        let index = WorkspaceIndex::new();
        let config = url("/config.js");
        let app1 = url("/app1.js");
        let app2 = url("/app2.js");
        let app3 = url("/app3.js");

        assert_eq!(index.dirty_count(), 0);

        index.update_dependency_graph(&app1, vec![config.clone()]);
        index.update_dependency_graph(&app2, vec![config.clone()]);
        index.update_dependency_graph(&app3, vec![config.clone()]);
        index.invalidate_for_file_change(&config);

        assert_eq!(index.dirty_count(), 3);

        index.clear_dirty(&app1);
        assert_eq!(index.dirty_count(), 2);

        index.clear_dirty(&app2);
        index.clear_dirty(&app3);
        assert_eq!(index.dirty_count(), 0);
    }

    // =========================================================================
    // Task 3: Module Dependency Graph Tests - remove_from_dependency_graph
    // =========================================================================

    #[test]
    fn test_remove_from_dependency_graph_removes_forward_edges() {
        let index = WorkspaceIndex::new();
        let app = url("/app.js");
        let config = url("/config.js");
        let utils = url("/utils.js");

        // app depends on config and utils
        index.update_dependency_graph(&app, vec![config.clone(), utils.clone()]);

        // Remove app from dependency graph
        index.remove_from_dependency_graph(&app);

        // app should have no dependencies
        assert!(index.get_dependencies(&app).is_empty());

        // config and utils should not have app as a dependent
        assert!(!index.get_dependents(&config).contains(&app));
        assert!(!index.get_dependents(&utils).contains(&app));
    }

    #[test]
    fn test_remove_from_dependency_graph_removes_reverse_edges() {
        let index = WorkspaceIndex::new();
        let config = url("/config.js");
        let app1 = url("/app1.js");
        let app2 = url("/app2.js");

        // app1 and app2 depend on config
        index.update_dependency_graph(&app1, vec![config.clone()]);
        index.update_dependency_graph(&app2, vec![config.clone()]);

        // Remove config
        index.remove_from_dependency_graph(&config);

        // config should have no dependents
        assert!(index.get_dependents(&config).is_empty());
    }

    #[test]
    fn test_remove_from_dependency_graph_marks_importers_dirty() {
        let index = WorkspaceIndex::new();
        let config = url("/config.js");
        let app = url("/app.js");

        // app depends on config
        index.update_dependency_graph(&app, vec![config.clone()]);

        // Remove config (simulating file deletion)
        index.remove_from_dependency_graph(&config);

        // app should be marked as dirty because it imported config
        assert!(index.get_dirty_files().contains(&app));
    }

    #[test]
    fn test_remove_from_dependency_graph_cleans_empty_dependents() {
        let index = WorkspaceIndex::new();
        let config = url("/config.js");
        let app = url("/app.js");

        // app depends on config
        index.update_dependency_graph(&app, vec![config.clone()]);

        // Verify config has dependents
        assert!(!index.get_dependents(&config).is_empty());

        // Remove app
        index.remove_from_dependency_graph(&app);

        // config's dependents list should be cleaned up (empty)
        // Note: The entry might be removed entirely or left empty
        assert!(index.get_dependents(&config).is_empty());
    }

    #[test]
    fn test_remove_file_calls_remove_from_dependency_graph() {
        let index = WorkspaceIndex::new();
        let config = url("/config.js");
        let app = url("/app.js");

        // Add file entry and dependency
        index.update_file(&config, make_entry(&["DB_URL"], false));
        index.update_dependency_graph(&app, vec![config.clone()]);

        // Verify setup
        assert!(index.is_file_indexed(&config));
        assert!(index.get_dependents(&config).contains(&app));

        // Remove file
        index.remove_file(&config);

        // File should be removed and dependency graph updated
        assert!(!index.is_file_indexed(&config));
        assert!(!index.get_dependents(&config).contains(&app));
        // app should be marked dirty
        assert!(index.get_dirty_files().contains(&app));
    }

    // =========================================================================
    // Task 3: Module Dependency Graph Tests - Getters
    // =========================================================================

    #[test]
    fn test_get_dependencies_empty() {
        let index = WorkspaceIndex::new();
        let unknown = url("/unknown.js");

        // Unknown file should return empty list
        let deps = index.get_dependencies(&unknown);
        assert!(deps.is_empty());
    }

    #[test]
    fn test_get_dependencies_returns_correct_list() {
        let index = WorkspaceIndex::new();
        let app = url("/app.js");
        let dep1 = url("/dep1.js");
        let dep2 = url("/dep2.js");
        let dep3 = url("/dep3.js");

        index.update_dependency_graph(&app, vec![dep1.clone(), dep2.clone(), dep3.clone()]);

        let deps = index.get_dependencies(&app);
        assert_eq!(deps.len(), 3);
        assert!(deps.contains(&dep1));
        assert!(deps.contains(&dep2));
        assert!(deps.contains(&dep3));
    }

    #[test]
    fn test_get_dependents_empty() {
        let index = WorkspaceIndex::new();
        let config = url("/config.js");

        // File with no importers should return empty list
        let dependents = index.get_dependents(&config);
        assert!(dependents.is_empty());
    }

    #[test]
    fn test_get_dependents_returns_correct_list() {
        let index = WorkspaceIndex::new();
        let config = url("/config.js");
        let app1 = url("/app1.js");
        let app2 = url("/app2.js");
        let app3 = url("/app3.js");

        index.update_dependency_graph(&app1, vec![config.clone()]);
        index.update_dependency_graph(&app2, vec![config.clone()]);
        index.update_dependency_graph(&app3, vec![config.clone()]);

        let dependents = index.get_dependents(&config);
        assert_eq!(dependents.len(), 3);
        assert!(dependents.contains(&app1));
        assert!(dependents.contains(&app2));
        assert!(dependents.contains(&app3));
    }

    #[test]
    fn test_clear_clears_dependency_graph() {
        let index = WorkspaceIndex::new();
        let app = url("/app.js");
        let config = url("/config.js");

        // Set up some state
        index.update_file(&app, make_entry(&["VAR1"], false));
        index.update_dependency_graph(&app, vec![config.clone()]);
        index.invalidate_for_file_change(&config);

        // Verify state
        assert!(!index.get_dependencies(&app).is_empty());
        assert!(index.has_dirty_files());

        // Clear everything
        index.clear();

        // Dependency graph should be cleared
        assert!(index.get_dependencies(&app).is_empty());
        assert!(index.get_dependents(&config).is_empty());
        assert!(!index.has_dirty_files());
    }

    // =========================================================================
    // Additional Edge Case Tests
    // =========================================================================

    #[test]
    fn test_dependency_graph_self_reference() {
        let index = WorkspaceIndex::new();
        let file = url("/file.js");

        // File depending on itself (unusual but possible)
        index.update_dependency_graph(&file, vec![file.clone()]);

        assert!(index.get_dependencies(&file).contains(&file));
        assert!(index.get_dependents(&file).contains(&file));
    }

    #[test]
    fn test_dependency_graph_update_partial() {
        let index = WorkspaceIndex::new();
        let app = url("/app.js");
        let dep1 = url("/dep1.js");
        let dep2 = url("/dep2.js");
        let dep3 = url("/dep3.js");

        // Initial: app depends on dep1 and dep2
        index.update_dependency_graph(&app, vec![dep1.clone(), dep2.clone()]);

        // Update: app now depends on dep2 and dep3 (dep1 removed, dep3 added)
        index.update_dependency_graph(&app, vec![dep2.clone(), dep3.clone()]);

        let deps = index.get_dependencies(&app);
        assert!(!deps.contains(&dep1));
        assert!(deps.contains(&dep2));
        assert!(deps.contains(&dep3));

        // dep1 should no longer have app as a dependent
        assert!(!index.get_dependents(&dep1).contains(&app));
    }

    #[test]
    fn test_multiple_invalidations_same_file() {
        let index = WorkspaceIndex::new();
        let config = url("/config.js");
        let app = url("/app.js");

        index.update_dependency_graph(&app, vec![config.clone()]);

        // Multiple invalidations should not duplicate dirty entries
        index.invalidate_for_file_change(&config);
        index.invalidate_for_file_change(&config);
        index.invalidate_for_file_change(&config);

        // Should still only have one dirty entry for app
        let dirty = index.get_dirty_files();
        assert_eq!(dirty.iter().filter(|u| *u == &app).count(), 1);
    }
}
