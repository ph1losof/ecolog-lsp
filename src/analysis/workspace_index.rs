





use crate::types::FileExportEntry;
use compact_str::CompactString;
use dashmap::DashMap;
use parking_lot::RwLock;
use rustc_hash::FxHashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::SystemTime;
use tower_lsp::lsp_types::{Range, Url};



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

    
    
    
    module_resolution_cache: DashMap<(Url, CompactString), Option<Url>>,
}

impl WorkspaceIndex {
    
    pub fn new() -> Self {
        Self {
            env_to_files: DashMap::new(),
            file_entries: DashMap::new(),
            state: RwLock::new(IndexState::default()),
            export_index: DashMap::new(),
            env_export_to_files: DashMap::new(),
            module_resolution_cache: DashMap::new(),
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
    ) -> Option<dashmap::mapref::one::Ref<Url, FileExportEntry>> {
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

    
    
    pub fn cached_module_resolution(
        &self,
        importer: &Url,
        specifier: &str,
    ) -> Option<Option<Url>> {
        self.module_resolution_cache
            .get(&(importer.clone(), CompactString::from(specifier)))
            .map(|r| r.clone())
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
    }

    
    pub fn clear(&self) {
        self.env_to_files.clear();
        self.file_entries.clear();
        self.export_index.clear();
        self.env_export_to_files.clear();
        self.module_resolution_cache.clear();
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

    
    pub fn cache_module_resolution(
        &self,
        importer: &Url,
        specifier: &str,
        resolved: Option<Url>,
    ) {
        self.module_resolution_cache
            .insert((importer.clone(), CompactString::from(specifier)), resolved);
    }

    
    
    
    
    
    pub fn invalidate_resolution_cache(&self, changed_uri: &Url) {
        
        self.module_resolution_cache.retain(|_, resolved| {
            resolved.as_ref() != Some(changed_uri)
        });

        
        self.module_resolution_cache.retain(|(importer, _), _| {
            importer != changed_uri
        });
    }

    
    pub fn clear_resolution_cache(&self) {
        self.module_resolution_cache.clear();
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
        Url::parse(&format!("file:
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

        
        index.cache_module_resolution(&app, "./config", Some(config.clone()));
        index.cache_module_resolution(&app, "./utils", Some(utils.clone()));
        index.cache_module_resolution(&config, "./utils", Some(utils.clone()));

        
        index.invalidate_resolution_cache(&config);

        
        assert!(index.cached_module_resolution(&app, "./config").is_none());

        
        assert!(index.cached_module_resolution(&config, "./utils").is_none());

        
        assert!(index.cached_module_resolution(&app, "./utils").is_some());
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
}
