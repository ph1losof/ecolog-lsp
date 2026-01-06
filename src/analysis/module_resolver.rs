//! Module Resolution for Cross-Module Import Tracking
//!
//! This module provides workspace-relative module path resolution for supporting
//! cross-module env var tracking. It resolves import specifiers (e.g., "./config")
//! to absolute file paths within the workspace.
//!
//! ## Design Principles
//!
//! - **Language-agnostic**: Uses `LanguageSupport::extensions()` for extension inference
//! - **Workspace-only**: Only resolves relative paths within the project
//! - **No external dependencies**: Does not resolve node_modules, cargo deps, etc.
//!
//! ## Supported Import Patterns
//!
//! - `./relative/path` - Relative to current file
//! - `../parent/path` - Parent directory traversal
//!
//! ## NOT Supported (returns None)
//!
//! - Absolute paths (`/absolute/path`)
//! - Package imports (`lodash`, `@scope/pkg`)
//! - Language-specific path mappings (tsconfig paths, Cargo.toml)

use crate::languages::LanguageSupport;
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::Url;

/// Resolves module import specifiers to file paths within the workspace.
///
/// This resolver is designed to be language-agnostic and only handles
/// workspace-relative imports (paths starting with "./" or "../").
#[derive(Debug, Clone)]
pub struct ModuleResolver {
    /// The workspace root directory.
    workspace_root: PathBuf,
}

impl ModuleResolver {
    /// Creates a new ModuleResolver with the given workspace root.
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    /// Get the workspace root path.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Resolve an import specifier to an absolute file path.
    ///
    /// Only resolves relative imports (starting with "./" or "../").
    /// Returns `None` for absolute or package imports.
    ///
    /// # Arguments
    ///
    /// * `specifier` - The import path (e.g., "./config", "../utils/env")
    /// * `from_uri` - The URI of the importing file
    /// * `language` - The language support for extension inference
    ///
    /// # Returns
    ///
    /// * `Some(PathBuf)` - The resolved absolute path if found
    /// * `None` - If the specifier is not relative or file not found
    pub fn resolve(
        &self,
        specifier: &str,
        from_uri: &Url,
        language: &dyn LanguageSupport,
    ) -> Option<PathBuf> {
        // Only handle relative imports
        if !specifier.starts_with("./") && !specifier.starts_with("../") {
            return None;
        }

        let from_path = from_uri.to_file_path().ok()?;
        let from_dir = from_path.parent()?;
        let base_path = from_dir.join(specifier);

        // Normalize the path (resolve .. and .)
        let normalized = normalize_path(&base_path);

        // Ensure the resolved path is within the workspace
        if !normalized.starts_with(&self.workspace_root) {
            return None;
        }

        self.resolve_with_extensions(&normalized, language)
    }

    /// Resolve an import specifier to a file URI.
    ///
    /// Convenience method that wraps `resolve()` and converts to `Url`.
    pub fn resolve_to_uri(
        &self,
        specifier: &str,
        from_uri: &Url,
        language: &dyn LanguageSupport,
    ) -> Option<Url> {
        let path = self.resolve(specifier, from_uri, language)?;
        Url::from_file_path(path).ok()
    }

    /// Try to resolve a path by adding language-specific extensions.
    ///
    /// Resolution order:
    /// 1. Path as-is (if exists)
    /// 2. Path + each language extension (appended, e.g., "./foo" -> "./foo.ts")
    /// 3. Path/index + each language extension
    fn resolve_with_extensions(
        &self,
        base_path: &Path,
        language: &dyn LanguageSupport,
    ) -> Option<PathBuf> {
        // If path already exists as-is, return it
        if base_path.exists() && base_path.is_file() {
            return Some(base_path.to_path_buf());
        }

        // Try each language extension by APPENDING (not replacing)
        // This handles filenames with dots like "change-settings.input" correctly
        // where we want "change-settings.input.ts", not "change-settings.ts"
        let base_str = base_path.to_string_lossy();
        for ext in language.extensions() {
            let with_ext = PathBuf::from(format!("{}.{}", base_str, ext));
            if with_ext.exists() {
                return Some(with_ext);
            }
        }

        // Try index file resolution (e.g., ./config -> ./config/index.ts)
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

    /// Check if an import specifier is a relative import.
    ///
    /// Returns `true` for specifiers starting with "./" or "../".
    #[inline]
    pub fn is_relative_import(specifier: &str) -> bool {
        specifier.starts_with("./") || specifier.starts_with("../")
    }

    /// Check if an import specifier is a package import (not relative).
    ///
    /// Returns `true` for package imports like "lodash", "@scope/pkg".
    #[inline]
    pub fn is_package_import(specifier: &str) -> bool {
        !specifier.starts_with("./")
            && !specifier.starts_with("../")
            && !specifier.starts_with('/')
    }
}

/// Normalize a path by resolving `.` and `..` components.
///
/// Unlike `canonicalize()`, this doesn't require the path to exist
/// and doesn't resolve symlinks.
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();

    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                // Pop the last component if possible
                if !components.is_empty() {
                    components.pop();
                }
            }
            std::path::Component::CurDir => {
                // Skip current directory markers
            }
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

    // Mock language support for testing
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

        // Create directory structure
        fs::create_dir_all(workspace.join("src/utils")).unwrap();
        fs::create_dir_all(workspace.join("src/config")).unwrap();

        // Create test files
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

        // Should resolve ./config to ./config.ts
        let result = resolver.resolve("./config", &from_uri, &lang);
        assert_eq!(result, Some(workspace.join("src/config.ts")));

        // Should resolve ./utils/env to ./utils/env.ts
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

        // Should resolve ../config to ./config.ts
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

        // Should resolve ./config to ./config/index.ts when ./config.ts doesn't match
        // But in our setup ./config.ts exists, so it should resolve to that first
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

        // Should not resolve package imports
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

        // Should not resolve absolute paths
        assert!(resolver.resolve("/absolute/path", &from_uri, &lang).is_none());
    }

    #[test]
    fn test_no_resolve_outside_workspace() {
        let (_temp, workspace) = setup_test_workspace();
        let resolver = ModuleResolver::new(workspace.clone());
        let lang = MockLanguage {
            extensions: &["ts", "tsx", "js", "jsx"],
        };

        let from_uri = Url::from_file_path(workspace.join("src/index.ts")).unwrap();

        // Should not resolve paths that go outside workspace
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
