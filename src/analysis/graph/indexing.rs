//! Interval tree indexing and position-based lookups.
//!
//! This module provides O(log n) position-based lookups using interval trees:
//! - Symbol lookup by position
//! - Scope lookup by position (returns most specific scope)
//! - Usage lookup by position
//! - Destructure key lookup by position
//!
//! The interval trees are built from pending entries when `rebuild_range_index` is called.

use super::BindingGraph;
use crate::analysis::range_utils::{contains_position, position_to_point, range_size, range_to_interval};
use crate::types::{EnvReference, ScopeId, Symbol, SymbolId, SymbolUsage};
use intervaltree::IntervalTree;
use tower_lsp::lsp_types::Position;

impl BindingGraph {
    /// O(log n) symbol lookup by position using the interval tree.
    ///
    /// Must call `rebuild_range_index` after batch additions for this to work.
    pub fn symbol_at_position(&self, position: Position) -> Option<&Symbol> {
        let tree = self.symbol_range_tree.as_ref()?;
        let point = position_to_point(position);
        tree.query_point(point)
            .filter_map(|entry| self.get_symbol(entry.value))
            .next()
    }

    /// O(log n) destructure key lookup by position using the interval tree.
    ///
    /// Returns the symbol ID of the destructured property at the given position.
    pub fn symbol_at_destructure_key(&self, position: Position) -> Option<SymbolId> {
        let tree = self.destructure_range_tree.as_ref()?;
        let point = position_to_point(position);
        tree.query_point(point).map(|entry| entry.value).next()
    }

    /// O(log n) scope lookup by position using the range index.
    ///
    /// Returns the most specific (smallest) scope containing the position.
    /// Results are cached for repeated lookups at the same position.
    /// Must call `rebuild_range_index` after batch scope additions.
    pub fn scope_at_position(&self, position: Position) -> ScopeId {
        let key = (position.line, position.character);

        // Check cache first
        if let Some(&cached) = self.scope_cache.read().get(&key) {
            return cached;
        }

        // Compute result
        let result = self.scope_at_position_uncached(position);

        // Cache result
        self.scope_cache.write().insert(key, result);
        result
    }

    /// Uncached O(log n) scope lookup by position using the interval tree.
    ///
    /// Returns the most specific (smallest) scope containing the position.
    fn scope_at_position_uncached(&self, position: Position) -> ScopeId {
        let point = position_to_point(position);
        let mut best_scope = ScopeId::root();
        let mut best_size = u64::MAX;

        // Query all scopes containing this point and find the smallest
        if let Some(tree) = &self.scope_range_tree {
            for entry in tree.query_point(point) {
                let (scope_id, size) = entry.value;
                if size < best_size {
                    best_size = size;
                    best_scope = scope_id;
                }
            }
        }

        // Also check the root scope
        if let Some(root) = self.scopes.first() {
            if contains_position(root.range, position) {
                let root_size = range_size(root.range);
                if root_size < best_size {
                    best_scope = root.id;
                }
            }
        }

        best_scope
    }

    /// O(log n) usage lookup by position using the interval tree.
    ///
    /// Must call `rebuild_range_index` after batch additions for this to work.
    pub fn usage_at_position(&self, position: Position) -> Option<&SymbolUsage> {
        let tree = self.usage_range_tree.as_ref()?;
        let point = position_to_point(position);
        tree.query_point(point)
            .filter_map(|entry| self.usages.get(entry.value))
            .next()
    }

    /// Add a direct reference to an environment variable.
    pub fn add_direct_reference(&mut self, reference: EnvReference) {
        self.direct_references.push(reference);
    }

    /// Get all direct references.
    #[inline]
    pub fn direct_references(&self) -> &[EnvReference] {
        &self.direct_references
    }

    /// Clear all direct references.
    #[inline]
    pub fn clear_direct_references(&mut self) {
        self.direct_references.clear();
    }

    /// Get mutable access to direct references (test only).
    #[cfg(test)]
    #[inline]
    pub fn direct_references_mut(&mut self) -> &mut Vec<EnvReference> {
        &mut self.direct_references
    }

    /// Add a usage of a symbol.
    pub fn add_usage(&mut self, usage: SymbolUsage) {
        let usage_index = self.usages.len();
        // Add to pending entries - will be built into interval tree on rebuild
        self.pending_usage_entries.push(super::PendingRangeEntry {
            range: usage.range,
            value: usage_index,
        });
        self.usages.push(usage);
    }

    /// Get all usages.
    #[inline]
    pub fn usages(&self) -> &[SymbolUsage] {
        &self.usages
    }

    /// Clear all usages.
    #[inline]
    pub fn clear_usages(&mut self) {
        self.usages.clear();
    }

    /// Get mutable access to usages (test only).
    #[cfg(test)]
    #[inline]
    pub fn usages_mut(&mut self) -> &mut Vec<SymbolUsage> {
        &mut self.usages
    }

    /// Finalizes the binding graph after batch additions.
    ///
    /// Builds interval trees from pending entries for O(log n) position lookups.
    /// Also builds the env var index for fast env var lookups.
    pub fn rebuild_range_index(&mut self) {
        // Build destructure range interval tree
        if !self.pending_destructure_entries.is_empty() {
            self.destructure_range_tree = Some(IntervalTree::from_iter(
                self.pending_destructure_entries
                    .iter()
                    .map(|e| (range_to_interval(e.range), e.value)),
            ));
        }

        // Build symbol range interval tree
        if !self.pending_symbol_entries.is_empty() {
            self.symbol_range_tree = Some(IntervalTree::from_iter(
                self.pending_symbol_entries
                    .iter()
                    .map(|e| (range_to_interval(e.range), e.value)),
            ));
        }

        // Build usage range interval tree
        if !self.pending_usage_entries.is_empty() {
            self.usage_range_tree = Some(IntervalTree::from_iter(
                self.pending_usage_entries
                    .iter()
                    .map(|e| (range_to_interval(e.range), e.value)),
            ));
        }

        // Build scope range interval tree
        if !self.pending_scope_entries.is_empty() {
            self.scope_range_tree = Some(IntervalTree::from_iter(
                self.pending_scope_entries
                    .iter()
                    .map(|e| (range_to_interval(e.range), e.value)),
            ));
        }

        // Clear scope cache since data may have changed
        self.scope_cache.write().clear();

        // Build env var index (resolution cache is populated lazily as needed)
        self.build_env_var_index();
    }

    /// Builds just the scope range tree for O(log n) scope lookups.
    ///
    /// Call this after adding all scopes but before any scope_at_position lookups.
    pub fn rebuild_scope_range_index(&mut self) {
        if !self.pending_scope_entries.is_empty() {
            self.scope_range_tree = Some(IntervalTree::from_iter(
                self.pending_scope_entries
                    .iter()
                    .map(|e| (range_to_interval(e.range), e.value)),
            ));
        }
        // Clear scope cache since data may have changed
        self.scope_cache.write().clear();
    }
}
