use ecolog_lsp::server::LspServer;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let mut config_manager = ecolog_lsp::server::config::ConfigManager::new();

    let config = config_manager
        .load_from_workspace(&root)
        .await
        .expect("Failed to load configuration");

    let abundantis_config = config.to_abundantis_config();
    let core = abundantis::Abundantis::builder()
        .root(&root)
        .precedence(abundantis_config.resolution.precedence)
        .env_files(abundantis_config.workspace.env_files)
        .interpolation(abundantis_config.interpolation.enabled)
        .max_interpolation_depth(abundantis_config.interpolation.max_depth)
        .interpolation_features(abundantis_config.interpolation.features)
        .cache_enabled(abundantis_config.cache.enabled)
        .cache_size(abundantis_config.cache.hot_cache_size)
        .cache_ttl(abundantis_config.cache.ttl)
        .build()
        .await
        .expect("Failed to initialize Ecolog core");

    let shelter_config = config.masking.to_shelter_config();
    let masker = Arc::new(Mutex::new(shelter::Masker::new(shelter_config)));

    config_manager.set_masker(masker.clone());
    let config_arc = Arc::new(config_manager);

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) =
        LspService::new(|client| LspServer::new_with_config(client, core, masker, config_arc));
    Server::new(stdin, stdout, socket).serve(service).await;
}
