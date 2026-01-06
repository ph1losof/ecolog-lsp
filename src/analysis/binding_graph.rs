//! Binding Graph - Arena-based sparse graph for tracking env var bindings.
//!
//! This module provides the core data structure for tracking environment variable
//! references across binding chains like:
//! - `const a = process.env.DB_URL; const b = a;`
//! - `const env = process.env; const { DB_URL } = env;`

use crate::types::{
    EnvReference, ResolvedEnv, Scope, ScopeId, ScopeKind, Symbol, SymbolId, SymbolOrigin,
    SymbolUsage,
};
use compact_str::CompactString;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use tower_lsp::lsp_types::{Position, Range};

/// Maximum depth for chain resolution to prevent infinite loops.
pub const MAX_CHAIN_DEPTH: usize = 10;

/// Multiplier for line count when calculating range size.
/// Used to ensure line differences are weighted more heavily than character differences.
const RANGE_SIZE_LINE_WEIGHT: u64 = 10000;

/// Index entry for range-based lookups.
#[derive(Debug, Clone, Copy)]
struct RangeIndexEntry {
    range: Range,
    symbol_id: SymbolId,
}

/// The binding graph for a single document.
/// Uses sparse representation - only env-related symbols are tracked.
#[derive(Debug, Default)]
pub struct BindingGraph {
    /// Arena storage for symbols.
    symbols: Vec<Symbol>,

    /// Arena storage for scopes.
    scopes: Vec<Scope>,

    /// Symbol lookup by (name, scope_id).
    /// Multiple symbols can have the same name in the same scope due to shadowing.
    name_index: FxHashMap<(CompactString, ScopeId), SmallVec<[SymbolId; 2]>>,

    /// Direct env references (not through bindings).
    /// Kept for backward compatibility and for references that don't create bindings.
    direct_references: Vec<EnvReference>,

    /// Symbol usages.
    usages: Vec<SymbolUsage>,

    /// Sorted index of destructured key ranges for fast position lookups.
    /// Enables O(log n) binary search instead of O(n) iteration.
    destructure_range_index: Vec<RangeIndexEntry>,

    /// Counter for generating symbol IDs.
    next_symbol_id: u32,

    /// Counter for generating scope IDs.
    next_scope_id: u32,
}

impl BindingGraph {
    /// Create a new empty binding graph with the root scope.
    pub fn new() -> Self {
        let mut graph = Self {
            symbols: Vec::new(),
            scopes: Vec::new(),
            name_index: FxHashMap::default(),
            direct_references: Vec::new(),
            usages: Vec::new(),
            destructure_range_index: Vec::new(),
            next_symbol_id: 0,
            next_scope_id: 1, // Start at 1 since we pre-create root
        };

        // Create the root/module scope
        graph.scopes.push(Scope {
            id: ScopeId::root(),
            parent: None,
            range: Range::default(), // Will be set to document range
            kind: ScopeKind::Module,
        });
        graph.next_scope_id = 2;

        graph
    }

    /// Set the range of the root scope (typically the entire document).
    pub fn set_root_range(&mut self, range: Range) {
        if let Some(root) = self.scopes.first_mut() {
            root.range = range;
        }
    }

    // =========================================================================
    // Symbol Operations
    // =========================================================================

    /// Add a symbol to the graph.
    /// Returns the SymbolId assigned to the symbol.
    pub fn add_symbol(&mut self, mut symbol: Symbol) -> SymbolId {
        // Increment first to get 1-based ID (0 is reserved)
        self.next_symbol_id += 1;
        let id = SymbolId::new(self.next_symbol_id)
            .expect("Symbol ID counter overflow - too many symbols");
        symbol.id = id;
        // Symbol at index (id - 1) for 0-based vector access

        // Index by (name, scope) for lookups
        let key = (symbol.name.clone(), symbol.scope);
        self.name_index
            .entry(key)
            .or_insert_with(SmallVec::new)
            .push(id);

        // Index destructured key range if present
        if let Some(key_range) = symbol.destructured_key_range {
            self.destructure_range_index.push(RangeIndexEntry {
                range: key_range,
                symbol_id: id,
            });
        }

        self.symbols.push(symbol);
        id
    }

