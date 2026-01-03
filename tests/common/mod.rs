use ecolog_lsp::analysis::DocumentManager;
use ecolog_lsp::server::state::ServerState;
use ecolog_lsp::languages::LanguageRegistry;
use ecolog_lsp::server::config::ConfigManager;
use abundantis::Abundantis;
use shelter::masker::Masker;
use shelter::MaskingConfig;
use tokio::sync::Mutex;
use std::sync::Arc;
use std::fs::{self, File};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};
use tower_lsp::lsp_types::Url;
use std::sync::atomic::{AtomicU64, Ordering};

// Global atomic counter to ensure unique temp directory names
static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TestFixture {
    pub temp_dir: std::path::PathBuf,
    pub state: ServerState,
}

impl TestFixture {
    pub async fn new() -> Self {
        // Setup unique temp dir using both timestamp and atomic counter to prevent collisions
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!("ecolog_test_{}_{}", timestamp, counter));
        fs::create_dir_all(&temp_dir).unwrap();
        
        // Set CWD for Abundantis - REMOVED to avoid parallel test race conditions
        // std::env::set_current_dir(&temp_dir).unwrap();
        
        // Create standard .env
        let env_path = temp_dir.join(".env");
        let mut env_file = File::create(&env_path).unwrap();
        writeln!(env_file, "DB_URL=postgres://localhost:5432").unwrap();
        writeln!(env_file, "API_KEY=secret_key").unwrap();
        writeln!(env_file, "DEBUG=true").unwrap();
        writeln!(env_file, "PORT=8080").unwrap();

        // Setup Server
        let mut registry = LanguageRegistry::new();
        registry.register(Arc::new(ecolog_lsp::languages::javascript::JavaScript));
        registry.register(Arc::new(ecolog_lsp::languages::typescript::TypeScript));
        registry.register(Arc::new(ecolog_lsp::languages::typescript::TypeScriptReact));
        registry.register(Arc::new(ecolog_lsp::languages::python::Python));
        registry.register(Arc::new(ecolog_lsp::languages::rust::Rust));
        registry.register(Arc::new(ecolog_lsp::languages::go::Go));
        let languages = Arc::new(registry);
        
        let query_engine = Arc::new(ecolog_lsp::analysis::QueryEngine::new());
        let document_manager = Arc::new(DocumentManager::new(query_engine, languages.clone()));
        let mut config_manager = ConfigManager::new();
        let core = Arc::new(Abundantis::builder()
            .root(&temp_dir)
            .build().await.expect("Failed to build Abundantis"));
        let masker = Arc::new(Mutex::new(Masker::new(MaskingConfig::default())));

        config_manager.set_masker(masker.clone());
        let config_manager = Arc::new(config_manager);

        let state = ServerState::new(
            document_manager,
            languages,
            core,
            masker,
            config_manager
        );

        Self {
            temp_dir,
            state,
        }
    }

    pub fn create_file(&self, name: &str, content: &str) -> Url {
        let path = self.temp_dir.join(name);
        let mut f = File::create(&path).unwrap();
        write!(f, "{}", content).unwrap();
        Url::from_file_path(&path).unwrap()
    }
}

impl Drop for TestFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temp_dir);
    }
}
