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
