pub mod cross_module_resolver;
pub mod document;
pub mod graph;
pub mod indexer;
pub mod module_resolver;
pub mod pipeline;
pub mod query;
pub mod range_utils;
pub mod resolver;
pub mod workspace_index;

pub use graph::BindingGraph;
pub use cross_module_resolver::{CrossModuleResolution, CrossModuleResolver};
pub use document::{DocumentEntry, DocumentManager};
pub use indexer::WorkspaceIndexer;
pub use module_resolver::ModuleResolver;
pub use pipeline::{ts_to_lsp_range, AnalysisPipeline};
pub use query::QueryEngine;
pub use resolver::BindingResolver;
pub use workspace_index::{
    EnvVarLocation, FileIndexEntry, IndexStats, IndexStateSnapshot, LocationKind, WorkspaceIndex,
};
