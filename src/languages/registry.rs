use crate::languages::LanguageSupport;
use std::collections::HashMap;
use std::sync::Arc;
use tower_lsp::lsp_types::Url;

pub struct LanguageRegistry {
    by_id: HashMap<&'static str, Arc<dyn LanguageSupport>>,

    by_extension: HashMap<&'static str, Arc<dyn LanguageSupport>>,

    by_language_id: HashMap<&'static str, Arc<dyn LanguageSupport>>,
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageRegistry {
    pub fn new() -> Self {
        Self {
            by_id: HashMap::new(),
            by_extension: HashMap::new(),
            by_language_id: HashMap::new(),
        }
    }

    pub fn register(&mut self, language: Arc<dyn LanguageSupport>) {
        let lang = language.clone();
        self.by_id.insert(lang.id(), lang.clone());

        for ext in lang.extensions() {
            self.by_extension.insert(ext, lang.clone());
        }

        for id in lang.language_ids() {
            self.by_language_id.insert(id, lang.clone());
        }
    }

    pub fn get_by_extension(&self, ext: &str) -> Option<Arc<dyn LanguageSupport>> {
        self.by_extension.get(ext).cloned()
    }

    pub fn get_by_language_id(&self, id: &str) -> Option<Arc<dyn LanguageSupport>> {
        self.by_language_id.get(id).cloned()
    }

    pub fn get_for_uri(&self, uri: &Url) -> Option<Arc<dyn LanguageSupport>> {
        let path = uri.to_file_path().ok()?;
        let ext = path.extension()?.to_str()?;
        self.get_by_extension(ext)
    }

    pub fn all_languages(&self) -> Vec<Arc<dyn LanguageSupport>> {
        self.by_id.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::javascript::JavaScript;
    use crate::languages::python::Python;
    use crate::languages::typescript::TypeScript;

    #[test]
    fn test_new_registry_is_empty() {
        let registry = LanguageRegistry::new();
        assert!(registry.all_languages().is_empty());
    }

    #[test]
    fn test_default_is_empty() {
        let registry = LanguageRegistry::default();
        assert!(registry.all_languages().is_empty());
    }

    #[test]
    fn test_register_language() {
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(JavaScript));
        assert_eq!(registry.all_languages().len(), 1);
    }

    #[test]
    fn test_register_multiple_languages() {
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(JavaScript));
        registry.register(Arc::new(Python));
        registry.register(Arc::new(TypeScript));
        assert_eq!(registry.all_languages().len(), 3);
    }

    #[test]
    fn test_get_by_extension() {
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(JavaScript));

        let lang = registry.get_by_extension("js");
        assert!(lang.is_some());
        assert_eq!(lang.unwrap().id(), "javascript");

        let missing = registry.get_by_extension("xyz");
        assert!(missing.is_none());
    }

    #[test]
    fn test_get_by_extension_multiple() {
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(TypeScript));

        // TypeScript registers both .ts and .tsx
        let ts = registry.get_by_extension("ts");
        assert!(ts.is_some());
    }

    #[test]
    fn test_get_by_language_id() {
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(Python));

        let lang = registry.get_by_language_id("python");
        assert!(lang.is_some());
        assert_eq!(lang.unwrap().id(), "python");

        let missing = registry.get_by_language_id("ruby");
        assert!(missing.is_none());
    }

    #[test]
    fn test_get_for_uri_javascript() {
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(JavaScript));

        let uri = Url::parse("file:///path/to/file.js").unwrap();
        let lang = registry.get_for_uri(&uri);
        assert!(lang.is_some());
        assert_eq!(lang.unwrap().id(), "javascript");
    }

    #[test]
    fn test_get_for_uri_typescript() {
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(TypeScript));

        let uri = Url::parse("file:///path/to/file.ts").unwrap();
        let lang = registry.get_for_uri(&uri);
        assert!(lang.is_some());
        assert_eq!(lang.unwrap().id(), "typescript");
    }

    #[test]
    fn test_get_for_uri_unknown_extension() {
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(JavaScript));

        let uri = Url::parse("file:///path/to/file.unknown").unwrap();
        let lang = registry.get_for_uri(&uri);
        assert!(lang.is_none());
    }

    #[test]
    fn test_get_for_uri_no_extension() {
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(JavaScript));

        let uri = Url::parse("file:///path/to/Makefile").unwrap();
        let lang = registry.get_for_uri(&uri);
        assert!(lang.is_none());
    }

    #[test]
    fn test_get_for_uri_invalid_uri() {
        let registry = LanguageRegistry::new();
        // Non-file URIs can't be converted to file paths
        let uri = Url::parse("https://example.com/file.js").unwrap();
        let lang = registry.get_for_uri(&uri);
        assert!(lang.is_none());
    }
}
