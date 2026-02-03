use ecolog_lsp::server::LspServer;
use std::sync::Arc;
use tower_lsp::{LspService, Server};
use tracing_subscriber::EnvFilter;

#[cfg(feature = "doppler")]
use abundantis::source::remote::providers::DopplerSourceFactory;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let initial_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let config_manager = ecolog_lsp::server::config::ConfigManager::new();

    let config_future = config_manager.load_from_workspace(&initial_root);
    let core_future = {
        let root = initial_root.clone();
        async move { abundantis::Abundantis::builder().root(&root).build().await }
    };

    let (config_result, core_result) = tokio::join!(config_future, core_future);

    let config = config_result.expect("Failed to load configuration");
    let core = core_result.expect("Failed to initialize Ecolog core");

    // Register remote source factories
    #[cfg(feature = "doppler")]
    core.registry.register_remote_factory(Arc::new(DopplerSourceFactory));

    let abundantis_config = config.to_abundantis_config();
    core.resolution
        .update_resolution_config(abundantis_config.resolution);
    core.resolution
        .update_interpolation_config(abundantis_config.interpolation);

    let config_arc = Arc::new(config_manager);

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) =
        LspService::new(|client| LspServer::new_with_config(client, core, config_arc));
    Server::new(stdin, stdout, socket).serve(service).await;
}