    /// Get a symbol by ID.
    #[inline]
    pub fn get_symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(id.index())
    }

    /// Get a mutable reference to a symbol by ID.
    #[inline]
    pub fn get_symbol_mut(&mut self, id: SymbolId) -> Option<&mut Symbol> {
        self.symbols.get_mut(id.index())
    }

    /// Get all symbols.
    #[inline]
    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    /// Get mutable access to all symbols.
    #[inline]
    pub fn symbols_mut(&mut self) -> &mut [Symbol] {
        &mut self.symbols
    }

    /// Find a symbol by name in the given scope (walks up scope chain).
    /// Returns the most recently declared valid symbol with that name.
    pub fn lookup_symbol(&self, name: &str, scope: ScopeId) -> Option<&Symbol> {
        let mut current_scope = Some(scope);

        while let Some(scope_id) = current_scope {
            let key = (CompactString::from(name), scope_id);

            if let Some(symbol_ids) = self.name_index.get(&key) {
                // Return the last (most recent) valid symbol
                for &id in symbol_ids.iter().rev() {
                    if let Some(symbol) = self.get_symbol(id) {
                        if symbol.is_valid {
                            return Some(symbol);
                        }
                    }
                }
            }

            // Walk up to parent scope
            current_scope = self.get_scope(scope_id).and_then(|s| s.parent);
        }

        None
    }

    /// Find symbol ID by name in the given scope.
    pub fn lookup_symbol_id(&self, name: &str, scope: ScopeId) -> Option<SymbolId> {
        self.lookup_symbol(name, scope).map(|s| s.id)
    }

    /// Find symbol at a specific position (checks declaration ranges).
    pub fn symbol_at_position(&self, position: Position) -> Option<&Symbol> {
        for symbol in &self.symbols {
            if Self::contains_position(symbol.name_range, position) {
                return Some(symbol);
            }
        }
        None
    }

    /// Rebuild the destructure range index (sort by range start position).
    /// This should be called after all symbols have been added to enable efficient lookups.
    pub fn rebuild_range_index(&mut self) {
        self.destructure_range_index.sort_by(|a, b| {
            a.range
                .start
                .line
                .cmp(&b.range.start.line)
                .then_with(|| a.range.start.character.cmp(&b.range.start.character))
        });
    }

    /// Binary search for symbol at position in destructured keys.
    /// Returns SymbolId if found. Requires the index to be sorted (call rebuild_range_index first).
    pub fn symbol_at_destructure_key(&self, position: Position) -> Option<SymbolId> {
        // Binary search for entries that might contain the position
        // We search for the rightmost entry whose start is <= position
        let mut left = 0;
        let mut right = self.destructure_range_index.len();
        let mut found_idx = None;

        while left < right {
            let mid = left + (right - left) / 2;
            let entry = &self.destructure_range_index[mid];

            if Self::contains_position(entry.range, position) {
                return Some(entry.symbol_id);
            }

            // Check if position is before this entry
            if position.line < entry.range.start.line
                || (position.line == entry.range.start.line
                    && position.character < entry.range.start.character)
            {
                right = mid;
            } else {
                left = mid + 1;
                found_idx = Some(mid);
            }
        }

        // Linear search nearby entries (in case of overlapping ranges)
        if let Some(idx) = found_idx {
            // Check a few entries before and after
            for offset in 0..3 {
                if let Some(i) = idx.checked_sub(offset) {
                    if i < self.destructure_range_index.len() {
                        let entry = &self.destructure_range_index[i];
                        if Self::contains_position(entry.range, position) {
                            return Some(entry.symbol_id);
                        }
                    }
                }
                let i = idx + offset + 1;
                if i < self.destructure_range_index.len() {
                    let entry = &self.destructure_range_index[i];
                    if Self::contains_position(entry.range, position) {
                        return Some(entry.symbol_id);
                    }
                }
            }
        }

        None
    }

    // =========================================================================
    // Scope Operations
    // =========================================================================

    /// Add a scope to the graph.
    /// Returns the ScopeId assigned to the scope.
    pub fn add_scope(&mut self, mut scope: Scope) -> ScopeId {
        // Assign ID first (matches vector index + 1), then increment for next scope
        let id =
            ScopeId::new(self.next_scope_id).expect("Scope ID counter overflow - too many scopes");
        self.next_scope_id += 1;
        scope.id = id;
        // Scope with ID n is at scopes[n-1] (0-based vector)
        self.scopes.push(scope);
        id
    }

    /// Get a scope by ID.
    #[inline]
    pub fn get_scope(&self, id: ScopeId) -> Option<&Scope> {
        self.scopes.get(id.index())
    }

    /// Get all scopes.
    #[inline]
    pub fn scopes(&self) -> &[Scope] {
        &self.scopes
    }

    /// Find the innermost scope containing a position.
    pub fn scope_at_position(&self, position: Position) -> ScopeId {
        let mut best_scope = ScopeId::root();
        let mut best_size = u64::MAX;

        for scope in &self.scopes {
            if Self::contains_position(scope.range, position) {
                let size = Self::range_size(scope.range);
                if size < best_size {
                    best_size = size;
                    best_scope = scope.id;
                }
            }
        }

        best_scope
    }

    // =========================================================================
    // Direct References
    // =========================================================================

    /// Add a direct env reference.
    pub fn add_direct_reference(&mut self, reference: EnvReference) {
        self.direct_references.push(reference);
    }

    /// Get all direct references.
    #[inline]
    pub fn direct_references(&self) -> &[EnvReference] {
        &self.direct_references
    }

    /// Get mutable access to direct references.
    #[inline]
    pub fn direct_references_mut(&mut self) -> &mut Vec<EnvReference> {
        &mut self.direct_references
    }

    // =========================================================================
    // Usages
    // =========================================================================

    /// Add a symbol usage.
    pub fn add_usage(&mut self, usage: SymbolUsage) {
        self.usages.push(usage);
    }

    /// Get all usages.
    #[inline]
    pub fn usages(&self) -> &[SymbolUsage] {
        &self.usages
    }

    /// Get mutable access to usages.
    #[inline]
    pub fn usages_mut(&mut self) -> &mut Vec<SymbolUsage> {
        &mut self.usages
    }

    /// Find usage at a specific position.
    pub fn usage_at_position(&self, position: Position) -> Option<&SymbolUsage> {
        for usage in &self.usages {
            if Self::contains_position(usage.range, position) {
                return Some(usage);
            }
        }
        None
    }

    // =========================================================================
    // Resolution
    // =========================================================================

    /// Resolve a symbol to its terminal environment variable (if any).
    /// Follows the chain of Symbol/DestructuredProperty origins up to MAX_CHAIN_DEPTH.
    pub fn resolve_to_env(&self, symbol_id: SymbolId) -> Option<ResolvedEnv> {
        self.resolve_to_env_with_depth(symbol_id, MAX_CHAIN_DEPTH, 0)
    }

    /// Resolve with custom max depth.
    pub fn resolve_to_env_with_max(
        &self,
        symbol_id: SymbolId,
        max_depth: usize,
    ) -> Option<ResolvedEnv> {
        self.resolve_to_env_with_depth(symbol_id, max_depth, 0)
    }

    fn resolve_to_env_with_depth(
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
                // Follow the chain
                self.resolve_to_env_with_depth(*target, max_depth, current_depth + 1)
            }

            SymbolOrigin::DestructuredProperty { source, key } => {
                // First resolve the source
                match self.resolve_to_env_with_depth(*source, max_depth, current_depth + 1)? {
                    ResolvedEnv::Object(_) => {
                        // Source is an env object, so this property is an env var
                        Some(ResolvedEnv::Variable(key.clone()))
                    }
                    ResolvedEnv::Variable(_) => {
                        // Source is a specific var, can't destructure further
                        None
                    }
                }
            }

            SymbolOrigin::Unknown
            | SymbolOrigin::Unresolvable
            | SymbolOrigin::UnresolvedSymbol { .. }
            | SymbolOrigin::UnresolvedDestructure { .. } => None,
        }
    }

    /// Check if a symbol resolves to an env object (not a specific var).
    pub fn resolves_to_env_object(&self, symbol_id: SymbolId) -> bool {
        matches!(self.resolve_to_env(symbol_id), Some(ResolvedEnv::Object(_)))
    }

    /// Get the env var name if the symbol resolves to a specific variable.
    pub fn get_env_var_name(&self, symbol_id: SymbolId) -> Option<CompactString> {
        match self.resolve_to_env(symbol_id)? {
            ResolvedEnv::Variable(name) => Some(name),
            ResolvedEnv::Object(_) => None,
        }
    }

    // =========================================================================
    // Utilities
    // =========================================================================

    /// Check if a range contains a position.
    #[inline]
    pub fn contains_position(range: Range, pos: Position) -> bool {
        if pos.line < range.start.line || pos.line > range.end.line {
            return false;
        }
        if pos.line == range.start.line && pos.character < range.start.character {
            return false;
        }
        if pos.line == range.end.line && pos.character > range.end.character {
            return false;
        }
        true
    }

    /// Check if inner range is contained within outer range.
    #[inline]
    pub fn is_range_contained(inner: Range, outer: Range) -> bool {
        // Start of inner must be >= start of outer
        if inner.start.line < outer.start.line {
            return false;
        }
        if inner.start.line == outer.start.line && inner.start.character < outer.start.character {
            return false;
        }

        // End of inner must be <= end of outer
        if inner.end.line > outer.end.line {
            return false;
        }
        if inner.end.line == outer.end.line && inner.end.character > outer.end.character {
            return false;
        }

        true
    }

    /// Calculate the "size" of a range for comparison (smaller = more specific).
    #[inline]
    fn range_size(range: Range) -> u64 {
        let lines = (range.end.line - range.start.line) as u64;
        let chars = if range.end.line == range.start.line {
            (range.end.character - range.start.character) as u64
        } else {
            range.end.character as u64
        };
        lines * RANGE_SIZE_LINE_WEIGHT + chars
    }

    /// Clear all data (useful for re-analysis).
    pub fn clear(&mut self) {
        self.symbols.clear();
        self.scopes.clear();
        self.name_index.clear();
        self.direct_references.clear();
        self.usages.clear();
        self.destructure_range_index.clear();
        self.next_symbol_id = 0;
        self.next_scope_id = 1;

        // Re-create root scope
        self.scopes.push(Scope {
            id: ScopeId::root(),
            parent: None,
            range: Range::default(),
            kind: ScopeKind::Module,
        });
        self.next_scope_id = 2;
    }

    /// Get statistics about the graph.
    pub fn stats(&self) -> BindingGraphStats {
        BindingGraphStats {
            symbol_count: self.symbols.len(),
            scope_count: self.scopes.len(),
            usage_count: self.usages.len(),
            direct_reference_count: self.direct_references.len(),
        }
    }
}

