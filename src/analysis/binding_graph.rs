use crate::types::{
    EnvReference, ResolvedEnv, Scope, ScopeId, ScopeKind, Symbol, SymbolId, SymbolOrigin,
    SymbolUsage,
};
use compact_str::CompactString;
use intervaltree::IntervalTree;
use parking_lot::RwLock;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::ops::Range as StdRange;
use tower_lsp::lsp_types::{Position, Range};

pub const MAX_CHAIN_DEPTH: usize = 10;

const RANGE_SIZE_LINE_WEIGHT: u64 = 10000;

/// Convert a Position to a 1D point for interval tree operations.
/// Uses 32-bit line number in upper bits, 32-bit character in lower bits.
#[inline]
fn position_to_point(pos: Position) -> u64 {
    ((pos.line as u64) << 32) | (pos.character as u64)
}

/// Convert an LSP Range to an interval for the interval tree.
/// Returns (start, end) where start is inclusive and end is exclusive.
#[inline]
fn range_to_interval(range: Range) -> StdRange<u64> {
    position_to_point(range.start)..position_to_point(range.end)
}

/// A location where an env var is referenced
#[derive(Debug, Clone)]
pub struct EnvVarLocation {
    pub range: Range,
    pub kind: EnvVarLocationKind,
    pub binding_name: Option<CompactString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvVarLocationKind {
    DirectReference,
    BindingDeclaration,
    BindingUsage,
    PropertyAccess,
}

/// Entry for building interval trees during analysis
#[derive(Debug, Clone)]
struct PendingRangeEntry<T: Clone> {
    range: Range,
    value: T,
}

#[derive(Debug)]
pub struct BindingGraph {
    symbols: Vec<Symbol>,

    scopes: Vec<Scope>,

    name_index: FxHashMap<(CompactString, ScopeId), SmallVec<[SymbolId; 2]>>,

    /// Index for fast lookup of all symbols by name only (ignoring scope)
    name_only_index: FxHashMap<CompactString, SmallVec<[SymbolId; 4]>>,

    direct_references: Vec<EnvReference>,

    usages: Vec<SymbolUsage>,

    /// Pending entries for destructure range index (built into tree on rebuild)
    pending_destructure_entries: Vec<PendingRangeEntry<SymbolId>>,

    /// Pending entries for symbol range index (built into tree on rebuild)
    pending_symbol_entries: Vec<PendingRangeEntry<SymbolId>>,

    /// Pending entries for usage range index (built into tree on rebuild)
    pending_usage_entries: Vec<PendingRangeEntry<usize>>,

    /// Pending entries for scope range index (built into tree on rebuild)
    /// Stores (scope_id, size) where size is used to find the most specific scope
    pending_scope_entries: Vec<PendingRangeEntry<(ScopeId, u64)>>,

    /// Interval tree for O(log n) destructure key lookup by position
    destructure_range_tree: Option<IntervalTree<u64, SymbolId>>,

    /// Interval tree for O(log n) symbol lookup by position
    symbol_range_tree: Option<IntervalTree<u64, SymbolId>>,

    /// Interval tree for O(log n) usage lookup by position
    usage_range_tree: Option<IntervalTree<u64, usize>>,

    /// Interval tree for O(log n) scope lookup by position
    scope_range_tree: Option<IntervalTree<u64, (ScopeId, u64)>>,

    /// Index for O(1) lookup of all usages of a given env var name
    env_var_index: FxHashMap<CompactString, Vec<EnvVarLocation>>,

    /// Cache for resolved env vars (symbol_id -> resolved result)
    resolution_cache: FxHashMap<SymbolId, Option<ResolvedEnv>>,

    /// Cache for scope lookups by position (line, character) -> ScopeId
    scope_cache: RwLock<FxHashMap<(u32, u32), ScopeId>>,

    next_symbol_id: u32,

    next_scope_id: u32,
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

impl BindingGraph {
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

        graph.scopes.push(Scope {
            id: ScopeId::root(),
            parent: None,
            range: Range::default(),
            kind: ScopeKind::Module,
        });
        graph.next_scope_id = 2;

        graph
    }

    pub fn set_root_range(&mut self, range: Range) {
        if let Some(root) = self.scopes.first_mut() {
            root.range = range;
        }
    }

    pub fn add_symbol(&mut self, mut symbol: Symbol) -> SymbolId {
        self.next_symbol_id += 1;
        let id = SymbolId::new(self.next_symbol_id)
            .expect("Symbol ID counter overflow - too many symbols");
        symbol.id = id;

        let key = (symbol.name.clone(), symbol.scope);
        self.name_index
            .entry(key)
            .or_default()
            .push(id);

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

    #[inline]
    pub fn get_symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(id.index())
    }

    #[inline]
    pub fn get_symbol_mut(&mut self, id: SymbolId) -> Option<&mut Symbol> {
        self.symbols.get_mut(id.index())
    }

    #[inline]
    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    #[inline]
    pub fn symbols_mut(&mut self) -> &mut [Symbol] {
        &mut self.symbols
    }

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

    pub fn lookup_symbol_id(&self, name: &str, scope: ScopeId) -> Option<SymbolId> {
        self.lookup_symbol(name, scope).map(|s| s.id)
    }

    /// Look up all symbols with the given name across all scopes.
    /// Returns an iterator over symbol IDs. O(1) lookup.
    pub fn lookup_symbols_by_name(&self, name: &str) -> impl Iterator<Item = SymbolId> + '_ {
        self.name_only_index
            .get(name)
            .map(|ids| ids.iter().copied())
            .into_iter()
            .flatten()
    }

    /// O(log n) symbol lookup by position using the interval tree.
    pub fn symbol_at_position(&self, position: Position) -> Option<&Symbol> {
        let tree = self.symbol_range_tree.as_ref()?;
        let point = position_to_point(position);
        tree.query_point(point)
            .filter_map(|entry| self.get_symbol(entry.value))
            .next()
    }

    /// Finalizes the binding graph after batch additions.
    /// Builds interval trees from pending entries for O(log n) position lookups.
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

