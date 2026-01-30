//! Bulk operations and utilities for the binding graph.
//!
//! This module contains:
//! - `clear()` - reset the graph
//! - `remove_in_range()` - incremental analysis support
//! - `merge_from()` - incremental analysis merge
//! - Range utility wrappers
//! - Statistics and diagnostics

use super::BindingGraph;
use crate::analysis::range_utils::{
    contains_position, expand_range, range_contains_range, range_size, ranges_overlap,
};
use crate::types::{Scope, ScopeId, ScopeKind, SymbolId, SymbolOrigin};
use rustc_hash::{FxHashMap, FxHashSet};
use tower_lsp::lsp_types::{Position, Range};

/// Statistics about the binding graph contents.
#[derive(Debug, Clone, Copy)]
pub struct BindingGraphStats {
    /// Number of symbols in the graph
    pub symbol_count: usize,
    /// Number of scopes in the graph
    pub scope_count: usize,
    /// Number of usages in the graph
    pub usage_count: usize,
    /// Number of direct references in the graph
    pub direct_reference_count: usize,
}

impl BindingGraph {
    /// Check if a position is contained within a range.
    #[inline]
    pub fn contains_position(range: Range, pos: Position) -> bool {
        contains_position(range, pos)
    }

    /// Check if an inner range is fully contained within an outer range.
    #[inline]
    pub fn is_range_contained(inner: Range, outer: Range) -> bool {
        range_contains_range(outer, inner)
    }

    /// Calculate a size metric for a range.
    #[inline]
    pub(crate) fn range_size(range: Range) -> u64 {
        range_size(range)
    }

    /// Check if two ranges overlap.
    #[inline]
    pub fn ranges_overlap(a: Range, b: Range) -> bool {
        ranges_overlap(a, b)
    }

    /// Remove all symbols, usages, and direct references that overlap with the given range.
    ///
    /// This is used for incremental analysis to clear affected regions before re-analysis.
    /// Returns the number of items removed.
    pub fn remove_in_range(&mut self, range: Range) -> usize {
        let mut removed = 0;

        // Track which symbol IDs are being removed for cleanup
        let removed_symbol_ids: FxHashSet<_> = self
            .symbols
            .iter()
            .filter(|s| Self::ranges_overlap(s.declaration_range, range))
            .map(|s| s.id)
            .collect();

        // Remove symbols that overlap with the range
        let original_symbol_count = self.symbols.len();
        self.symbols
            .retain(|s| !Self::ranges_overlap(s.declaration_range, range));
        removed += original_symbol_count - self.symbols.len();

        // Clean up name indices for removed symbols
        self.name_index.retain(|_, ids| {
            ids.retain(|id| !removed_symbol_ids.contains(id));
            !ids.is_empty()
        });
        self.name_only_index.retain(|_, ids| {
            ids.retain(|id| !removed_symbol_ids.contains(id));
            !ids.is_empty()
        });

        // Remove usages that overlap with the range or reference removed symbols
        let original_usage_count = self.usages.len();
        self.usages.retain(|u| {
            !Self::ranges_overlap(u.range, range) && !removed_symbol_ids.contains(&u.symbol_id)
        });
        removed += original_usage_count - self.usages.len();

        // Remove direct references that overlap with the range
        let original_ref_count = self.direct_references.len();
        self.direct_references
            .retain(|r| !Self::ranges_overlap(r.full_range, range));
        removed += original_ref_count - self.direct_references.len();

        // Clear pending entries that overlap with the range
        self.pending_symbol_entries
            .retain(|e| !Self::ranges_overlap(e.range, range));
        self.pending_usage_entries
            .retain(|e| !Self::ranges_overlap(e.range, range));
        self.pending_destructure_entries
            .retain(|e| !Self::ranges_overlap(e.range, range));

        // Note: We don't remove scopes since they may contain items outside the edit range.
        // Scopes will be rebuilt in the next full analysis if needed.

        // Clear caches since they may reference removed items
        self.resolution_cache.clear();
        self.scope_cache.write().clear();

        // Interval trees will be rebuilt on next rebuild_range_index()
        self.destructure_range_tree = None;
        self.symbol_range_tree = None;
        self.usage_range_tree = None;

        removed
    }

    /// Get all scope IDs whose ranges overlap with the given range.
    pub fn scopes_overlapping(&self, range: Range) -> Vec<ScopeId> {
        self.scopes
            .iter()
            .filter(|s| Self::ranges_overlap(s.range, range))
            .map(|s| s.id)
            .collect()
    }

    /// Get all symbols whose declaration range overlaps with the given range.
    pub fn symbols_in_range(&self, range: Range) -> Vec<&crate::types::Symbol> {
        self.symbols
            .iter()
            .filter(|s| Self::ranges_overlap(s.declaration_range, range))
            .collect()
    }

    /// Get all usages whose range overlaps with the given range.
    pub fn usages_in_range(&self, range: Range) -> Vec<&crate::types::SymbolUsage> {
        self.usages
            .iter()
            .filter(|u| Self::ranges_overlap(u.range, range))
            .collect()
    }

    /// Get all direct references whose range overlaps with the given range.
    pub fn references_in_range(&self, range: Range) -> Vec<&crate::types::EnvReference> {
        self.direct_references
            .iter()
            .filter(|r| Self::ranges_overlap(r.full_range, range))
            .collect()
    }

