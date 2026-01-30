//! Core symbol and scope CRUD operations for the binding graph.
//!
//! This module contains:
//! - Graph construction (`new`)
//! - Symbol operations (add, get, update, invalidate)
//! - Scope operations (add, get, set root range)
//! - Name-based symbol lookups

use super::{BindingGraph, PendingRangeEntry};
use crate::analysis::range_utils::range_size;
use crate::types::{Scope, ScopeId, ScopeKind, Symbol, SymbolId, SymbolOrigin};
use compact_str::CompactString;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use tower_lsp::lsp_types::Range;

impl BindingGraph {
    /// Create a new empty binding graph.
    ///
    /// The graph is initialized with a root scope (ScopeId = 1) that represents
    /// the module/file level scope.
    pub fn new() -> Self {
        let mut graph = Self {
            symbols: Vec::new(),
            scopes: Vec::new(),
            name_index: FxHashMap::default(),
            name_only_index: FxHashMap::default(),
            direct_references: Vec::new(),
            usages: Vec::new(),
            pending_destructure_entries: Vec::new(),
            pending_symbol_entries: Vec::new(),
            pending_usage_entries: Vec::new(),
            pending_scope_entries: Vec::new(),
            destructure_range_tree: None,
            symbol_range_tree: None,
            usage_range_tree: None,
            scope_range_tree: None,
            env_var_index: FxHashMap::default(),
            resolution_cache: FxHashMap::default(),
            scope_cache: RwLock::new(FxHashMap::default()),
            next_symbol_id: 0,
            next_scope_id: 1,
        };

        // Add root scope
        graph.scopes.push(Scope {
            id: ScopeId::root(),
            parent: None,
            range: Range::default(),
            kind: ScopeKind::Module,
        });
        graph.next_scope_id = 2;

        graph
    }

    /// Set the range of the root scope.
    ///
    /// This should be called after parsing to set the range to cover
    /// the entire document.
    pub fn set_root_range(&mut self, range: Range) {
        if let Some(root) = self.scopes.first_mut() {
            root.range = range;
        }
    }

    /// Add a symbol to the graph.
    ///
    /// The symbol is assigned a new ID and added to the name indices.
    /// Returns the assigned symbol ID.
    pub fn add_symbol(&mut self, mut symbol: Symbol) -> SymbolId {
        self.next_symbol_id += 1;
        let id = SymbolId::new(self.next_symbol_id)
            .expect("Symbol ID counter overflow - too many symbols");
        symbol.id = id;

        let key = (symbol.name.clone(), symbol.scope);
        self.name_index.entry(key).or_default().push(id);

        // Also add to name-only index for fast name lookups across all scopes
        self.name_only_index
            .entry(symbol.name.clone())
            .or_default()
            .push(id);

        if let Some(key_range) = symbol.destructured_key_range {
            // Add to pending entries - will be built into interval tree on rebuild
            self.pending_destructure_entries.push(PendingRangeEntry {
                range: key_range,
                value: id,
            });
        }

        // Add to pending symbol entries - will be built into interval tree on rebuild
        self.pending_symbol_entries.push(PendingRangeEntry {
            range: symbol.name_range,
            value: id,
        });

        self.symbols.push(symbol);
        id
    }

    /// Get a symbol by ID.
    #[inline]
    pub fn get_symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(id.index())
    }

    /// Get a mutable reference to a symbol by ID (test only).
    #[cfg(test)]
    #[inline]
    pub fn get_symbol_mut(&mut self, id: SymbolId) -> Option<&mut Symbol> {
        self.symbols.get_mut(id.index())
    }

    /// Update the origin of a symbol.
    ///
    /// This is an intent-based method for modifying symbol origin during analysis.
    #[inline]
    pub fn update_symbol_origin(&mut self, id: SymbolId, origin: SymbolOrigin) {
        if let Some(symbol) = self.symbols.get_mut(id.index()) {
            symbol.origin = origin;
        }
    }

    /// Mark a symbol as invalid.
    ///
    /// Invalid symbols have been shadowed or reassigned and should not
    /// be considered when resolving bindings.
    #[inline]
    pub fn invalidate_symbol(&mut self, id: SymbolId) {
        if let Some(symbol) = self.symbols.get_mut(id.index()) {
            symbol.is_valid = false;
        }
    }

    /// Get a slice of all symbols.
    #[inline]
    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    /// Mark all symbols as invalid.
    ///
    /// This is useful when clearing state or preparing for a full re-analysis.
    #[inline]
    pub fn invalidate_all_symbols(&mut self) {
        for symbol in &mut self.symbols {
            symbol.is_valid = false;
        }
    }

    /// Get a mutable slice of all symbols (test only).
    #[cfg(test)]
    #[inline]
    pub fn symbols_mut(&mut self) -> &mut [Symbol] {
        &mut self.symbols
    }

    /// Look up a symbol by name in the given scope or its ancestors.
    ///
    /// Returns the most recent valid symbol with the given name that is
    /// visible from the specified scope.
    pub fn lookup_symbol(&self, name: &str, scope: ScopeId) -> Option<&Symbol> {
        let mut current_scope = Some(scope);

        while let Some(scope_id) = current_scope {
            let key = (CompactString::from(name), scope_id);

            if let Some(symbol_ids) = self.name_index.get(&key) {
                for &id in symbol_ids.iter().rev() {
                    if let Some(symbol) = self.get_symbol(id) {
                        if symbol.is_valid {
                            return Some(symbol);
                        }
                    }
                }
            }

            current_scope = self.get_scope(scope_id).and_then(|s| s.parent);
        }

        None
    }

    /// Look up a symbol ID by name in the given scope or its ancestors.
    pub fn lookup_symbol_id(&self, name: &str, scope: ScopeId) -> Option<SymbolId> {
        self.lookup_symbol(name, scope).map(|s| s.id)
    }

    /// Look up all symbols with the given name across all scopes.
    ///
    /// Returns an iterator over symbol IDs. O(1) lookup.
    pub fn lookup_symbols_by_name(&self, name: &str) -> impl Iterator<Item = SymbolId> + '_ {
        self.name_only_index
            .get(name)
            .map(|ids| ids.iter().copied())
            .into_iter()
            .flatten()
    }

    /// Add a scope to the graph.
    ///
    /// The scope is assigned a new ID and added to the pending scope entries.
    /// Returns the assigned scope ID.
    pub fn add_scope(&mut self, mut scope: Scope) -> ScopeId {
        let id =
            ScopeId::new(self.next_scope_id).expect("Scope ID counter overflow - too many scopes");
        self.next_scope_id += 1;
        scope.id = id;

        // Add to pending entries - will be built into interval tree on rebuild
        let size = range_size(scope.range);
        self.pending_scope_entries.push(PendingRangeEntry {
            range: scope.range,
            value: (id, size),
        });

        // Clear scope cache since new scope was added
        self.scope_cache.write().clear();

        self.scopes.push(scope);
        id
    }

    /// Get a scope by ID.
    #[inline]
    pub fn get_scope(&self, id: ScopeId) -> Option<&Scope> {
        self.scopes.get(id.index())
    }

    /// Get a slice of all scopes.
    #[inline]
    pub fn scopes(&self) -> &[Scope] {
        &self.scopes
    }
}
