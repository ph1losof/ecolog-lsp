use crate::analysis::DocumentManager;
use crate::languages::LanguageRegistry;
use abundantis::Abundantis;
use shelter::Masker;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::server::config::ConfigManager;

#[derive(Clone)]
pub struct ServerState {
    pub document_manager: Arc<DocumentManager>,
    pub languages: Arc<LanguageRegistry>,
    pub core: Arc<Abundantis>,
    pub masker: Arc<Mutex<Masker>>,
    pub config: Arc<ConfigManager>,
}

impl ServerState {
    pub fn new(
        document_manager: Arc<DocumentManager>,
        languages: Arc<LanguageRegistry>,
        core: Arc<Abundantis>,
        masker: Arc<Mutex<Masker>>,
        config: Arc<ConfigManager>,
    ) -> Self {
        Self {
            document_manager,
            languages,
            core,
            masker,
            config,
        }
    }
}
