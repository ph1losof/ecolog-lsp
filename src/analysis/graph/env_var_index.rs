//! Environment variable indexing for the binding graph.
//!
//! This module provides:
//! - `EnvVarLocation` and `EnvVarLocationKind` types for tracking where env vars are used
//! - Building and querying the env var index
//! - Lazy resolution caching

use super::BindingGraph;
use crate::constants::MAX_CHAIN_DEPTH;
use crate::types::{ResolvedEnv, SymbolId};
use compact_str::CompactString;
use rustc_hash::FxHashSet;
use tower_lsp::lsp_types::Range;

/// A location where an env var is referenced.
#[derive(Debug, Clone)]
pub struct EnvVarLocation {
    /// The range of the reference
    pub range: Range,
    /// The kind of reference
    pub kind: EnvVarLocationKind,
    /// The name of the binding (if this is a binding or usage)
    pub binding_name: Option<CompactString>,
}

/// The kind of environment variable reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvVarLocationKind {
    /// Direct access like `process.env.DATABASE_URL`
    DirectReference,
    /// Declaration of a binding like `const db = process.env.DATABASE_URL`
    BindingDeclaration,
    /// Usage of a binding like `db` after `const db = process.env.DATABASE_URL`
    BindingUsage,
    /// Property access like `env.DATABASE_URL` where `env` is `process.env`
    PropertyAccess,
}

impl BindingGraph {
    /// Get all locations where the given env var is used.
    ///
    /// O(1) lookup after `rebuild_range_index` has been called.
    pub fn get_env_var_locations(&self, env_var_name: &str) -> Option<&Vec<EnvVarLocation>> {
        self.env_var_index.get(env_var_name)
    }

    /// Helper to get or compute a symbol's resolution, caching the result.
    ///
    /// This avoids pre-computing all resolutions and only computes on-demand.
    pub(crate) fn get_or_compute_resolution(&mut self, symbol_id: SymbolId) -> Option<ResolvedEnv> {
        // Check cache first
        if let Some(cached) = self.resolution_cache.get(&symbol_id) {
            return cached.clone();
        }

        // Compute and cache
        let resolved = self.resolve_to_env_with_depth(symbol_id, MAX_CHAIN_DEPTH, 0);
        self.resolution_cache.insert(symbol_id, resolved.clone());
        resolved
    }

    /// Builds the env_var_index for fast lookup of all locations referencing a given env var.
    ///
    /// Also populates the resolution cache lazily as symbols are processed.
    pub(crate) fn build_env_var_index(&mut self) {
        // Retain capacity to reduce allocations on rebuild
        self.env_var_index.clear();
        self.resolution_cache.clear();

        // Track seen ranges for deduplication
        let mut seen_ranges: FxHashSet<(u32, u32, u32, u32)> = FxHashSet::default();

        // Index direct references
        for reference in &self.direct_references {
            let range_key = (
                reference.name_range.start.line,
                reference.name_range.start.character,
                reference.name_range.end.line,
                reference.name_range.end.character,
            );
            if seen_ranges.insert(range_key) {
                self.env_var_index
                    .entry(reference.name.clone())
                    .or_default()
                    .push(EnvVarLocation {
                        range: reference.name_range,
                        kind: EnvVarLocationKind::DirectReference,
                        binding_name: None,
                    });
            }
        }

        // Collect symbol data to avoid borrow issues
        let symbol_data: Vec<_> = self
            .symbols
            .iter()
            .map(|s| (s.id, s.name.clone(), s.name_range, s.destructured_key_range))
            .collect();

        // Index symbols that resolve to env vars
        for (symbol_id, symbol_name, name_range, destructured_key_range) in symbol_data {
            let resolved = self.get_or_compute_resolution(symbol_id);
            if let Some(ResolvedEnv::Variable(name)) = resolved {
                // Determine the range to index
                let index_range = if let Some(key_range) = destructured_key_range {
                    Some(key_range)
                } else if symbol_name.as_str() == name.as_str() {
                    Some(name_range)
                } else {
                    None
                };

                if let Some(range) = index_range {
                    let range_key = (
                        range.start.line,
                        range.start.character,
                        range.end.line,
                        range.end.character,
                    );
                    if seen_ranges.insert(range_key) {
                        self.env_var_index
                            .entry(name.clone())
                            .or_default()
                            .push(EnvVarLocation {
                                range,
                                kind: EnvVarLocationKind::BindingDeclaration,
                                binding_name: Some(symbol_name),
                            });
                    }
                }
            }
        }

        // Collect usage data to avoid borrow issues
        let usage_data: Vec<_> = self
            .usages
            .iter()
            .map(|u| {
                (
                    u.symbol_id,
                    u.range,
                    u.property_access.clone(),
                    u.property_access_range,
                )
            })
            .collect();

        // Index usages
        for (symbol_id, usage_range, property_access, property_access_range) in usage_data {
            let resolved = self.get_or_compute_resolution(symbol_id);
            if let Some(resolved) = resolved {
                match &resolved {
                    ResolvedEnv::Variable(name) => {
                        let range_key = (
                            usage_range.start.line,
                            usage_range.start.character,
                            usage_range.end.line,
                            usage_range.end.character,
                        );
                        if seen_ranges.insert(range_key) {
                            let binding_name = self.get_symbol(symbol_id).map(|s| s.name.clone());
                            self.env_var_index
                                .entry(name.clone())
                                .or_default()
                                .push(EnvVarLocation {
                                    range: usage_range,
                                    kind: EnvVarLocationKind::BindingUsage,
                                    binding_name,
                                });
                        }
                    }
                    ResolvedEnv::Object(_) => {
                        if let Some(prop) = &property_access {
                            let range = property_access_range.unwrap_or(usage_range);
                            let range_key = (
                                range.start.line,
                                range.start.character,
                                range.end.line,
                                range.end.character,
                            );
                            if seen_ranges.insert(range_key) {
                                let binding_name =
                                    self.get_symbol(symbol_id).map(|s| s.name.clone());
                                self.env_var_index
                                    .entry(prop.clone())
                                    .or_default()
                                    .push(EnvVarLocation {
                                        range,
                                        kind: EnvVarLocationKind::PropertyAccess,
                                        binding_name,
                                    });
                            }
                        }
                    }
                }
            }
        }
    }
}