    /// Helper to get or compute a symbol's resolution, caching the result.
    /// This avoids pre-computing all resolutions and only computes on-demand.
    fn get_or_compute_resolution(&mut self, symbol_id: SymbolId) -> Option<ResolvedEnv> {
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
    /// Also populates the resolution cache lazily as symbols are processed.
    fn build_env_var_index(&mut self) {
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
                            let binding_name =
                                self.get_symbol(symbol_id).map(|s| s.name.clone());
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

    /// Get all locations where the given env var is used. O(1) lookup.
    pub fn get_env_var_locations(&self, env_var_name: &str) -> Option<&Vec<EnvVarLocation>> {
        self.env_var_index.get(env_var_name)
    }

    /// O(log n) destructure key lookup by position using the interval tree.
    pub fn symbol_at_destructure_key(&self, position: Position) -> Option<SymbolId> {
        let tree = self.destructure_range_tree.as_ref()?;
        let point = position_to_point(position);
        tree.query_point(point)
            .map(|entry| entry.value)
            .next()
    }

    pub fn add_scope(&mut self, mut scope: Scope) -> ScopeId {
        let id =
            ScopeId::new(self.next_scope_id).expect("Scope ID counter overflow - too many scopes");
        self.next_scope_id += 1;
        scope.id = id;

        // Add to pending entries - will be built into interval tree on rebuild
        let size = Self::range_size(scope.range);
        self.pending_scope_entries.push(PendingRangeEntry {
            range: scope.range,
            value: (id, size),
        });

        // Clear scope cache since new scope was added
        self.scope_cache.write().clear();

        self.scopes.push(scope);
        id
    }

    #[inline]
    pub fn get_scope(&self, id: ScopeId) -> Option<&Scope> {
        self.scopes.get(id.index())
    }

    #[inline]
    pub fn scopes(&self) -> &[Scope] {
        &self.scopes
    }

    /// O(log n) scope lookup by position using the range index.
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
            if Self::contains_position(root.range, position) {
                let root_size = Self::range_size(root.range);
                if root_size < best_size {
                    best_scope = root.id;
                }
            }
        }

        best_scope
    }

    pub fn add_direct_reference(&mut self, reference: EnvReference) {
        self.direct_references.push(reference);
    }

    #[inline]
    pub fn direct_references(&self) -> &[EnvReference] {
        &self.direct_references
    }

    #[inline]
    pub fn direct_references_mut(&mut self) -> &mut Vec<EnvReference> {
        &mut self.direct_references
    }

    pub fn add_usage(&mut self, usage: SymbolUsage) {
        let usage_index = self.usages.len();
        // Add to pending entries - will be built into interval tree on rebuild
        self.pending_usage_entries.push(PendingRangeEntry {
            range: usage.range,
            value: usage_index,
        });
        self.usages.push(usage);
    }

    #[inline]
    pub fn usages(&self) -> &[SymbolUsage] {
        &self.usages
    }

    #[inline]
    pub fn usages_mut(&mut self) -> &mut Vec<SymbolUsage> {
        &mut self.usages
    }

    /// O(log n) usage lookup by position using the interval tree.
    pub fn usage_at_position(&self, position: Position) -> Option<&SymbolUsage> {
        let tree = self.usage_range_tree.as_ref()?;
        let point = position_to_point(position);
        tree.query_point(point)
            .filter_map(|entry| self.usages.get(entry.value))
            .next()
    }

    /// Resolve a symbol to its env var or env object. Uses cached results if available.
    pub fn resolve_to_env(&self, symbol_id: SymbolId) -> Option<ResolvedEnv> {
        // Try cache first (O(1) lookup)
        if let Some(cached) = self.resolution_cache.get(&symbol_id) {
            return cached.clone();
        }
        // Fall back to walking the chain (for queries before rebuild_range_index)
        self.resolve_to_env_with_depth(symbol_id, MAX_CHAIN_DEPTH, 0)
    }

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

    pub fn resolves_to_env_object(&self, symbol_id: SymbolId) -> bool {
        matches!(self.resolve_to_env(symbol_id), Some(ResolvedEnv::Object(_)))
    }

    pub fn get_env_var_name(&self, symbol_id: SymbolId) -> Option<CompactString> {
        match self.resolve_to_env(symbol_id)? {
            ResolvedEnv::Variable(name) => Some(name),
            ResolvedEnv::Object(_) => None,
        }
    }

    #[inline]
    pub fn contains_position(range: Range, pos: Position) -> bool {
        if pos.line < range.start.line || pos.line > range.end.line {
            return false;
        }
        if pos.line == range.start.line && pos.character < range.start.character {
            return false;
        }

        if pos.line == range.end.line && pos.character >= range.end.character {
            return false;
        }
        true
    }

    #[inline]
    pub fn is_range_contained(inner: Range, outer: Range) -> bool {
        if inner.start.line < outer.start.line {
            return false;
        }
        if inner.start.line == outer.start.line && inner.start.character < outer.start.character {
            return false;
        }

        if inner.end.line > outer.end.line {
            return false;
        }
        if inner.end.line == outer.end.line && inner.end.character > outer.end.character {
            return false;
        }

        true
    }

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

    /// Check if two ranges overlap.
    #[inline]
    pub fn ranges_overlap(a: Range, b: Range) -> bool {
        // No overlap if one ends before the other starts
        if a.end.line < b.start.line
            || (a.end.line == b.start.line && a.end.character <= b.start.character)
        {
            return false;
        }
        if b.end.line < a.start.line
            || (b.end.line == a.start.line && b.end.character <= a.start.character)
        {
            return false;
        }
        true
    }

    /// Remove all symbols, usages, and direct references that overlap with the given range.
    /// This is used for incremental analysis to clear affected regions before re-analysis.
    /// Returns the number of items removed.
    pub fn remove_in_range(&mut self, range: Range) -> usize {
        let mut removed = 0;

        // Track which symbol IDs are being removed for cleanup
        let removed_symbol_ids: FxHashSet<SymbolId> = self
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
    /// Returns true if the edit covers more than 50% of the document.
    pub fn is_large_edit(&self, edit_range: Range) -> bool {
        let (doc_lines, _) = self.document_size();
        if doc_lines == 0 {
            return true; // Treat empty document as needing full analysis
        }
        let edit_lines = edit_range.end.line.saturating_sub(edit_range.start.line) + 1;
        edit_lines > doc_lines / 2
    }

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

        self.scopes.push(Scope {
            id: ScopeId::root(),
            parent: None,
            range: Range::default(),
            kind: ScopeKind::Module,
        });
        self.next_scope_id = 2;
    }

