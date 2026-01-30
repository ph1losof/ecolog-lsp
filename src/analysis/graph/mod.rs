//! Binding graph for tracking symbol bindings and environment variable references.
//!
//! The binding graph is the core data structure for tracking:
//! - Symbol declarations and their scopes
//! - Environment variable bindings and chains
//! - Usages of symbols and their property accesses
//! - Direct env var references
//!
//! This module is split into focused sub-modules:
//! - `core`: Arena storage and basic symbol/scope CRUD operations
//! - `indexing`: Interval trees and position-based lookups
//! - `resolution`: Chain resolution and origin tracking
//! - `env_var_index`: Environment variable indexing
//! - `operations`: Bulk operations and utilities

mod core;
mod env_var_index;
mod indexing;
mod operations;
mod resolution;

pub use env_var_index::{EnvVarLocation, EnvVarLocationKind};
pub use operations::BindingGraphStats;

// Re-export for backwards compatibility
pub use crate::constants::{MAX_CHAIN_DEPTH, RANGE_SIZE_LINE_WEIGHT};

use crate::types::{EnvReference, ResolvedEnv, Scope, ScopeId, Symbol, SymbolId, SymbolUsage};
use compact_str::CompactString;
use intervaltree::IntervalTree;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use tower_lsp::lsp_types::Range;

/// Entry for building interval trees during analysis
#[derive(Debug, Clone)]
pub(crate) struct PendingRangeEntry<T: Clone> {
    pub(crate) range: Range,
    pub(crate) value: T,
}

/// The main binding graph structure.
///
/// Tracks symbol declarations, scopes, usages, and environment variable references
/// for a single document. Uses interval trees for efficient position lookups and
/// maintains various indices for fast name-based and env var lookups.
#[derive(Debug)]
pub struct BindingGraph {
    /// Arena storage for symbols
    pub(crate) symbols: Vec<Symbol>,

    /// Arena storage for scopes
    pub(crate) scopes: Vec<Scope>,

    /// Index for fast lookup of symbols by (name, scope)
    pub(crate) name_index: FxHashMap<(CompactString, ScopeId), SmallVec<[SymbolId; 2]>>,

    /// Index for fast lookup of all symbols by name only (ignoring scope)
    pub(crate) name_only_index: FxHashMap<CompactString, SmallVec<[SymbolId; 4]>>,

    /// Direct references to environment variables (e.g., `process.env.DATABASE_URL`)
    pub(crate) direct_references: Vec<EnvReference>,

    /// Usages of symbols (e.g., reading a variable that was bound to an env var)
    pub(crate) usages: Vec<SymbolUsage>,

    /// Pending entries for destructure range index (built into tree on rebuild)
    pub(crate) pending_destructure_entries: Vec<PendingRangeEntry<SymbolId>>,

    /// Pending entries for symbol range index (built into tree on rebuild)
    pub(crate) pending_symbol_entries: Vec<PendingRangeEntry<SymbolId>>,

    /// Pending entries for usage range index (built into tree on rebuild)
    pub(crate) pending_usage_entries: Vec<PendingRangeEntry<usize>>,

    /// Pending entries for scope range index (built into tree on rebuild)
    /// Stores (scope_id, size) where size is used to find the most specific scope
    pub(crate) pending_scope_entries: Vec<PendingRangeEntry<(ScopeId, u64)>>,

    /// Interval tree for O(log n) destructure key lookup by position
    pub(crate) destructure_range_tree: Option<IntervalTree<u64, SymbolId>>,

    /// Interval tree for O(log n) symbol lookup by position
    pub(crate) symbol_range_tree: Option<IntervalTree<u64, SymbolId>>,

    /// Interval tree for O(log n) usage lookup by position
    pub(crate) usage_range_tree: Option<IntervalTree<u64, usize>>,

    /// Interval tree for O(log n) scope lookup by position
    pub(crate) scope_range_tree: Option<IntervalTree<u64, (ScopeId, u64)>>,

    /// Index for O(1) lookup of all usages of a given env var name
    pub(crate) env_var_index: FxHashMap<CompactString, Vec<EnvVarLocation>>,

    /// Cache for resolved env vars (symbol_id -> resolved result)
    pub(crate) resolution_cache: FxHashMap<SymbolId, Option<ResolvedEnv>>,

    /// Cache for scope lookups by position (line, character) -> ScopeId
    pub(crate) scope_cache: RwLock<FxHashMap<(u32, u32), ScopeId>>,

    /// Next symbol ID to assign
    pub(crate) next_symbol_id: u32,

    /// Next scope ID to assign
    pub(crate) next_scope_id: u32,
}

impl Clone for BindingGraph {
    fn clone(&self) -> Self {
        Self {
            symbols: self.symbols.clone(),
            scopes: self.scopes.clone(),
            name_index: self.name_index.clone(),
            name_only_index: self.name_only_index.clone(),
            direct_references: self.direct_references.clone(),
            usages: self.usages.clone(),
            pending_destructure_entries: self.pending_destructure_entries.clone(),
            pending_symbol_entries: self.pending_symbol_entries.clone(),
            pending_usage_entries: self.pending_usage_entries.clone(),
            pending_scope_entries: self.pending_scope_entries.clone(),
            destructure_range_tree: self.destructure_range_tree.clone(),
            symbol_range_tree: self.symbol_range_tree.clone(),
            usage_range_tree: self.usage_range_tree.clone(),
            scope_range_tree: self.scope_range_tree.clone(),
            env_var_index: self.env_var_index.clone(),
            resolution_cache: self.resolution_cache.clone(),
            // Create empty cache for clone - it will be repopulated on demand
            scope_cache: RwLock::new(FxHashMap::default()),
            next_symbol_id: self.next_symbol_id,
            next_scope_id: self.next_scope_id,
        }
    }
}

impl Default for BindingGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