    /// Estimate the size of the document based on root scope range.
    ///
    /// Returns (line_count, approximate_char_count).
    pub fn document_size(&self) -> (u32, u64) {
        if let Some(root) = self.scopes.first() {
            let lines = root.range.end.line.saturating_sub(root.range.start.line) + 1;
            let chars = Self::range_size(root.range);
            (lines, chars)
        } else {
            (0, 0)
        }
    }

    /// Check if an edit range is "large" relative to the document size.
    ///
    /// Returns true if the edit covers more than 50% of the document.
    pub fn is_large_edit(&self, edit_range: Range) -> bool {
        let (doc_lines, _) = self.document_size();
        if doc_lines == 0 {
            return true; // Treat empty document as needing full analysis
        }
        let edit_lines = edit_range.end.line.saturating_sub(edit_range.start.line) + 1;
        edit_lines > doc_lines / 2
    }

    /// Clear all data from the graph and reset to initial state.
    ///
    /// Preserves the root scope but resets everything else.
    pub fn clear(&mut self) {
        self.symbols.clear();
        self.scopes.clear();
        self.name_index.clear();
        self.name_only_index.clear();
        self.direct_references.clear();
        self.usages.clear();
        self.pending_destructure_entries.clear();
        self.pending_symbol_entries.clear();
        self.pending_usage_entries.clear();
        self.pending_scope_entries.clear();
        self.destructure_range_tree = None;
        self.symbol_range_tree = None;
        self.usage_range_tree = None;
        self.scope_range_tree = None;
        self.env_var_index.clear();
        self.resolution_cache.clear();
        self.scope_cache.write().clear();
        self.next_symbol_id = 0;
        self.next_scope_id = 1;

        // Re-add root scope
        self.scopes.push(Scope {
            id: ScopeId::root(),
            parent: None,
            range: Range::default(),
            kind: ScopeKind::Module,
        });
        self.next_scope_id = 2;
    }

    /// Get statistics about the graph contents.
    pub fn stats(&self) -> BindingGraphStats {
        BindingGraphStats {
            symbol_count: self.symbols.len(),
            scope_count: self.scopes.len(),
            usage_count: self.usages.len(),
            direct_reference_count: self.direct_references.len(),
        }
    }

    /// Merge items from another graph that fall within the given range.
    ///
    /// This is the core of incremental analysis. Items from the `other` graph
    /// that overlap with an expanded version of `edit_range` are copied to
    /// this graph with remapped IDs to avoid conflicts.
    ///
    /// # Arguments
    /// * `other` - The graph containing new analysis results
    /// * `edit_range` - The original edit range (will be expanded by 5 lines)
    ///
    /// # Returns
    /// Statistics about what was merged.
    pub fn merge_from(&mut self, other: &BindingGraph, edit_range: Range) -> MergeStats {
        let expanded = expand_range(edit_range, 5); // 5 lines buffer
        let mut stats = MergeStats::default();

        // Build ID mapping for symbols (old ID in other -> new ID in self)
        let mut id_map: FxHashMap<SymbolId, SymbolId> = FxHashMap::default();

        // Merge symbols in range with new IDs
        for symbol in other.symbols_in_range(expanded) {
            let old_id = symbol.id;
            let new_id = self.allocate_symbol_id();
            id_map.insert(old_id, new_id);

            let mut new_symbol = symbol.clone();
            new_symbol.id = new_id;

            // Remap origin if it references another symbol
            new_symbol.origin = match &symbol.origin {
                SymbolOrigin::Symbol { target } => {
                    if let Some(&new_target) = id_map.get(target) {
                        SymbolOrigin::Symbol { target: new_target }
                    } else {
                        // Target symbol not in our merge set - keep original reference
                        // This can happen if the target is outside the expanded range
                        symbol.origin.clone()
                    }
                }
                SymbolOrigin::DestructuredProperty { source, key } => {
                    if let Some(&new_source) = id_map.get(source) {
                        SymbolOrigin::DestructuredProperty {
                            source: new_source,
                            key: key.clone(),
                        }
                    } else {
                        symbol.origin.clone()
                    }
                }
                other_origin => other_origin.clone(),
            };

            self.add_symbol_with_id(new_symbol);
            stats.symbols_merged += 1;
        }

        // Merge usages in range, remapping symbol references
        for usage in other.usages_in_range(expanded) {
            let mut new_usage = usage.clone();
            if let Some(&new_id) = id_map.get(&usage.symbol_id) {
                new_usage.symbol_id = new_id;
            }
            // If the symbol_id wasn't remapped, it references a symbol outside
            // our merge range - keep the original reference
            self.add_usage(new_usage);
            stats.usages_merged += 1;
        }

        // Merge direct references in range
        for reference in other.references_in_range(expanded) {
            self.add_direct_reference(reference.clone());
            stats.references_merged += 1;
        }

        stats
    }
}

/// Statistics about a merge operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct MergeStats {
    /// Number of symbols merged
    pub symbols_merged: usize,
    /// Number of usages merged
    pub usages_merged: usize,
    /// Number of direct references merged
    pub references_merged: usize,
}

impl MergeStats {
    /// Total number of items merged.
    pub fn total(&self) -> usize {
        self.symbols_merged + self.usages_merged + self.references_merged
    }
}