    pub fn stats(&self) -> BindingGraphStats {
        BindingGraphStats {
            symbol_count: self.symbols.len(),
            scope_count: self.scopes.len(),
            usage_count: self.usages.len(),
            direct_reference_count: self.direct_references.len(),
        }
    }
}

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
            id: SymbolId::new(1).unwrap(),
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

        let found = graph.get_symbol(id).unwrap();
        assert_eq!(found.name, "dbUrl");

        let found = graph.lookup_symbol("dbUrl", ScopeId::root()).unwrap();
        assert_eq!(found.id, id);
    }

    #[test]
    fn test_scope_chain_lookup() {
        let mut graph = BindingGraph::new();

        let func_scope_id = graph.add_scope(Scope {
            id: ScopeId::root(),
            parent: Some(ScopeId::root()),
            range: make_range(5, 0, 10, 1),
            kind: ScopeKind::Function,
        });

        let root_symbol = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
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

        let found = graph.lookup_symbol("globalEnv", func_scope_id).unwrap();
        assert_eq!(found.id, root_symbol);
    }

    #[test]
    fn test_resolve_env_chain() {
        let mut graph = BindingGraph::new();

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

        let func_scope_id = graph.add_scope(Scope {
            id: ScopeId::root(),
            parent: Some(ScopeId::root()),
            range: make_range(5, 0, 10, 1),
            kind: ScopeKind::Function,
        });
        graph.rebuild_range_index();

        let scope = graph.scope_at_position(Position::new(2, 5));
        assert_eq!(scope, ScopeId::root());

        let scope = graph.scope_at_position(Position::new(7, 5));
        assert_eq!(scope, func_scope_id);
    }

    #[test]
    fn test_contains_position() {
        let range = make_range(5, 10, 5, 20);

        assert!(BindingGraph::contains_position(range, Position::new(5, 10)));
        assert!(BindingGraph::contains_position(range, Position::new(5, 15)));
        assert!(BindingGraph::contains_position(range, Position::new(5, 19)));

        assert!(!BindingGraph::contains_position(
            range,
            Position::new(5, 20)
        ));
        assert!(!BindingGraph::contains_position(
            range,
            Position::new(5, 21)
        ));

        assert!(!BindingGraph::contains_position(range, Position::new(5, 9)));

        assert!(!BindingGraph::contains_position(
            range,
            Position::new(4, 15)
        ));
        assert!(!BindingGraph::contains_position(
            range,
            Position::new(6, 15)
        ));
    }

    #[test]
    fn test_set_root_range() {
        let mut graph = BindingGraph::new();
        let range = make_range(0, 0, 100, 0);
        graph.set_root_range(range);

        let root = graph.get_scope(ScopeId::root()).unwrap();
        assert_eq!(root.range, range);
    }

    #[test]
    fn test_get_symbol_mut() {
        let mut graph = BindingGraph::new();

        let symbol = Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "test".into(),
            declaration_range: make_range(0, 0, 0, 10),
            name_range: make_range(0, 0, 0, 4),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        };

        let id = graph.add_symbol(symbol);

        if let Some(sym) = graph.get_symbol_mut(id) {
            sym.is_valid = false;
        }

        let sym = graph.get_symbol(id).unwrap();
        assert!(!sym.is_valid);
    }

    #[test]
    fn test_symbols_mut() {
        let mut graph = BindingGraph::new();

        let symbol = Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "test".into(),
            declaration_range: make_range(0, 0, 0, 10),
            name_range: make_range(0, 0, 0, 4),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        };

        graph.add_symbol(symbol);

        for sym in graph.symbols_mut() {
            sym.is_valid = false;
        }

        assert!(!graph.symbols()[0].is_valid);
    }

    #[test]
    fn test_symbol_at_position() {
        let mut graph = BindingGraph::new();

        let symbol = Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "test".into(),
            declaration_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 6, 0, 10),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        };

        graph.add_symbol(symbol);
        graph.rebuild_range_index();

        let found = graph.symbol_at_position(Position::new(0, 8));
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "test");

        let not_found = graph.symbol_at_position(Position::new(0, 0));
        assert!(not_found.is_none());
    }

    #[test]
    fn test_symbol_at_destructure_key() {
        let mut graph = BindingGraph::new();

        let symbol = Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "dbUrl".into(),
            declaration_range: make_range(0, 0, 0, 40),
            name_range: make_range(0, 24, 0, 29),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "DATABASE_URL".into(),
            },
            kind: SymbolKind::DestructuredProperty,
            is_valid: true,
            destructured_key_range: Some(make_range(0, 8, 0, 20)),
        };

        let id = graph.add_symbol(symbol);
        graph.rebuild_range_index();

        let found = graph.symbol_at_destructure_key(Position::new(0, 10));
        assert!(found.is_some());
        assert_eq!(found.unwrap(), id);

        let not_found = graph.symbol_at_destructure_key(Position::new(0, 30));
        assert!(not_found.is_none());
    }

    #[test]
    fn test_direct_references() {
        use crate::types::AccessType;

        let mut graph = BindingGraph::new();

        let reference = EnvReference {
            name: "DATABASE_URL".into(),
            full_range: make_range(0, 0, 0, 22),
            name_range: make_range(0, 10, 0, 22),
            access_type: AccessType::Property,
            has_default: false,
            default_value: None,
        };

        graph.add_direct_reference(reference);

        assert_eq!(graph.direct_references().len(), 1);
        assert_eq!(graph.direct_references()[0].name, "DATABASE_URL");

        graph.direct_references_mut().clear();
        assert!(graph.direct_references().is_empty());
    }

    #[test]
    fn test_usages() {
        let mut graph = BindingGraph::new();

        let symbol_id = graph.add_symbol(Symbol {
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

        let usage = SymbolUsage {
            symbol_id,
            range: make_range(1, 10, 1, 23),
            scope: ScopeId::root(),
            property_access: Some("DATABASE_URL".into()),
            property_access_range: Some(make_range(1, 14, 1, 26)),
        };

        graph.add_usage(usage);

        assert_eq!(graph.usages().len(), 1);
        assert_eq!(
            graph.usages()[0].property_access.as_ref().unwrap(),
            "DATABASE_URL"
        );
    }

    #[test]
    fn test_usage_at_position() {
        let mut graph = BindingGraph::new();

        let symbol_id = graph.add_symbol(Symbol {
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

        let usage = SymbolUsage {
            symbol_id,
            range: make_range(1, 10, 1, 23),
            scope: ScopeId::root(),
            property_access: Some("DATABASE_URL".into()),
            property_access_range: None,
        };

        graph.add_usage(usage);
        graph.rebuild_range_index();

        let found = graph.usage_at_position(Position::new(1, 15));
        assert!(found.is_some());

        let not_found = graph.usage_at_position(Position::new(2, 0));
        assert!(not_found.is_none());
    }

    #[test]
    fn test_resolve_with_max_depth() {
        let mut graph = BindingGraph::new();

        let a_id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "a".into(),
            declaration_range: make_range(0, 0, 0, 10),
            name_range: make_range(0, 6, 0, 7),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvObject {
                canonical_name: "process.env".into(),
            },
            kind: SymbolKind::EnvObject,
            is_valid: true,
            destructured_key_range: None,
        });

        let b_id = graph.add_symbol(Symbol {
            id: SymbolId::new(2).unwrap(),
            name: "b".into(),
            declaration_range: make_range(1, 0, 1, 10),
            name_range: make_range(1, 6, 1, 7),
            scope: ScopeId::root(),
            origin: SymbolOrigin::Symbol { target: a_id },
            kind: SymbolKind::Variable,
            is_valid: true,
            destructured_key_range: None,
        });

        let c_id = graph.add_symbol(Symbol {
            id: SymbolId::new(3).unwrap(),
            name: "c".into(),
            declaration_range: make_range(2, 0, 2, 10),
            name_range: make_range(2, 6, 2, 7),
            scope: ScopeId::root(),
            origin: SymbolOrigin::Symbol { target: b_id },
            kind: SymbolKind::Variable,
            is_valid: true,
            destructured_key_range: None,
        });

        let d_id = graph.add_symbol(Symbol {
            id: SymbolId::new(4).unwrap(),
            name: "d".into(),
            declaration_range: make_range(3, 0, 3, 10),
            name_range: make_range(3, 6, 3, 7),
            scope: ScopeId::root(),
            origin: SymbolOrigin::Symbol { target: c_id },
            kind: SymbolKind::Variable,
            is_valid: true,
            destructured_key_range: None,
        });

        assert!(graph.resolve_to_env(d_id).is_some());

        assert!(graph.resolve_to_env_with_max(d_id, 2).is_none());
        assert!(graph.resolve_to_env_with_max(d_id, 5).is_some());
    }

    #[test]
    fn test_resolves_to_env_object() {
        let mut graph = BindingGraph::new();

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

        let var_id = graph.add_symbol(Symbol {
            id: SymbolId::new(2).unwrap(),
            name: "db".into(),
            declaration_range: make_range(1, 0, 1, 25),
            name_range: make_range(1, 6, 1, 8),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "DATABASE_URL".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        assert!(graph.resolves_to_env_object(env_id));
        assert!(!graph.resolves_to_env_object(var_id));
    }

    #[test]
    fn test_get_env_var_name() {
        let mut graph = BindingGraph::new();

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

        let var_id = graph.add_symbol(Symbol {
            id: SymbolId::new(2).unwrap(),
            name: "db".into(),
            declaration_range: make_range(1, 0, 1, 25),
            name_range: make_range(1, 6, 1, 8),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "DATABASE_URL".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        assert!(graph.get_env_var_name(env_id).is_none());

        assert_eq!(graph.get_env_var_name(var_id), Some("DATABASE_URL".into()));
    }

    #[test]
    fn test_is_range_contained() {
        let outer = make_range(0, 0, 10, 50);
        let inner = make_range(2, 10, 5, 30);
        let outside = make_range(15, 0, 20, 50);
        let partial = make_range(0, 0, 15, 0);

        assert!(BindingGraph::is_range_contained(inner, outer));
        assert!(BindingGraph::is_range_contained(outer, outer));
        assert!(!BindingGraph::is_range_contained(outside, outer));
        assert!(!BindingGraph::is_range_contained(partial, outer));
    }

    #[test]
    fn test_clear() {
        let mut graph = BindingGraph::new();

        graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "test".into(),
            declaration_range: make_range(0, 0, 0, 10),
            name_range: make_range(0, 0, 0, 4),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        graph.add_scope(Scope {
            id: ScopeId::root(),
            parent: Some(ScopeId::root()),
            range: make_range(5, 0, 10, 0),
            kind: ScopeKind::Function,
        });

        graph.add_direct_reference(EnvReference {
            name: "TEST".into(),
            full_range: make_range(0, 0, 0, 10),
            name_range: make_range(0, 0, 0, 4),
            access_type: crate::types::AccessType::Property,
            has_default: false,
            default_value: None,
        });

        graph.clear();

        assert!(graph.symbols().is_empty());
        assert_eq!(graph.scopes().len(), 1);
        assert!(graph.direct_references().is_empty());
        assert!(graph.usages().is_empty());
    }

    #[test]
    fn test_stats() {
        let mut graph = BindingGraph::new();

        graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "test".into(),
            declaration_range: make_range(0, 0, 0, 10),
            name_range: make_range(0, 0, 0, 4),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        graph.add_direct_reference(EnvReference {
            name: "TEST".into(),
            full_range: make_range(0, 0, 0, 10),
            name_range: make_range(0, 0, 0, 4),
            access_type: crate::types::AccessType::Property,
            has_default: false,
            default_value: None,
        });

        let stats = graph.stats();
        assert_eq!(stats.symbol_count, 1);
        assert_eq!(stats.scope_count, 1);
        assert_eq!(stats.usage_count, 0);
        assert_eq!(stats.direct_reference_count, 1);
    }

    #[test]
    fn test_invalid_symbol_not_found_in_lookup() {
        let mut graph = BindingGraph::new();

        let id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "test".into(),
            declaration_range: make_range(0, 0, 0, 10),
            name_range: make_range(0, 0, 0, 4),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: false,
            destructured_key_range: None,
        });

        let found = graph.lookup_symbol("test", ScopeId::root());
        assert!(found.is_none());

        let by_id = graph.get_symbol(id);
        assert!(by_id.is_some());
    }

    #[test]
    fn test_multiple_symbols_same_name_different_scopes() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(make_range(0, 0, 20, 0));

        let func_scope = graph.add_scope(Scope {
            id: ScopeId::root(),
            parent: Some(ScopeId::root()),
            range: make_range(5, 0, 15, 0),
            kind: ScopeKind::Function,
        });

        let root_id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "db".into(),
            declaration_range: make_range(0, 0, 0, 30),
            name_range: make_range(0, 6, 0, 8),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "ROOT_DB".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let func_id = graph.add_symbol(Symbol {
            id: SymbolId::new(2).unwrap(),
            name: "db".into(),
            declaration_range: make_range(6, 0, 6, 30),
            name_range: make_range(6, 6, 6, 8),
            scope: func_scope,
            origin: SymbolOrigin::EnvVar {
                name: "FUNC_DB".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let found = graph.lookup_symbol("db", func_scope).unwrap();
        assert_eq!(found.id, func_id);

        let found = graph.lookup_symbol("db", ScopeId::root()).unwrap();
        assert_eq!(found.id, root_id);
    }

    #[test]
    fn test_resolve_unresolvable_origins() {
        let mut graph = BindingGraph::new();

        let unknown_id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "unknown".into(),
            declaration_range: make_range(0, 0, 0, 10),
            name_range: make_range(0, 0, 0, 7),
            scope: ScopeId::root(),
            origin: SymbolOrigin::Unknown,
            kind: SymbolKind::Variable,
            is_valid: true,
            destructured_key_range: None,
        });

        let unresolvable_id = graph.add_symbol(Symbol {
            id: SymbolId::new(2).unwrap(),
            name: "unresolvable".into(),
            declaration_range: make_range(1, 0, 1, 15),
            name_range: make_range(1, 0, 1, 12),
            scope: ScopeId::root(),
            origin: SymbolOrigin::Unresolvable,
            kind: SymbolKind::Variable,
            is_valid: true,
            destructured_key_range: None,
        });

        assert!(graph.resolve_to_env(unknown_id).is_none());
        assert!(graph.resolve_to_env(unresolvable_id).is_none());
    }

    #[test]
    fn test_usages_mut() {
        let mut graph = BindingGraph::new();

        let symbol_id = graph.add_symbol(Symbol {
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

        graph.add_usage(SymbolUsage {
            symbol_id,
            range: make_range(1, 0, 1, 10),
            scope: ScopeId::root(),
            property_access: None,
            property_access_range: None,
        });

        graph.usages_mut().clear();
        assert!(graph.usages().is_empty());
    }

    #[test]
    fn test_contains_position_multiline() {
        let range = make_range(5, 10, 8, 20);

        assert!(BindingGraph::contains_position(range, Position::new(5, 10)));
        assert!(BindingGraph::contains_position(range, Position::new(5, 50)));
        assert!(!BindingGraph::contains_position(range, Position::new(5, 9)));

        assert!(BindingGraph::contains_position(range, Position::new(6, 0)));
        assert!(BindingGraph::contains_position(
            range,
            Position::new(7, 100)
        ));

        assert!(BindingGraph::contains_position(range, Position::new(8, 0)));
        assert!(BindingGraph::contains_position(range, Position::new(8, 19)));
        assert!(!BindingGraph::contains_position(
            range,
            Position::new(8, 20)
        ));
        assert!(!BindingGraph::contains_position(
            range,
            Position::new(8, 21)
        ));
    }

    // =========================================================================
    // Task 1: Interval Tree Tests - Helper Functions
    // =========================================================================

    #[test]
    fn test_position_to_point() {
        // Line 0, char 0 should be 0
        assert_eq!(position_to_point(Position::new(0, 0)), 0);

        // Line 0, char 10 should be 10
        assert_eq!(position_to_point(Position::new(0, 10)), 10);

        // Line 1, char 0 should have line in upper 32 bits
        let point = position_to_point(Position::new(1, 0));
        assert_eq!(point >> 32, 1);
        assert_eq!(point & 0xFFFFFFFF, 0);

        // Line 5, char 15
        let point = position_to_point(Position::new(5, 15));
        assert_eq!(point >> 32, 5);
        assert_eq!(point & 0xFFFFFFFF, 15);

        // Large line number
        let point = position_to_point(Position::new(1000, 500));
        assert_eq!(point >> 32, 1000);
        assert_eq!(point & 0xFFFFFFFF, 500);
    }

    #[test]
    fn test_range_to_interval() {
        let range = make_range(5, 10, 8, 20);
        let interval = range_to_interval(range);

        // Start should be (5 << 32) | 10
        assert_eq!(interval.start, (5u64 << 32) | 10);
        // End should be (8 << 32) | 20
        assert_eq!(interval.end, (8u64 << 32) | 20);

        // Verify it's a half-open interval [start, end)
        assert!(interval.start < interval.end);
    }

    #[test]
    fn test_range_size_single_line() {
        // Single line range: just character difference
        let range = make_range(5, 10, 5, 30);
        let size = BindingGraph::range_size(range);
        // lines = 0, chars = 20
        assert_eq!(size, 20);
    }

    #[test]
    fn test_range_size_multi_line() {
        // Multi-line range: uses RANGE_SIZE_LINE_WEIGHT
        let range = make_range(5, 10, 8, 20);
        let size = BindingGraph::range_size(range);
        // lines = 3, chars = end.character = 20
        // size = 3 * RANGE_SIZE_LINE_WEIGHT + 20
        assert_eq!(size, 3 * RANGE_SIZE_LINE_WEIGHT + 20);
    }

    #[test]
    fn test_ranges_overlap_separate() {
        // Ranges on separate lines, no overlap
        let a = make_range(0, 0, 5, 10);
        let b = make_range(10, 0, 15, 10);
        assert!(!BindingGraph::ranges_overlap(a, b));
        assert!(!BindingGraph::ranges_overlap(b, a));
    }

    #[test]
    fn test_ranges_overlap_adjacent() {
        // Adjacent ranges (end of a == start of b) - no overlap
        let a = make_range(0, 0, 5, 10);
        let b = make_range(5, 10, 10, 0);
        assert!(!BindingGraph::ranges_overlap(a, b));
        assert!(!BindingGraph::ranges_overlap(b, a));
    }

    #[test]
    fn test_ranges_overlap_partial() {
        // Partial overlap
        let a = make_range(0, 0, 5, 10);
        let b = make_range(3, 5, 10, 0);
        assert!(BindingGraph::ranges_overlap(a, b));
        assert!(BindingGraph::ranges_overlap(b, a));
    }

    #[test]
    fn test_ranges_overlap_contained() {
        // One range completely contained in another
        let outer = make_range(0, 0, 10, 0);
        let inner = make_range(3, 5, 7, 10);
        assert!(BindingGraph::ranges_overlap(outer, inner));
        assert!(BindingGraph::ranges_overlap(inner, outer));
    }

    #[test]
    fn test_ranges_overlap_identical() {
        // Identical ranges
        let a = make_range(5, 10, 8, 20);
        let b = make_range(5, 10, 8, 20);
        assert!(BindingGraph::ranges_overlap(a, b));
    }

    #[test]
    fn test_ranges_overlap_same_line() {
        // Same line, different columns
        let a = make_range(5, 0, 5, 10);
        let b = make_range(5, 5, 5, 15);
        assert!(BindingGraph::ranges_overlap(a, b));

        // Non-overlapping on same line
        let c = make_range(5, 0, 5, 5);
        let d = make_range(5, 10, 5, 15);
        assert!(!BindingGraph::ranges_overlap(c, d));
    }

    // =========================================================================
    // Task 1: Interval Tree Tests - Position Lookup Edge Cases
    // =========================================================================

    #[test]
    fn test_symbol_at_position_empty_tree() {
        let graph = BindingGraph::new();
        // No symbols, no rebuild - should return None
        assert!(graph.symbol_at_position(Position::new(0, 0)).is_none());
    }

    #[test]
    fn test_symbol_at_position_before_rebuild() {
        let mut graph = BindingGraph::new();

        graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "test".into(),
            declaration_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 6, 0, 10),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        // Before rebuild_range_index, tree is not built
        assert!(graph.symbol_at_position(Position::new(0, 8)).is_none());
    }

    #[test]
    fn test_symbol_at_position_boundary_conditions() {
        let mut graph = BindingGraph::new();

        graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "test".into(),
            declaration_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 6, 0, 10),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });
        graph.rebuild_range_index();

        // Start position (inclusive) - should be found
        let found = graph.symbol_at_position(Position::new(0, 6));
        assert!(found.is_some());

        // End position (exclusive) - should NOT be found
        let not_found = graph.symbol_at_position(Position::new(0, 10));
        assert!(not_found.is_none());

        // Just before end - should be found
        let found = graph.symbol_at_position(Position::new(0, 9));
        assert!(found.is_some());

        // Just before start - should NOT be found
        let not_found = graph.symbol_at_position(Position::new(0, 5));
        assert!(not_found.is_none());
    }

    #[test]
    fn test_symbol_at_position_multiple_symbols() {
        let mut graph = BindingGraph::new();

        let id1 = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "first".into(),
            declaration_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 6, 0, 11),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "FIRST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let id2 = graph.add_symbol(Symbol {
            id: SymbolId::new(2).unwrap(),
            name: "second".into(),
            declaration_range: make_range(1, 0, 1, 20),
            name_range: make_range(1, 6, 1, 12),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "SECOND".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        graph.rebuild_range_index();

        // First symbol
        let found = graph.symbol_at_position(Position::new(0, 8));
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, id1);

        // Second symbol
        let found = graph.symbol_at_position(Position::new(1, 8));
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, id2);
    }

    #[test]
    fn test_scope_at_position_nested_scopes() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(make_range(0, 0, 20, 0));

        // Add an outer function scope
        let outer_scope = graph.add_scope(Scope {
            id: ScopeId::root(),
            parent: Some(ScopeId::root()),
            range: make_range(2, 0, 18, 1),
            kind: ScopeKind::Function,
        });

        // Add an inner block scope
        let inner_scope = graph.add_scope(Scope {
            id: ScopeId::root(),
            parent: Some(outer_scope),
            range: make_range(5, 4, 10, 5),
            kind: ScopeKind::Block,
        });

        graph.rebuild_range_index();

        // Position in inner scope should return the most specific (smallest) scope
        let scope = graph.scope_at_position(Position::new(7, 10));
        assert_eq!(scope, inner_scope);

        // Position in outer scope but outside inner should return outer
        let scope = graph.scope_at_position(Position::new(3, 10));
        assert_eq!(scope, outer_scope);

        // Position outside all scopes (but in root) should return root
        let scope = graph.scope_at_position(Position::new(19, 0));
        assert_eq!(scope, ScopeId::root());
    }

    #[test]
    fn test_scope_at_position_empty_tree() {
        let graph = BindingGraph::new();
        // Empty graph should return root scope
        let scope = graph.scope_at_position(Position::new(0, 0));
        assert_eq!(scope, ScopeId::root());
    }

    #[test]
    fn test_usage_at_position_empty_tree() {
        let graph = BindingGraph::new();
        assert!(graph.usage_at_position(Position::new(0, 0)).is_none());
    }

    #[test]
    fn test_usage_at_position_multiple_usages() {
        let mut graph = BindingGraph::new();

        let symbol_id = graph.add_symbol(Symbol {
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

        graph.add_usage(SymbolUsage {
            symbol_id,
            range: make_range(1, 10, 1, 13),
            scope: ScopeId::root(),
            property_access: Some("DB_URL".into()),
            property_access_range: None,
        });

        graph.add_usage(SymbolUsage {
            symbol_id,
            range: make_range(2, 10, 2, 13),
            scope: ScopeId::root(),
            property_access: Some("API_KEY".into()),
            property_access_range: None,
        });

        graph.rebuild_range_index();

        // First usage
        let usage = graph.usage_at_position(Position::new(1, 11));
        assert!(usage.is_some());
        assert_eq!(usage.unwrap().property_access.as_deref(), Some("DB_URL"));

        // Second usage
        let usage = graph.usage_at_position(Position::new(2, 11));
        assert!(usage.is_some());
        assert_eq!(usage.unwrap().property_access.as_deref(), Some("API_KEY"));
    }

    #[test]
    fn test_symbol_at_destructure_key_empty_tree() {
        let graph = BindingGraph::new();
        assert!(graph.symbol_at_destructure_key(Position::new(0, 0)).is_none());
    }

    #[test]
    fn test_symbol_at_destructure_key_multiple_keys() {
        let mut graph = BindingGraph::new();

        let id1 = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "dbUrl".into(),
            declaration_range: make_range(0, 0, 0, 50),
            name_range: make_range(0, 30, 0, 35),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "DATABASE_URL".into(),
            },
            kind: SymbolKind::DestructuredProperty,
            is_valid: true,
            destructured_key_range: Some(make_range(0, 8, 0, 20)),
        });

        let id2 = graph.add_symbol(Symbol {
            id: SymbolId::new(2).unwrap(),
            name: "apiKey".into(),
            declaration_range: make_range(1, 0, 1, 50),
            name_range: make_range(1, 30, 1, 36),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "API_KEY".into(),
            },
            kind: SymbolKind::DestructuredProperty,
            is_valid: true,
            destructured_key_range: Some(make_range(1, 8, 1, 15)),
        });

        graph.rebuild_range_index();

        // First key
        let found = graph.symbol_at_destructure_key(Position::new(0, 10));
        assert!(found.is_some());
        assert_eq!(found.unwrap(), id1);

        // Second key
        let found = graph.symbol_at_destructure_key(Position::new(1, 10));
        assert!(found.is_some());
        assert_eq!(found.unwrap(), id2);

        // Outside any key
        assert!(graph.symbol_at_destructure_key(Position::new(0, 25)).is_none());
    }

    // =========================================================================
    // Task 1: Interval Tree Tests - Rebuild Range Index
    // =========================================================================

    #[test]
    fn test_rebuild_range_index_builds_all_trees() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(make_range(0, 0, 10, 0));

        // Add a symbol
        graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "test".into(),
            declaration_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 6, 0, 10),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: Some(make_range(0, 6, 0, 10)),
        });

        // Add a scope
        graph.add_scope(Scope {
            id: ScopeId::root(),
            parent: Some(ScopeId::root()),
            range: make_range(2, 0, 8, 0),
            kind: ScopeKind::Function,
        });

        // Add a usage
        let symbol_id = SymbolId::new(1).unwrap();
        graph.add_usage(SymbolUsage {
            symbol_id,
            range: make_range(5, 0, 5, 10),
            scope: ScopeId::root(),
            property_access: None,
            property_access_range: None,
        });

        // Before rebuild, trees should be None
        assert!(graph.symbol_range_tree.is_none());
        assert!(graph.scope_range_tree.is_none());
        assert!(graph.usage_range_tree.is_none());
        assert!(graph.destructure_range_tree.is_none());

        graph.rebuild_range_index();

        // After rebuild, trees should be built
        assert!(graph.symbol_range_tree.is_some());
        assert!(graph.scope_range_tree.is_some());
        assert!(graph.usage_range_tree.is_some());
        assert!(graph.destructure_range_tree.is_some());
    }

    #[test]
    fn test_rebuild_range_index_clears_scope_cache() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(make_range(0, 0, 10, 0));

        graph.add_scope(Scope {
            id: ScopeId::root(),
            parent: Some(ScopeId::root()),
            range: make_range(2, 0, 8, 0),
            kind: ScopeKind::Function,
        });

        graph.rebuild_range_index();

        // Access scope to populate cache
        let _ = graph.scope_at_position(Position::new(5, 0));
        assert!(!graph.scope_cache.read().is_empty());

        // Rebuild should clear cache
        graph.rebuild_range_index();
        assert!(graph.scope_cache.read().is_empty());
    }

    #[test]
    fn test_rebuild_range_index_idempotent() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(make_range(0, 0, 10, 0));

        graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "test".into(),
            declaration_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 6, 0, 10),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        graph.rebuild_range_index();
        let first_result = graph.symbol_at_position(Position::new(0, 8)).map(|s| s.id);

        graph.rebuild_range_index();
        let second_result = graph.symbol_at_position(Position::new(0, 8)).map(|s| s.id);

        assert_eq!(first_result, second_result);
    }

    #[test]
    fn test_rebuild_range_index_empty_graph() {
        let mut graph = BindingGraph::new();
        // Should not panic on empty graph
        graph.rebuild_range_index();

        // Trees should remain None for empty pending entries
        assert!(graph.symbol_range_tree.is_none());
        assert!(graph.usage_range_tree.is_none());
        assert!(graph.destructure_range_tree.is_none());
    }

    // =========================================================================
    // Task 2: Incremental Analysis Tests - remove_in_range
    // =========================================================================

    #[test]
    fn test_remove_in_range_basic() {
        let mut graph = BindingGraph::new();

        // Add symbols in different ranges
        let id1 = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "kept".into(),
            declaration_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 6, 0, 10),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "KEPT".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        graph.add_symbol(Symbol {
            id: SymbolId::new(2).unwrap(),
            name: "removed".into(),
            declaration_range: make_range(5, 0, 5, 20),
            name_range: make_range(5, 6, 5, 13),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "REMOVED".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let removed_count = graph.remove_in_range(make_range(4, 0, 6, 0));

        assert_eq!(removed_count, 1);
        assert_eq!(graph.symbols().len(), 1);
        assert!(graph.get_symbol(id1).is_some());
        assert_eq!(graph.get_symbol(id1).unwrap().name, "kept");
    }

    #[test]
    fn test_remove_in_range_removes_usages() {
        let mut graph = BindingGraph::new();

        let symbol_id = graph.add_symbol(Symbol {
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

        // Add usage in range to be removed
        graph.add_usage(SymbolUsage {
            symbol_id,
            range: make_range(5, 0, 5, 10),
            scope: ScopeId::root(),
            property_access: None,
            property_access_range: None,
        });

        // Add usage outside range
        graph.add_usage(SymbolUsage {
            symbol_id,
            range: make_range(10, 0, 10, 10),
            scope: ScopeId::root(),
            property_access: None,
            property_access_range: None,
        });

        assert_eq!(graph.usages().len(), 2);

        graph.remove_in_range(make_range(4, 0, 6, 0));

        assert_eq!(graph.usages().len(), 1);
        assert_eq!(graph.usages()[0].range.start.line, 10);
    }

    #[test]
    fn test_remove_in_range_removes_direct_references() {
        use crate::types::AccessType;

        let mut graph = BindingGraph::new();

        // Add reference in range to be removed
        graph.add_direct_reference(EnvReference {
            name: "DB_URL".into(),
            full_range: make_range(5, 0, 5, 30),
            name_range: make_range(5, 20, 5, 26),
            access_type: AccessType::Property,
            has_default: false,
            default_value: None,
        });

        // Add reference outside range
        graph.add_direct_reference(EnvReference {
            name: "API_KEY".into(),
            full_range: make_range(10, 0, 10, 30),
            name_range: make_range(10, 20, 10, 27),
            access_type: AccessType::Property,
            has_default: false,
            default_value: None,
        });

        assert_eq!(graph.direct_references().len(), 2);

        graph.remove_in_range(make_range(4, 0, 6, 0));

        assert_eq!(graph.direct_references().len(), 1);
        assert_eq!(graph.direct_references()[0].name, "API_KEY");
    }

    #[test]
    fn test_remove_in_range_cleans_name_indices() {
        let mut graph = BindingGraph::new();

        graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "removed".into(),
            declaration_range: make_range(5, 0, 5, 20),
            name_range: make_range(5, 6, 5, 13),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "REMOVED".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        // Verify symbol is in name index
        assert!(graph.lookup_symbol("removed", ScopeId::root()).is_some());

        graph.remove_in_range(make_range(4, 0, 6, 0));

        // Symbol should be removed from name index
        assert!(graph.lookup_symbol("removed", ScopeId::root()).is_none());
    }

    #[test]
    fn test_remove_in_range_clears_caches() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(make_range(0, 0, 20, 0));

        let symbol_id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "test".into(),
            declaration_range: make_range(5, 0, 5, 20),
            name_range: make_range(5, 6, 5, 10),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        graph.rebuild_range_index();

        // Populate caches
        let _ = graph.scope_at_position(Position::new(5, 0));
        let _ = graph.resolve_to_env(symbol_id);

        assert!(!graph.scope_cache.read().is_empty());
        assert!(!graph.resolution_cache.is_empty());

        graph.remove_in_range(make_range(4, 0, 6, 0));

        // Caches should be cleared
        assert!(graph.scope_cache.read().is_empty());
        assert!(graph.resolution_cache.is_empty());
    }

    #[test]
    fn test_remove_in_range_invalidates_trees() {
        let mut graph = BindingGraph::new();

        graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "test".into(),
            declaration_range: make_range(5, 0, 5, 20),
            name_range: make_range(5, 6, 5, 10),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: Some(make_range(5, 6, 5, 10)),
        });

        graph.rebuild_range_index();

        assert!(graph.symbol_range_tree.is_some());
        assert!(graph.destructure_range_tree.is_some());

        graph.remove_in_range(make_range(4, 0, 6, 0));

        // Trees should be invalidated
        assert!(graph.symbol_range_tree.is_none());
        assert!(graph.usage_range_tree.is_none());
        assert!(graph.destructure_range_tree.is_none());
    }

    #[test]
    fn test_remove_in_range_cascading_usages() {
        let mut graph = BindingGraph::new();

        // Add symbol that will be removed
        let removed_symbol_id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "env".into(),
            declaration_range: make_range(5, 0, 5, 25),
            name_range: make_range(5, 6, 5, 9),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvObject {
                canonical_name: "process.env".into(),
            },
            kind: SymbolKind::EnvObject,
            is_valid: true,
            destructured_key_range: None,
        });

        // Add usage referencing the removed symbol (outside the edit range)
        graph.add_usage(SymbolUsage {
            symbol_id: removed_symbol_id,
            range: make_range(10, 0, 10, 10), // Outside edit range
            scope: ScopeId::root(),
            property_access: Some("DB_URL".into()),
            property_access_range: None,
        });

        assert_eq!(graph.usages().len(), 1);

        // Remove the symbol - usages referencing it should also be removed
        graph.remove_in_range(make_range(4, 0, 6, 0));

        // Usage should be removed because it references the removed symbol
        assert_eq!(graph.usages().len(), 0);
    }

    #[test]
    fn test_remove_in_range_empty_range() {
        let mut graph = BindingGraph::new();

        graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "test".into(),
            declaration_range: make_range(5, 0, 5, 20),
            name_range: make_range(5, 6, 5, 10),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let original_count = graph.symbols().len();

        // Empty range (same start and end)
        let removed = graph.remove_in_range(make_range(0, 0, 0, 0));

        assert_eq!(removed, 0);
        assert_eq!(graph.symbols().len(), original_count);
    }

    #[test]
    fn test_remove_in_range_entire_document() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(make_range(0, 0, 100, 0));

        for i in 0..5 {
            graph.add_symbol(Symbol {
                id: SymbolId::new(i as u32 + 1).unwrap(),
                name: format!("sym{}", i).into(),
                declaration_range: make_range(i * 10, 0, i * 10, 20),
                name_range: make_range(i * 10, 6, i * 10, 10),
                scope: ScopeId::root(),
                origin: SymbolOrigin::EnvVar {
                    name: format!("VAR{}", i).into(),
                },
                kind: SymbolKind::Value,
                is_valid: true,
                destructured_key_range: None,
            });
        }

        assert_eq!(graph.symbols().len(), 5);

        // Remove entire document range
        let removed = graph.remove_in_range(make_range(0, 0, 100, 0));

        assert_eq!(removed, 5);
        assert!(graph.symbols().is_empty());
    }

    #[test]
    fn test_remove_in_range_clears_pending_entries() {
        let mut graph = BindingGraph::new();

        graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "test".into(),
            declaration_range: make_range(5, 0, 5, 20),
            name_range: make_range(5, 6, 5, 10),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: Some(make_range(5, 6, 5, 10)),
        });

        // Don't rebuild - pending entries should exist
        assert!(!graph.pending_symbol_entries.is_empty());
        assert!(!graph.pending_destructure_entries.is_empty());

        graph.remove_in_range(make_range(4, 0, 6, 0));

        // Pending entries should be cleared
        assert!(graph.pending_symbol_entries.is_empty());
        assert!(graph.pending_destructure_entries.is_empty());
    }

    // =========================================================================
    // Task 2: Incremental Analysis Tests - scopes_overlapping
    // =========================================================================

    #[test]
    fn test_scopes_overlapping_basic() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(make_range(0, 0, 20, 0));

        let func_scope = graph.add_scope(Scope {
            id: ScopeId::root(),
            parent: Some(ScopeId::root()),
            range: make_range(5, 0, 15, 0),
            kind: ScopeKind::Function,
        });

        let overlapping = graph.scopes_overlapping(make_range(8, 0, 12, 0));

        assert!(overlapping.contains(&func_scope));
        assert!(overlapping.contains(&ScopeId::root())); // Root overlaps too
    }

    #[test]
    fn test_scopes_overlapping_nested() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(make_range(0, 0, 30, 0));

        let outer_scope = graph.add_scope(Scope {
            id: ScopeId::root(),
            parent: Some(ScopeId::root()),
            range: make_range(5, 0, 25, 0),
            kind: ScopeKind::Function,
        });

        let inner_scope = graph.add_scope(Scope {
            id: ScopeId::root(),
            parent: Some(outer_scope),
            range: make_range(10, 0, 20, 0),
            kind: ScopeKind::Block,
        });

        let overlapping = graph.scopes_overlapping(make_range(12, 0, 18, 0));

        // Both outer and inner should be returned
        assert!(overlapping.contains(&outer_scope));
        assert!(overlapping.contains(&inner_scope));
    }

    #[test]
    fn test_scopes_overlapping_none() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(make_range(0, 0, 10, 0));

        graph.add_scope(Scope {
            id: ScopeId::root(),
            parent: Some(ScopeId::root()),
            range: make_range(2, 0, 5, 0),
            kind: ScopeKind::Function,
        });

        // Query range outside all non-root scopes
        let overlapping = graph.scopes_overlapping(make_range(20, 0, 25, 0));

        // Only root scope (which has default range) would overlap if it were set
        // Since root has default range (0,0,0,0), it won't overlap with (20,0,25,0)
        assert!(!overlapping.iter().any(|&id| id != ScopeId::root()));
    }

    // =========================================================================
    // Task 2: Incremental Analysis Tests - document_size and is_large_edit
    // =========================================================================

    #[test]
    fn test_document_size_empty_document() {
        let graph = BindingGraph::new();
        // Default root scope has Range::default() which is (0,0,0,0)
        let (lines, chars) = graph.document_size();
        // With default range, end.line - start.line + 1 = 0 - 0 + 1 = 1
        // But the actual implementation checks for single-line: end.line == start.line
        // so chars = end.character - start.character = 0
        assert_eq!(lines, 1);
        assert_eq!(chars, 0);
    }

    #[test]
    fn test_document_size_basic() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(make_range(0, 0, 100, 50));

        let (lines, chars) = graph.document_size();
        // lines = 100 - 0 + 1 = 101
        assert_eq!(lines, 101);
        // chars = range_size = 100 * RANGE_SIZE_LINE_WEIGHT + 50
        assert_eq!(chars, 100 * RANGE_SIZE_LINE_WEIGHT + 50);
    }

    #[test]
    fn test_is_large_edit_empty_document() {
        let graph = BindingGraph::new();
        // Empty document with default range should return true (full analysis needed)
        // Actually, document_size returns (1, 0) for default range
        // But with such a small document, any edit is "large"
        let result = graph.is_large_edit(make_range(0, 0, 1, 0));
        assert!(result);
    }

    #[test]
    fn test_is_large_edit_small_edit() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(make_range(0, 0, 100, 0));

        // Edit covering 10 lines out of 101 (< 50%)
        let result = graph.is_large_edit(make_range(10, 0, 20, 0));
        assert!(!result);
    }

    #[test]
    fn test_is_large_edit_large_edit() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(make_range(0, 0, 100, 0));

        // Edit covering 60 lines out of 101 (> 50%)
        let result = graph.is_large_edit(make_range(20, 0, 80, 0));
        assert!(result);
    }

    #[test]
    fn test_is_large_edit_boundary() {
        let mut graph = BindingGraph::new();
        // Document with 100 lines (0-99)
        graph.set_root_range(make_range(0, 0, 99, 0));

        // Document has 100 lines (99 - 0 + 1)
        // 50% = 50 lines
        // Edit covering exactly 50 lines
        // edit_lines = 49 - 0 + 1 = 50
        // 50 > 100/2 = 50 is false (not strictly greater)
        let result = graph.is_large_edit(make_range(0, 0, 49, 0));
        assert!(!result);

        // Edit covering 51 lines (> 50%)
        let result = graph.is_large_edit(make_range(0, 0, 50, 0));
        assert!(result);
    }
}
