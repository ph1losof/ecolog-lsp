use crate::types::{
    EnvReference, ResolvedEnv, Scope, ScopeId, ScopeKind, Symbol, SymbolId, SymbolOrigin,
    SymbolUsage,
};
use compact_str::CompactString;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use tower_lsp::lsp_types::{Position, Range};

pub const MAX_CHAIN_DEPTH: usize = 10;

const RANGE_SIZE_LINE_WEIGHT: u64 = 10000;

#[derive(Debug, Clone, Copy)]
struct RangeIndexEntry {
    range: Range,
    symbol_id: SymbolId,
}

#[derive(Debug, Clone, Copy)]
struct UsageRangeIndexEntry {
    range: Range,
    usage_index: usize,
}

#[derive(Debug, Default, Clone)]
pub struct BindingGraph {
    symbols: Vec<Symbol>,

    scopes: Vec<Scope>,

    name_index: FxHashMap<(CompactString, ScopeId), SmallVec<[SymbolId; 2]>>,

    direct_references: Vec<EnvReference>,

    usages: Vec<SymbolUsage>,

    destructure_range_index: Vec<RangeIndexEntry>,

    /// Index for O(log n) symbol lookup by position (sorted by range start)
    symbol_range_index: Vec<RangeIndexEntry>,

    /// Index for O(log n) usage lookup by position (sorted by range start)
    usage_range_index: Vec<UsageRangeIndexEntry>,

    next_symbol_id: u32,

    next_scope_id: u32,
}

impl BindingGraph {
    pub fn new() -> Self {
        let mut graph = Self {
            symbols: Vec::new(),
            scopes: Vec::new(),
            name_index: FxHashMap::default(),
            direct_references: Vec::new(),
            usages: Vec::new(),
            destructure_range_index: Vec::new(),
            symbol_range_index: Vec::new(),
            usage_range_index: Vec::new(),
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
            .or_insert_with(SmallVec::new)
            .push(id);

        if let Some(key_range) = symbol.destructured_key_range {
            self.destructure_range_index.push(RangeIndexEntry {
                range: key_range,
                symbol_id: id,
            });
        }

        // Add to symbol range index for O(log n) position lookups
        self.symbol_range_index.push(RangeIndexEntry {
            range: symbol.name_range,
            symbol_id: id,
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

    /// O(log n) symbol lookup by position using the range index.
    /// Must call `rebuild_range_index` after batch symbol additions.
    pub fn symbol_at_position(&self, position: Position) -> Option<&Symbol> {
        self.binary_search_range_index(&self.symbol_range_index, position)
            .and_then(|id| self.get_symbol(id))
    }

    /// Rebuilds all range indices for O(log n) position lookups.
    /// Must be called after batch additions of symbols, usages, or destructured keys.
    pub fn rebuild_range_index(&mut self) {
        let sort_fn = |a: &RangeIndexEntry, b: &RangeIndexEntry| {
            a.range
                .start
                .line
                .cmp(&b.range.start.line)
                .then_with(|| a.range.start.character.cmp(&b.range.start.character))
        };

        self.destructure_range_index.sort_by(sort_fn);
        self.symbol_range_index.sort_by(sort_fn);

        self.usage_range_index.sort_by(|a, b| {
            a.range
                .start
                .line
                .cmp(&b.range.start.line)
                .then_with(|| a.range.start.character.cmp(&b.range.start.character))
        });
    }

    /// Generic binary search for range indices, returns SymbolId if found.
    fn binary_search_range_index(
        &self,
        index: &[RangeIndexEntry],
        position: Position,
    ) -> Option<SymbolId> {
        if index.is_empty() {
            return None;
        }

        let mut left = 0;
        let mut right = index.len();
        let mut found_idx = None;

        while left < right {
            let mid = left + (right - left) / 2;
            let entry = &index[mid];

            if Self::contains_position(entry.range, position) {
                return Some(entry.symbol_id);
            }

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

        // Check nearby entries for overlapping ranges
        if let Some(idx) = found_idx {
            for offset in 0..3 {
                if let Some(i) = idx.checked_sub(offset) {
                    if let Some(entry) = index.get(i) {
                        if Self::contains_position(entry.range, position) {
                            return Some(entry.symbol_id);
                        }
                    }
                }
                let i = idx + offset + 1;
                if let Some(entry) = index.get(i) {
                    if Self::contains_position(entry.range, position) {
                        return Some(entry.symbol_id);
                    }
                }
            }
        }

        None
    }

    pub fn symbol_at_destructure_key(&self, position: Position) -> Option<SymbolId> {
        self.binary_search_range_index(&self.destructure_range_index, position)
    }

    pub fn add_scope(&mut self, mut scope: Scope) -> ScopeId {
        let id =
            ScopeId::new(self.next_scope_id).expect("Scope ID counter overflow - too many scopes");
        self.next_scope_id += 1;
        scope.id = id;

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
        // Add to usage range index for O(log n) position lookups
        self.usage_range_index.push(UsageRangeIndexEntry {
            range: usage.range,
            usage_index,
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

    /// O(log n) usage lookup by position using the range index.
    /// Must call `rebuild_range_index` after batch usage additions.
    pub fn usage_at_position(&self, position: Position) -> Option<&SymbolUsage> {
        self.binary_search_usage_range_index(position)
            .and_then(|idx| self.usages.get(idx))
    }

    /// Binary search for usage range index, returns usage index if found.
    fn binary_search_usage_range_index(&self, position: Position) -> Option<usize> {
        if self.usage_range_index.is_empty() {
            return None;
        }

        let mut left = 0;
        let mut right = self.usage_range_index.len();
        let mut found_idx = None;

        while left < right {
            let mid = left + (right - left) / 2;
            let entry = &self.usage_range_index[mid];

            if Self::contains_position(entry.range, position) {
                return Some(entry.usage_index);
            }

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

        // Check nearby entries for overlapping ranges
        if let Some(idx) = found_idx {
            for offset in 0..3 {
                if let Some(i) = idx.checked_sub(offset) {
                    if let Some(entry) = self.usage_range_index.get(i) {
                        if Self::contains_position(entry.range, position) {
                            return Some(entry.usage_index);
                        }
                    }
                }
                let i = idx + offset + 1;
                if let Some(entry) = self.usage_range_index.get(i) {
                    if Self::contains_position(entry.range, position) {
                        return Some(entry.usage_index);
                    }
                }
            }
        }

        None
    }

    pub fn resolve_to_env(&self, symbol_id: SymbolId) -> Option<ResolvedEnv> {
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

    pub fn clear(&mut self) {
        self.symbols.clear();
        self.scopes.clear();
        self.name_index.clear();
        self.direct_references.clear();
        self.usages.clear();
        self.destructure_range_index.clear();
        self.symbol_range_index.clear();
        self.usage_range_index.clear();
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
}
