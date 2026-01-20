use crate::languages::LanguageSupport;
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::Url;

#[derive(Debug, Clone)]
pub struct ModuleResolver {
    workspace_root: PathBuf,
}

impl ModuleResolver {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn resolve(
        &self,
        specifier: &str,
        from_uri: &Url,
        language: &dyn LanguageSupport,
    ) -> Option<PathBuf> {
        if !specifier.starts_with("./") && !specifier.starts_with("../") {
            return None;
        }

        let from_path = from_uri.to_file_path().ok()?;
        let from_dir = from_path.parent()?;
        let base_path = from_dir.join(specifier);

        let normalized = normalize_path(&base_path);

        if !normalized.starts_with(&self.workspace_root) {
            return None;
        }

        self.resolve_with_extensions(&normalized, language)
    }

    pub fn resolve_to_uri(
        &self,
        specifier: &str,
        from_uri: &Url,
        language: &dyn LanguageSupport,
    ) -> Option<Url> {
        let path = self.resolve(specifier, from_uri, language)?;
        Url::from_file_path(path).ok()
    }

    fn resolve_with_extensions(
        &self,
        base_path: &Path,
        language: &dyn LanguageSupport,
    ) -> Option<PathBuf> {
        if base_path.exists() && base_path.is_file() {
            return Some(base_path.to_path_buf());
        }

        let base_str = base_path.to_string_lossy();
        for ext in language.extensions() {
            let with_ext = PathBuf::from(format!("{}.{}", base_str, ext));
            if with_ext.exists() {
                return Some(with_ext);
            }
        }

        if base_path.is_dir() || !base_path.exists() {
            for ext in language.extensions() {
                let index_path = base_path.join(format!("index.{}", ext));
                if index_path.exists() {
                    return Some(index_path);
                }
            }
        }

        None
    }

    #[inline]
    pub fn is_relative_import(specifier: &str) -> bool {
        specifier.starts_with("./") || specifier.starts_with("../")
    }

