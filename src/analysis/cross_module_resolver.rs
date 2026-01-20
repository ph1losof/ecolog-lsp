use crate::analysis::{ModuleResolver, WorkspaceIndex};
use crate::languages::LanguageRegistry;
use crate::types::{ExportResolution, ModuleExport};
use compact_str::CompactString;
use rustc_hash::FxHashSet;
use std::sync::Arc;
use tower_lsp::lsp_types::{Range, Url};

const MAX_RESOLUTION_DEPTH: usize = 10;

#[derive(Debug, Clone)]
pub enum CrossModuleResolution {
    EnvVar {
        name: CompactString,

        defining_file: Url,

        declaration_range: Range,
    },

    EnvObject {
        canonical_name: CompactString,

        defining_file: Url,
    },

    Unresolved,
}

pub struct CrossModuleResolver {
    workspace_index: Arc<WorkspaceIndex>,

    module_resolver: Arc<ModuleResolver>,

    languages: Arc<LanguageRegistry>,
}

impl CrossModuleResolver {
    pub fn new(
        workspace_index: Arc<WorkspaceIndex>,
        module_resolver: Arc<ModuleResolver>,
        languages: Arc<LanguageRegistry>,
    ) -> Self {
        Self {
            workspace_index,
            module_resolver,
            languages,
        }
    }

    pub fn resolve_import(
        &self,
        importer_uri: &Url,
        module_specifier: &str,
        imported_name: &str,
        is_default: bool,
    ) -> CrossModuleResolution {
        let source_uri = match self.resolve_module_specifier(importer_uri, module_specifier) {
            Some(uri) => uri,
            None => return CrossModuleResolution::Unresolved,
        };

        let mut visited = FxHashSet::default();
        self.resolve_recursive(&source_uri, imported_name, is_default, &mut visited, 0)
    }

    fn resolve_module_specifier(&self, from_uri: &Url, specifier: &str) -> Option<Url> {
        if let Some(cached) = self
            .workspace_index
            .cached_module_resolution(from_uri, specifier)
        {
            return cached;
        }

        let language = self.languages.get_for_uri(from_uri)?;
        let resolved = self
            .module_resolver
            .resolve_to_uri(specifier, from_uri, language.as_ref());

        self.workspace_index
            .cache_module_resolution(from_uri, specifier, resolved.clone());

        resolved
    }

    fn resolve_recursive(
        &self,
        source_uri: &Url,
        name: &str,
        is_default: bool,
        visited: &mut FxHashSet<(Url, String)>,
        depth: usize,
    ) -> CrossModuleResolution {
        if depth >= MAX_RESOLUTION_DEPTH {
            return CrossModuleResolution::Unresolved;
        }

        let key = (source_uri.clone(), name.to_string());
        if visited.contains(&key) {
            return CrossModuleResolution::Unresolved;
        }
        visited.insert(key);

        let exports = match self.workspace_index.get_exports(source_uri) {
            Some(e) => e,
            None => return CrossModuleResolution::Unresolved,
        };

        let export = if is_default {
            exports.default_export.as_ref()
        } else {
            exports.get_export(name)
        };

        if let Some(export) = export {
            return self.resolve_export(export, source_uri, visited, depth);
        }

        for wildcard_source in &exports.wildcard_reexports {
            if let Some(wildcard_uri) = self.resolve_module_specifier(source_uri, wildcard_source) {
                let result = self.resolve_recursive(&wildcard_uri, name, false, visited, depth + 1);
                if !matches!(result, CrossModuleResolution::Unresolved) {
                    return result;
                }
            }
        }

        CrossModuleResolution::Unresolved
    }

    fn resolve_export(
        &self,
        export: &ModuleExport,
        source_uri: &Url,
        visited: &mut FxHashSet<(Url, String)>,
        depth: usize,
    ) -> CrossModuleResolution {
        match &export.resolution {
            ExportResolution::EnvVar { name } => CrossModuleResolution::EnvVar {
                name: name.clone(),
                defining_file: source_uri.clone(),
                declaration_range: export.declaration_range,
            },

            ExportResolution::EnvObject { canonical_name } => CrossModuleResolution::EnvObject {
                canonical_name: canonical_name.clone(),
                defining_file: source_uri.clone(),
            },

            ExportResolution::ReExport {
                source_module,
                original_name,
            } => {
                if let Some(reexport_uri) = self.resolve_module_specifier(source_uri, source_module)
                {
                    self.resolve_recursive(&reexport_uri, original_name, false, visited, depth + 1)
                } else {
                    CrossModuleResolution::Unresolved
                }
            }

            ExportResolution::LocalChain { symbol_id: _ } => CrossModuleResolution::Unresolved,

            ExportResolution::Unknown => CrossModuleResolution::Unresolved,
        }
    }

