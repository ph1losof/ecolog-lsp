//! Benchmark harness for workspace indexing.
//!
//! Usage: cargo run --release --example bench_index -- <workspace-root>

use ecolog_lsp::analysis::{QueryEngine, WorkspaceIndexer};
use ecolog_lsp::analysis::workspace_index::WorkspaceIndex;
use ecolog_lsp::server::config::IndexingConfig;
use compact_str::CompactString;
use std::sync::Arc;
use std::time::Instant;

fn main() {
    tracing_subscriber::fmt().with_env_filter("ecolog_lsp=info").init();
    let root = std::env::args().nth(1).expect("usage: bench_index <root>");
    let root = std::path::PathBuf::from(root);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async move {
        let registry = ecolog_lsp::languages::LanguageRegistry::with_all_languages();
        let index = Arc::new(WorkspaceIndex::new());
        let indexer = WorkspaceIndexer::new(
            index.clone(),
            Arc::new(QueryEngine::new()),
            Arc::new(registry),
            root,
        );

        let env_files = vec![CompactString::new(".env"), CompactString::new(".env.*")];
        let cfg = IndexingConfig {
            max_files: 1_000_000,
            ..IndexingConfig::default()
        };

        let start = Instant::now();
        indexer.index_workspace(&env_files, &cfg).await.unwrap();
        let elapsed = start.elapsed();

        let stats = index.stats();
        println!(
            "wall={:?} files={} env_files={} env_vars={}",
            elapsed, stats.total_files, stats.env_files, stats.total_env_vars
        );
    });
}
