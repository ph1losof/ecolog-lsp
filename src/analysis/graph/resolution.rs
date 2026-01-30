//! Chain resolution logic for the binding graph.
//!
//! This module handles resolving symbol chains to their ultimate origin:
//! - Following `const b = a` chains
//! - Resolving destructured properties
//! - Determining if a symbol resolves to an env var or env object

use super::BindingGraph;
use crate::constants::MAX_CHAIN_DEPTH;
use crate::types::{ResolvedEnv, SymbolId, SymbolOrigin};
use compact_str::CompactString;

impl BindingGraph {
    /// Resolve a symbol to its env var or env object.
    ///
    /// Uses cached results if available, otherwise walks the chain.
    pub fn resolve_to_env(&self, symbol_id: SymbolId) -> Option<ResolvedEnv> {
        // Try cache first (O(1) lookup)
        if let Some(cached) = self.resolution_cache.get(&symbol_id) {
            return cached.clone();
        }
        // Fall back to walking the chain (for queries before rebuild_range_index)
        self.resolve_to_env_with_depth(symbol_id, MAX_CHAIN_DEPTH, 0)
    }

    /// Resolve a symbol with a custom maximum chain depth.
    pub fn resolve_to_env_with_max(
        &self,
        symbol_id: SymbolId,
        max_depth: usize,
    ) -> Option<ResolvedEnv> {
        self.resolve_to_env_with_depth(symbol_id, max_depth, 0)
    }

    /// Internal recursive resolution with depth tracking.
    pub(crate) fn resolve_to_env_with_depth(
        &self,
        symbol_id: SymbolId,
        max_depth: usize,
        current_depth: usize,
    ) -> Option<ResolvedEnv> {
        if current_depth >= max_depth {
            return None;
        }

        let symbol = self.get_symbol(symbol_id)?;

        match &symbol.origin {
            SymbolOrigin::EnvVar { name } => Some(ResolvedEnv::Variable(name.clone())),

            SymbolOrigin::EnvObject { canonical_name } => {
                Some(ResolvedEnv::Object(canonical_name.clone()))
            }

            SymbolOrigin::Symbol { target } => {
                self.resolve_to_env_with_depth(*target, max_depth, current_depth + 1)
            }

            SymbolOrigin::DestructuredProperty { source, key } => {
                match self.resolve_to_env_with_depth(*source, max_depth, current_depth + 1)? {
                    ResolvedEnv::Object(_) => Some(ResolvedEnv::Variable(key.clone())),
                    ResolvedEnv::Variable(_) => None,
                }
            }

            SymbolOrigin::Unknown
            | SymbolOrigin::Unresolvable
            | SymbolOrigin::UnresolvedSymbol { .. }
            | SymbolOrigin::UnresolvedDestructure { .. } => None,
        }
    }

    /// Check if a symbol resolves to an environment object (e.g., `process.env`).
    pub fn resolves_to_env_object(&self, symbol_id: SymbolId) -> bool {
        matches!(self.resolve_to_env(symbol_id), Some(ResolvedEnv::Object(_)))
    }

    /// Get the environment variable name that a symbol resolves to, if any.
    ///
    /// Returns `None` if the symbol resolves to an env object instead of a variable.
    pub fn get_env_var_name(&self, symbol_id: SymbolId) -> Option<CompactString> {
        match self.resolve_to_env(symbol_id)? {
            ResolvedEnv::Variable(name) => Some(name),
            ResolvedEnv::Object(_) => None,
        }
    }
}