/// Statistics about a binding graph.
#[derive(Debug, Clone, Copy)]
pub struct BindingGraphStats {
    pub symbol_count: usize,
    pub scope_count: usize,
    pub usage_count: usize,
    pub direct_reference_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SymbolKind;

    fn make_range(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Range {
        Range::new(
            Position::new(start_line, start_char),
            Position::new(end_line, end_char),
        )
    }

    #[test]
    fn test_new_graph_has_root_scope() {
        let graph = BindingGraph::new();
        assert_eq!(graph.scopes().len(), 1);
        assert_eq!(graph.scopes()[0].id, ScopeId::root());
        assert!(graph.scopes()[0].parent.is_none());
    }

    #[test]
    fn test_add_and_lookup_symbol() {
        let mut graph = BindingGraph::new();

        let symbol = Symbol {
            id: SymbolId::new(1).unwrap(), // Will be overwritten
            name: "dbUrl".into(),
            declaration_range: make_range(0, 0, 0, 30),
            name_range: make_range(0, 6, 0, 11),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "DATABASE_URL".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        };

        let id = graph.add_symbol(symbol);

        // Lookup by ID
        let found = graph.get_symbol(id).unwrap();
        assert_eq!(found.name, "dbUrl");

        // Lookup by name
        let found = graph.lookup_symbol("dbUrl", ScopeId::root()).unwrap();
        assert_eq!(found.id, id);
    }

    #[test]
    fn test_scope_chain_lookup() {
        let mut graph = BindingGraph::new();

        // Add a function scope (ID will be assigned by add_scope)
        let func_scope_id = graph.add_scope(Scope {
            id: ScopeId::root(), // Placeholder, will be overwritten
            parent: Some(ScopeId::root()),
            range: make_range(5, 0, 10, 1),
            kind: ScopeKind::Function,
        });

        // Add symbol in root scope
        let root_symbol = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(), // Placeholder, will be overwritten
            name: "globalEnv".into(),
            declaration_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 6, 0, 15),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvObject {
                canonical_name: "process.env".into(),
            },
            kind: SymbolKind::EnvObject,
            is_valid: true,
            destructured_key_range: None,
        });

        // Lookup from function scope should find root symbol (walks up scope chain)
        let found = graph.lookup_symbol("globalEnv", func_scope_id).unwrap();
        assert_eq!(found.id, root_symbol);
    }

    #[test]
    fn test_resolve_env_chain() {
        let mut graph = BindingGraph::new();

        // const env = process.env
        let env_id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "env".into(),
            declaration_range: make_range(0, 0, 0, 25),
            name_range: make_range(0, 6, 0, 9),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvObject {
                canonical_name: "process.env".into(),
            },
            kind: SymbolKind::EnvObject,
            is_valid: true,
            destructured_key_range: None,
        });

        // const config = env
        let config_id = graph.add_symbol(Symbol {
            id: SymbolId::new(2).unwrap(),
            name: "config".into(),
            declaration_range: make_range(1, 0, 1, 20),
            name_range: make_range(1, 6, 1, 12),
            scope: ScopeId::root(),
            origin: SymbolOrigin::Symbol { target: env_id },
            kind: SymbolKind::Variable,
            is_valid: true,
            destructured_key_range: None,
        });

        // const { DB_URL } = config
        let db_url_id = graph.add_symbol(Symbol {
            id: SymbolId::new(3).unwrap(),
            name: "DB_URL".into(),
            declaration_range: make_range(2, 0, 2, 30),
            name_range: make_range(2, 8, 2, 14),
            scope: ScopeId::root(),
            origin: SymbolOrigin::DestructuredProperty {
                source: config_id,
                key: "DB_URL".into(),
            },
            kind: SymbolKind::DestructuredProperty,
            is_valid: true,
            destructured_key_range: Some(make_range(2, 8, 2, 14)),
        });

        // Resolution tests
        assert_eq!(
            graph.resolve_to_env(env_id),
            Some(ResolvedEnv::Object("process.env".into()))
        );
        assert_eq!(
            graph.resolve_to_env(config_id),
            Some(ResolvedEnv::Object("process.env".into()))
        );
        assert_eq!(
            graph.resolve_to_env(db_url_id),
            Some(ResolvedEnv::Variable("DB_URL".into()))
        );
    }

    #[test]
    fn test_scope_at_position() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(make_range(0, 0, 20, 0));

        // Add function scope
        let func_scope_id = graph.add_scope(Scope {
            id: ScopeId::root(), // Placeholder, will be overwritten
            parent: Some(ScopeId::root()),
            range: make_range(5, 0, 10, 1),
            kind: ScopeKind::Function,
        });

        // Position in root (outside function)
        let scope = graph.scope_at_position(Position::new(2, 5));
        assert_eq!(scope, ScopeId::root());

        // Position in function (should return the function scope)
        let scope = graph.scope_at_position(Position::new(7, 5));
        assert_eq!(scope, func_scope_id);
    }

    #[test]
    fn test_contains_position() {
        let range = make_range(5, 10, 5, 20);

        assert!(BindingGraph::contains_position(range, Position::new(5, 10)));
        assert!(BindingGraph::contains_position(range, Position::new(5, 15)));
        assert!(BindingGraph::contains_position(range, Position::new(5, 20)));

        assert!(!BindingGraph::contains_position(range, Position::new(5, 9)));
        assert!(!BindingGraph::contains_position(
            range,
            Position::new(5, 21)
        ));
        assert!(!BindingGraph::contains_position(
            range,
            Position::new(4, 15)
        ));
        assert!(!BindingGraph::contains_position(
            range,
            Position::new(6, 15)
        ));
    }
}