    #[inline]
    pub fn is_package_import(specifier: &str) -> bool {
        !specifier.starts_with("./") && !specifier.starts_with("../") && !specifier.starts_with('/')
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();

    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                if !components.is_empty() {
                    components.pop();
                }
            }
            std::path::Component::CurDir => {}
            _ => {
                components.push(component);
            }
        }
    }

    components.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::TempDir;

    struct MockLanguage {
        extensions: &'static [&'static str],
    }

    impl LanguageSupport for MockLanguage {
        fn id(&self) -> &'static str {
            "mock"
        }

        fn extensions(&self) -> &'static [&'static str] {
            self.extensions
        }

        fn language_ids(&self) -> &'static [&'static str] {
            &["mock"]
        }

        fn grammar(&self) -> tree_sitter::Language {
            tree_sitter_javascript::LANGUAGE.into()
        }

        fn reference_query(&self) -> &tree_sitter::Query {
            static QUERY: std::sync::OnceLock<tree_sitter::Query> = std::sync::OnceLock::new();
            QUERY.get_or_init(|| {
                tree_sitter::Query::new(
                    &tree_sitter_javascript::LANGUAGE.into(),
                    "(identifier) @id",
                )
                .unwrap()
            })
        }
    }

    fn setup_test_workspace() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path().to_path_buf();

        fs::create_dir_all(workspace.join("src/utils")).unwrap();
        fs::create_dir_all(workspace.join("src/config")).unwrap();

        File::create(workspace.join("src/config.ts")).unwrap();
        File::create(workspace.join("src/utils/env.ts")).unwrap();
        File::create(workspace.join("src/config/index.ts")).unwrap();
        File::create(workspace.join("src/utils/helpers.js")).unwrap();

        (temp_dir, workspace)
    }

    #[test]
    fn test_resolve_relative_import() {
        let (_temp, workspace) = setup_test_workspace();
        let resolver = ModuleResolver::new(workspace.clone());
        let lang = MockLanguage {
            extensions: &["ts", "tsx", "js", "jsx"],
        };

        let from_uri = Url::from_file_path(workspace.join("src/index.ts")).unwrap();

        let result = resolver.resolve("./config", &from_uri, &lang);
        assert_eq!(result, Some(workspace.join("src/config.ts")));

        let result = resolver.resolve("./utils/env", &from_uri, &lang);
        assert_eq!(result, Some(workspace.join("src/utils/env.ts")));
    }

    #[test]
    fn test_resolve_parent_directory() {
        let (_temp, workspace) = setup_test_workspace();
        let resolver = ModuleResolver::new(workspace.clone());
        let lang = MockLanguage {
            extensions: &["ts", "tsx", "js", "jsx"],
        };

        let from_uri = Url::from_file_path(workspace.join("src/utils/helpers.js")).unwrap();

        let result = resolver.resolve("../config", &from_uri, &lang);
        assert_eq!(result, Some(workspace.join("src/config.ts")));
    }

    #[test]
    fn test_resolve_index_file() {
        let (_temp, workspace) = setup_test_workspace();
        let resolver = ModuleResolver::new(workspace.clone());
        let lang = MockLanguage {
            extensions: &["ts", "tsx", "js", "jsx"],
        };

        let from_uri = Url::from_file_path(workspace.join("src/index.ts")).unwrap();

        let result = resolver.resolve("./config", &from_uri, &lang);
        assert_eq!(result, Some(workspace.join("src/config.ts")));
    }

    #[test]
    fn test_no_resolve_package_import() {
        let (_temp, workspace) = setup_test_workspace();
        let resolver = ModuleResolver::new(workspace.clone());
        let lang = MockLanguage {
            extensions: &["ts", "tsx", "js", "jsx"],
        };

        let from_uri = Url::from_file_path(workspace.join("src/index.ts")).unwrap();

        assert!(resolver.resolve("lodash", &from_uri, &lang).is_none());
        assert!(resolver.resolve("@scope/pkg", &from_uri, &lang).is_none());
        assert!(resolver.resolve("react", &from_uri, &lang).is_none());
    }

    #[test]
    fn test_no_resolve_absolute_import() {
        let (_temp, workspace) = setup_test_workspace();
        let resolver = ModuleResolver::new(workspace.clone());
        let lang = MockLanguage {
            extensions: &["ts", "tsx", "js", "jsx"],
        };

        let from_uri = Url::from_file_path(workspace.join("src/index.ts")).unwrap();

        assert!(resolver
            .resolve("/absolute/path", &from_uri, &lang)
            .is_none());
    }

    #[test]
    fn test_no_resolve_outside_workspace() {
        let (_temp, workspace) = setup_test_workspace();
        let resolver = ModuleResolver::new(workspace.clone());
        let lang = MockLanguage {
            extensions: &["ts", "tsx", "js", "jsx"],
        };

        let from_uri = Url::from_file_path(workspace.join("src/index.ts")).unwrap();

        assert!(resolver
            .resolve("../../outside/workspace", &from_uri, &lang)
            .is_none());
    }

    #[test]
    fn test_is_relative_import() {
        assert!(ModuleResolver::is_relative_import("./config"));
        assert!(ModuleResolver::is_relative_import("../utils"));
        assert!(!ModuleResolver::is_relative_import("lodash"));
        assert!(!ModuleResolver::is_relative_import("@scope/pkg"));
        assert!(!ModuleResolver::is_relative_import("/absolute"));
    }

    #[test]
    fn test_is_package_import() {
        assert!(ModuleResolver::is_package_import("lodash"));
        assert!(ModuleResolver::is_package_import("@scope/pkg"));
        assert!(ModuleResolver::is_package_import("react"));
        assert!(!ModuleResolver::is_package_import("./config"));
        assert!(!ModuleResolver::is_package_import("../utils"));
        assert!(!ModuleResolver::is_package_import("/absolute"));
    }

    #[test]
    fn test_resolve_to_uri() {
        let (_temp, workspace) = setup_test_workspace();
        let resolver = ModuleResolver::new(workspace.clone());
        let lang = MockLanguage {
            extensions: &["ts", "tsx", "js", "jsx"],
        };

        let from_uri = Url::from_file_path(workspace.join("src/index.ts")).unwrap();

        let result = resolver.resolve_to_uri("./config", &from_uri, &lang);
        assert!(result.is_some());
        let uri = result.unwrap();
        assert!(uri.path().ends_with("config.ts"));
    }

    #[test]
    fn test_normalize_path() {
        let path = Path::new("/workspace/src/../src/./config");
        let normalized = normalize_path(path);
        assert_eq!(normalized, PathBuf::from("/workspace/src/config"));
    }
}
