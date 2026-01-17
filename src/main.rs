use ecolog_lsp::server::LspServer;
use std::sync::Arc;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let initial_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let config_manager = ecolog_lsp::server::config::ConfigManager::new();

    let config = config_manager
        .load_from_workspace(&initial_root)
        .await
        .expect("Failed to load configuration");

    // Use configured workspace.root if provided, otherwise use current_dir
    let workspace_root = config
        .workspace
        .root
        .as_ref()
        .and_then(|p| p.canonicalize().ok())
        .unwrap_or(initial_root);

    let abundantis_config = config.to_abundantis_config();
    let core = abundantis::Abundantis::builder()
        .root(&workspace_root)
        .source_defaults(abundantis_config.sources.defaults)
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

    let config_arc = Arc::new(config_manager);

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) =
        LspService::new(|client| LspServer::new_with_config(client, core, config_arc));
    Server::new(stdin, stdout, socket).serve(service).await;
}