    pub fn files_exporting_env_var(&self, env_var_name: &str) -> Vec<Url> {
        self.workspace_index.files_exporting_env_var(env_var_name)
    }

    pub fn resolve_namespace_import(
        &self,
        importer_uri: &Url,
        module_specifier: &str,
    ) -> Vec<(CompactString, CompactString)> {
        let source_uri = match self.resolve_module_specifier(importer_uri, module_specifier) {
            Some(uri) => uri,
            None => return Vec::new(),
        };

        let exports = match self.workspace_index.get_exports(&source_uri) {
            Some(e) => e,
            None => return Vec::new(),
        };

        let mut results = Vec::new();
        let mut visited = FxHashSet::default();

        for (name, export) in &exports.named_exports {
            if let CrossModuleResolution::EnvVar { name: env_name, .. } =
                self.resolve_export(export, &source_uri, &mut visited, 0)
            {
                results.push((name.clone(), env_name));
            }
        }

        results
    }
}

impl CrossModuleResolver {
    pub fn can_resolve(&self, from_uri: &Url, specifier: &str) -> bool {
        self.resolve_module_specifier(from_uri, specifier).is_some()
    }

    pub fn workspace_index(&self) -> &Arc<WorkspaceIndex> {
        &self.workspace_index
    }

    pub fn module_resolver(&self) -> &Arc<ModuleResolver> {
        &self.module_resolver
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FileExportEntry, ModuleExport};
    use tempfile::TempDir;

    fn setup_test_environment() -> (TempDir, Arc<WorkspaceIndex>, Arc<ModuleResolver>) {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();

        let workspace_index = Arc::new(WorkspaceIndex::new());
        let module_resolver = Arc::new(ModuleResolver::new(workspace_root));

        (temp_dir, workspace_index, module_resolver)
    }

    fn create_mock_registry() -> Arc<LanguageRegistry> {
        use crate::languages::javascript::JavaScript;
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(JavaScript));
        Arc::new(registry)
    }

    #[test]
    fn test_resolve_direct_env_export() {
        let (_temp, workspace_index, module_resolver) = setup_test_environment();
        let languages = create_mock_registry();

        let config_uri = Url::parse("file:///workspace/src/config.ts").unwrap();
        let mut exports = FileExportEntry::new();

        exports.named_exports.insert(
            "dbUrl".into(),
            ModuleExport {
                exported_name: "dbUrl".into(),
                local_name: None,
                resolution: ExportResolution::EnvVar {
                    name: "DATABASE_URL".into(),
                },
                declaration_range: Range::default(),
                is_default: false,
            },
        );

        workspace_index.update_exports(&config_uri, exports);

        let resolver = CrossModuleResolver::new(workspace_index, module_resolver, languages);

        let result = resolver.resolve_import(
            &Url::parse("file:///workspace/src/api.ts").unwrap(),
            "./config",
            "dbUrl",
            false,
        );

        assert!(matches!(result, CrossModuleResolution::Unresolved));
    }

    #[test]
    fn test_max_depth_prevents_infinite_loop() {
        let (_temp, workspace_index, module_resolver) = setup_test_environment();
        let languages = create_mock_registry();

        let uri_a = Url::parse("file:///workspace/a.ts").unwrap();
        let uri_b = Url::parse("file:///workspace/b.ts").unwrap();

        let mut exports_a = FileExportEntry::new();
        exports_a.named_exports.insert(
            "foo".into(),
            ModuleExport {
                exported_name: "foo".into(),
                local_name: None,
                resolution: ExportResolution::ReExport {
                    source_module: "./b".into(),
                    original_name: "foo".into(),
                },
                declaration_range: Range::default(),
                is_default: false,
            },
        );

        let mut exports_b = FileExportEntry::new();
        exports_b.named_exports.insert(
            "foo".into(),
            ModuleExport {
                exported_name: "foo".into(),
                local_name: None,
                resolution: ExportResolution::ReExport {
                    source_module: "./a".into(),
                    original_name: "foo".into(),
                },
                declaration_range: Range::default(),
                is_default: false,
            },
        );

        workspace_index.update_exports(&uri_a, exports_a);
        workspace_index.update_exports(&uri_b, exports_b);

        let resolver = CrossModuleResolver::new(workspace_index, module_resolver, languages);

        let result = resolver.resolve_import(&uri_a, "./b", "foo", false);
        assert!(matches!(result, CrossModuleResolution::Unresolved));
    }
}
