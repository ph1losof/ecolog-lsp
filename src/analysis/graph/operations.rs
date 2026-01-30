//! Bulk operations and utilities for the binding graph.
//!
//! This module contains:
//! - `clear()` - reset the graph
//! - `remove_in_range()` - incremental analysis support
//! - Range utility wrappers
//! - Statistics and diagnostics

use super::BindingGraph;
use crate::analysis::range_utils::{contains_position, range_contains_range, range_size, ranges_overlap};
use crate::types::{Scope, ScopeId, ScopeKind};
use rustc_hash::FxHashSet;
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
}
