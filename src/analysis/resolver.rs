use crate::analysis::binding_graph::BindingGraph;
use crate::types::{
    BindingKind, EnvBinding, EnvBindingUsage, EnvReference, ResolvedEnv, ScopeId, Symbol, SymbolId,
    SymbolUsage,
};
use compact_str::CompactString;
use tower_lsp::lsp_types::{Position, Range};
use tracing::error;

#[derive(Debug, Clone)]
pub enum EnvHit<'a> {
    DirectReference(&'a EnvReference),

    ViaSymbol {
        symbol: &'a Symbol,
        resolved: ResolvedEnv,
    },

    ViaUsage {
        usage: &'a SymbolUsage,
        symbol: &'a Symbol,
        resolved: ResolvedEnv,
    },
}

impl<'a> EnvHit<'a> {
    pub fn env_var_name(&self) -> Option<CompactString> {
        match self {
            EnvHit::DirectReference(r) => Some(r.name.clone()),
            EnvHit::ViaSymbol { resolved, .. } | EnvHit::ViaUsage { resolved, .. } => {
                match resolved {
                    ResolvedEnv::Variable(name) => Some(name.clone()),
                    ResolvedEnv::Object(_) => None,
                }
            }
        }
    }

    pub fn canonical_name(&self) -> CompactString {
        match self {
            EnvHit::DirectReference(r) => r.name.clone(),
            EnvHit::ViaSymbol { resolved, .. } | EnvHit::ViaUsage { resolved, .. } => {
                match resolved {
                    ResolvedEnv::Variable(name) => name.clone(),
                    ResolvedEnv::Object(name) => name.clone(),
                }
            }
        }
    }

    pub fn range(&self) -> Range {
        match self {
            EnvHit::DirectReference(r) => r.name_range,
            EnvHit::ViaSymbol { symbol, .. } => symbol.name_range,
            EnvHit::ViaUsage { usage, .. } => usage.range,
        }
    }

    pub fn is_env_object(&self) -> bool {
        match self {
            EnvHit::DirectReference(_) => false,
            EnvHit::ViaSymbol { resolved, .. } | EnvHit::ViaUsage { resolved, .. } => {
                matches!(resolved, ResolvedEnv::Object(_))
            }
        }
    }

    pub fn binding_name(&self) -> Option<&CompactString> {
        match self {
            EnvHit::DirectReference(_) => None,
            EnvHit::ViaSymbol { symbol, .. } => Some(&symbol.name),
            EnvHit::ViaUsage { symbol, .. } => Some(&symbol.name),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedBinding {
    pub binding_name: CompactString,

    pub env_var_name: CompactString,

    pub binding_range: Range,

    pub declaration_range: Range,

    pub kind: BindingKind,

    pub is_usage: bool,
}

impl ResolvedBinding {
    pub fn to_env_binding(&self, scope_range: Range) -> EnvBinding {
        EnvBinding {
            binding_name: self.binding_name.clone(),
            env_var_name: self.env_var_name.clone(),
            binding_range: self.binding_range,
            declaration_range: self.declaration_range,
            scope_range,
            is_valid: true,
            kind: self.kind.clone(),
            destructured_key_range: None,
        }
    }

    pub fn to_binding_usage(&self) -> EnvBindingUsage {
        EnvBindingUsage {
            name: self.binding_name.clone(),
            range: self.binding_range,
            declaration_range: self.declaration_range,
            env_var_name: self.env_var_name.clone(),
        }
    }
}

pub struct BindingResolver<'a> {
    graph: &'a BindingGraph,
}

impl<'a> BindingResolver<'a> {
    pub fn new(graph: &'a BindingGraph) -> Self {
        Self { graph }
    }

    pub fn env_at_position(&self, position: Position) -> Option<EnvHit<'a>> {
        tracing::trace!(
            "Looking up env at position line={}, char={}",
            position.line,
            position.character
        );

        for reference in self.graph.direct_references() {
            if BindingGraph::contains_position(reference.name_range, position) {
                return Some(EnvHit::DirectReference(reference));
            }
        }

        if let Some(symbol) = self.graph.symbol_at_position(position) {
            if let Some(resolved) = self.graph.resolve_to_env(symbol.id) {
                return Some(EnvHit::ViaSymbol { symbol, resolved });
            }
        }

        if let Some(symbol_id) = self.graph.symbol_at_destructure_key(position) {
            if let Some(symbol) = self.graph.get_symbol(symbol_id) {
                if let Some(resolved) = self.graph.resolve_to_env(symbol_id) {
                    return Some(EnvHit::ViaSymbol { symbol, resolved });
                }
            }
        }

        if let Some(usage) = self.graph.usage_at_position(position) {
            if let Some(symbol) = self.graph.get_symbol(usage.symbol_id) {
                if let Some(property) = &usage.property_access {
                    if let Some(resolved) = self.graph.resolve_to_env(usage.symbol_id) {
                        if matches!(resolved, ResolvedEnv::Object(_)) {
                            return Some(EnvHit::ViaUsage {
                                usage,
                                symbol,
                                resolved: ResolvedEnv::Variable(property.clone()),
                            });
                        }
                    }
                } else if let Some(resolved) = self.graph.resolve_to_env(usage.symbol_id) {
                    return Some(EnvHit::ViaUsage {
                        usage,
                        symbol,
                        resolved,
                    });
                }
            }
        }

        tracing::debug!(
            "No env var found at position line={}, char={}",
            position.line,
            position.character
        );
        None
    }

    pub fn binding_at_position(&self, position: Position) -> Option<ResolvedBinding> {
        let hit = self.env_at_position(position)?;

        match hit {
            EnvHit::DirectReference(_) => None,

            EnvHit::ViaSymbol { symbol, resolved } => {
                let env_var_name = match &resolved {
                    ResolvedEnv::Variable(name) => name.clone(),
                    ResolvedEnv::Object(name) => name.clone(),
                };

                let kind = match &resolved {
                    ResolvedEnv::Variable(_) => BindingKind::Value,
                    ResolvedEnv::Object(_) => BindingKind::Object,
                };

                Some(ResolvedBinding {
                    binding_name: symbol.name.clone(),
                    env_var_name,
                    binding_range: symbol.name_range,
                    declaration_range: symbol.declaration_range,
                    kind,
                    is_usage: false,
                })
            }

            EnvHit::ViaUsage {
                usage,
                symbol,
                resolved,
            } => {
                let env_var_name = match &resolved {
                    ResolvedEnv::Variable(name) => name.clone(),
                    ResolvedEnv::Object(name) => name.clone(),
                };

                let kind = match &resolved {
                    ResolvedEnv::Variable(_) => BindingKind::Value,
                    ResolvedEnv::Object(_) => BindingKind::Object,
                };

                Some(ResolvedBinding {
                    binding_name: symbol.name.clone(),
                    env_var_name,
                    binding_range: usage.range,
                    declaration_range: symbol.declaration_range,
                    kind,
                    is_usage: true,
                })
            }
        }
    }

    pub fn direct_reference_at_position(&self, position: Position) -> Option<&'a EnvReference> {
        for reference in self.graph.direct_references() {
            if BindingGraph::contains_position(reference.name_range, position) {
                return Some(reference);
            }
        }
        None
    }

    pub fn find_env_var_usages(&self, env_var_name: &str) -> Vec<EnvVarUsageLocation> {
        let mut locations = Vec::new();
        let mut seen_ranges = std::collections::HashSet::new();

        for reference in self.graph.direct_references() {
            if reference.name == env_var_name {
                let range_key = (
                    reference.name_range.start.line,
                    reference.name_range.start.character,
                    reference.name_range.end.line,
                    reference.name_range.end.character,
                );
                if seen_ranges.insert(range_key) {
                    locations.push(EnvVarUsageLocation {
                        range: reference.name_range,
                        kind: UsageKind::DirectReference,
                        binding_name: None,
                    });
                }
            }
        }

        for symbol in self.graph.symbols() {
            if let Some(resolved) = self.graph.resolve_to_env(symbol.id) {
                if let ResolvedEnv::Variable(name) = &resolved {
                    if name == env_var_name {
                        let rename_range = if let Some(key_range) = symbol.destructured_key_range {
                            Some(key_range)
                        } else if symbol.name.as_str() == env_var_name {
                            Some(symbol.name_range)
                        } else {
                            None
                        };

                        if let Some(range) = rename_range {
                            let range_key = (
                                range.start.line,
                                range.start.character,
                                range.end.line,
                                range.end.character,
                            );
                            if seen_ranges.insert(range_key) {
                                locations.push(EnvVarUsageLocation {
                                    range,
                                    kind: UsageKind::BindingDeclaration,
                                    binding_name: Some(symbol.name.clone()),
                                });
                            }
                        }
                    }
                }
            }
        }

        for usage in self.graph.usages() {
            if let Some(resolved) = self.graph.resolve_to_env(usage.symbol_id) {
                match &resolved {
                    ResolvedEnv::Variable(name) if name == env_var_name => {
                        let range_key = (
                            usage.range.start.line,
                            usage.range.start.character,
                            usage.range.end.line,
                            usage.range.end.character,
                        );
                        if seen_ranges.insert(range_key) {
                            let binding_name = self
                                .graph
                                .get_symbol(usage.symbol_id)
                                .map(|s| s.name.clone());
                            locations.push(EnvVarUsageLocation {
                                range: usage.range,
                                kind: UsageKind::BindingUsage,
                                binding_name,
                            });
                        }
                    }
                    ResolvedEnv::Object(_) => {
                        if let Some(prop) = &usage.property_access {
                            if prop == env_var_name {
                                let range = usage.property_access_range.unwrap_or(usage.range);

                                let range_key = (
                                    range.start.line,
                                    range.start.character,
                                    range.end.line,
                                    range.end.character,
                                );
                                if seen_ranges.insert(range_key) {
                                    let binding_name = self
                                        .graph
                                        .get_symbol(usage.symbol_id)
                                        .map(|s| s.name.clone());
                                    locations.push(EnvVarUsageLocation {
                                        range,
                                        kind: UsageKind::PropertyAccess,
                                        binding_name,
                                    });
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        locations
    }

    pub fn all_env_vars(&self) -> Vec<CompactString> {
        let mut vars = std::collections::HashSet::new();

        for reference in self.graph.direct_references() {
            vars.insert(reference.name.clone());
        }

        for symbol in self.graph.symbols() {
            if let Some(ResolvedEnv::Variable(name)) = self.graph.resolve_to_env(symbol.id) {
                vars.insert(name);
            }
        }

        vars.into_iter().collect()
    }

    pub fn get_symbol(&self, id: SymbolId) -> Option<&'a Symbol> {
        self.graph.get_symbol(id)
    }

    pub fn lookup_symbol(&self, name: &str, scope: ScopeId) -> Option<&'a Symbol> {
        self.graph.lookup_symbol(name, scope)
    }

    pub fn scope_at_position(&self, position: Position) -> ScopeId {
        self.graph.scope_at_position(position)
    }

    pub fn is_env_object(&self, symbol_id: SymbolId) -> bool {
        self.graph.resolves_to_env_object(symbol_id)
    }

    pub fn get_env_reference_cloned(&self, position: Position) -> Option<EnvReference> {
        if let Some(reference) = self.direct_reference_at_position(position) {
            return Some(reference.clone());
        }

        if let Some(usage) = self.graph.usage_at_position(position) {
            if let Some(property) = &usage.property_access {
                if let Some(resolved) = self.graph.resolve_to_env(usage.symbol_id) {
                    if matches!(resolved, ResolvedEnv::Object(_)) {
                        return Some(EnvReference {
                            name: property.clone(),
                            full_range: usage.range,
                            name_range: usage.range,
                            access_type: crate::types::AccessType::Property,
                            has_default: false,
                            default_value: None,
                        });
                    }
                }
            }
        }

        None
    }

    pub fn get_env_binding_cloned(&self, position: Position) -> Option<EnvBinding> {
        let hit = self.env_at_position(position)?;

        match hit {
            EnvHit::DirectReference(_) => None,
            EnvHit::ViaUsage { .. } => None,

            EnvHit::ViaSymbol { symbol, resolved } => {
                let env_var_name = match &resolved {
                    ResolvedEnv::Variable(name) => name.clone(),
                    ResolvedEnv::Object(name) => name.clone(),
                };

                let kind = match &resolved {
                    ResolvedEnv::Variable(_) => BindingKind::Value,
                    ResolvedEnv::Object(_) => BindingKind::Object,
                };

                let scope = match self.graph.get_scope(symbol.scope) {
                    Some(s) => s,
                    None => {
                        debug_assert!(
                            false,
                            "Symbol '{}' references non-existent scope {:?} - data consistency error",
                            symbol.name, symbol.scope
                        );
                        error!(
                            symbol_name = %symbol.name,
                            scope_id = ?symbol.scope,
                            "Symbol references non-existent scope - data consistency error"
                        );
                        return None;
                    }
                };

                Some(EnvBinding {
                    binding_name: symbol.name.clone(),
                    env_var_name,
                    binding_range: symbol.name_range,
                    declaration_range: symbol.declaration_range,
                    scope_range: scope.range,
                    is_valid: symbol.is_valid,
                    kind,
                    destructured_key_range: symbol.destructured_key_range,
                })
            }
        }
    }

    pub fn get_binding_usage_cloned(&self, position: Position) -> Option<EnvBindingUsage> {
        let binding = self.binding_at_position(position)?;
        if !binding.is_usage {
            return None;
        }
        Some(binding.to_binding_usage())
    }

    pub fn get_binding_kind(&self, name: &str) -> Option<BindingKind> {
        for symbol in self.graph.symbols() {
            if symbol.name == name && symbol.is_valid {
                if let Some(resolved) = self.graph.resolve_to_env(symbol.id) {
                    return Some(match resolved {
                        ResolvedEnv::Variable(_) => BindingKind::Value,
                        ResolvedEnv::Object(_) => BindingKind::Object,
                    });
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct EnvVarUsageLocation {
    pub range: Range,

    pub kind: UsageKind,

    pub binding_name: Option<CompactString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageKind {
    DirectReference,

    BindingDeclaration,

    BindingUsage,

    PropertyAccess,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AccessType, ScopeKind, SymbolKind, SymbolOrigin};

    fn make_range(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Range {
        Range::new(
            Position::new(start_line, start_char),
            Position::new(end_line, end_char),
        )
    }

    #[test]
    fn test_env_hit_direct_reference_methods() {
        let mut graph = BindingGraph::new();

        let reference = EnvReference {
            name: "DATABASE_URL".into(),
            full_range: make_range(0, 0, 0, 30),
            name_range: make_range(0, 12, 0, 24),
            access_type: AccessType::Property,
            has_default: false,
            default_value: None,
        };
        graph.add_direct_reference(reference);

        let resolver = BindingResolver::new(&graph);
        let hit = resolver.env_at_position(Position::new(0, 15)).unwrap();

        assert_eq!(hit.env_var_name(), Some("DATABASE_URL".into()));
        assert_eq!(hit.canonical_name().as_str(), "DATABASE_URL");
        assert_eq!(hit.range(), make_range(0, 12, 0, 24));
        assert!(!hit.is_env_object());
        assert!(hit.binding_name().is_none());
    }

    #[test]
    fn test_env_hit_via_symbol_env_var() {
        let mut graph = BindingGraph::new();

        let _id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "dbUrl".into(),
            declaration_range: make_range(0, 0, 0, 40),
            name_range: make_range(0, 6, 0, 11),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "DATABASE_URL".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let resolver = BindingResolver::new(&graph);
        let hit = resolver.env_at_position(Position::new(0, 8)).unwrap();

        assert_eq!(hit.env_var_name(), Some("DATABASE_URL".into()));
        assert_eq!(hit.canonical_name().as_str(), "DATABASE_URL");
        assert_eq!(hit.range(), make_range(0, 6, 0, 11));
        assert!(!hit.is_env_object());
        assert_eq!(hit.binding_name(), Some(&"dbUrl".into()));
    }

    #[test]
    fn test_env_hit_via_symbol_env_object() {
        let mut graph = BindingGraph::new();

        let _id = graph.add_symbol(Symbol {
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

        let resolver = BindingResolver::new(&graph);
        let hit = resolver.env_at_position(Position::new(0, 7)).unwrap();

        assert!(hit.env_var_name().is_none());
        assert_eq!(hit.canonical_name().as_str(), "process.env");
        assert!(hit.is_env_object());
        assert_eq!(hit.binding_name(), Some(&"env".into()));
    }

    #[test]
    fn test_env_hit_via_usage() {
        let mut graph = BindingGraph::new();

        let id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "apiKey".into(),
            declaration_range: make_range(0, 0, 0, 35),
            name_range: make_range(0, 6, 0, 12),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "API_KEY".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        graph.add_usage(SymbolUsage {
            symbol_id: id,
            range: make_range(2, 10, 2, 16),
            scope: ScopeId::root(),
            property_access: None,
            property_access_range: None,
        });

        let resolver = BindingResolver::new(&graph);
        let hit = resolver.env_at_position(Position::new(2, 12)).unwrap();

        assert!(matches!(hit, EnvHit::ViaUsage { .. }));
        assert_eq!(hit.env_var_name(), Some("API_KEY".into()));
        assert_eq!(hit.canonical_name().as_str(), "API_KEY");
        assert_eq!(hit.range(), make_range(2, 10, 2, 16));
        assert!(!hit.is_env_object());
        assert_eq!(hit.binding_name(), Some(&"apiKey".into()));
    }

    #[test]
    fn test_env_hit_via_usage_property_access() {
        let mut graph = BindingGraph::new();

        let id = graph.add_symbol(Symbol {
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
            symbol_id: id,
            range: make_range(1, 0, 1, 16),
            scope: ScopeId::root(),
            property_access: Some("DATABASE_URL".into()),
            property_access_range: Some(make_range(1, 4, 1, 16)),
        });

        let resolver = BindingResolver::new(&graph);
        let hit = resolver.env_at_position(Position::new(1, 8)).unwrap();

        assert!(matches!(hit, EnvHit::ViaUsage { .. }));

        assert_eq!(hit.env_var_name(), Some("DATABASE_URL".into()));
        assert!(!hit.is_env_object());
    }

    #[test]
    fn test_resolved_binding_to_env_binding() {
        let binding = ResolvedBinding {
            binding_name: "dbUrl".into(),
            env_var_name: "DATABASE_URL".into(),
            binding_range: make_range(0, 6, 0, 11),
            declaration_range: make_range(0, 0, 0, 40),
            kind: BindingKind::Value,
            is_usage: false,
        };

        let scope_range = make_range(0, 0, 10, 0);
        let env_binding = binding.to_env_binding(scope_range);

        assert_eq!(env_binding.binding_name, "dbUrl");
        assert_eq!(env_binding.env_var_name, "DATABASE_URL");
        assert_eq!(env_binding.binding_range, make_range(0, 6, 0, 11));
        assert_eq!(env_binding.declaration_range, make_range(0, 0, 0, 40));
        assert_eq!(env_binding.scope_range, scope_range);
        assert!(env_binding.is_valid);
        assert_eq!(env_binding.kind, BindingKind::Value);
        assert!(env_binding.destructured_key_range.is_none());
    }

    #[test]
    fn test_resolved_binding_to_binding_usage() {
        let binding = ResolvedBinding {
            binding_name: "apiKey".into(),
            env_var_name: "API_KEY".into(),
            binding_range: make_range(2, 10, 2, 16),
            declaration_range: make_range(0, 0, 0, 35),
            kind: BindingKind::Value,
            is_usage: true,
        };

        let usage = binding.to_binding_usage();

        assert_eq!(usage.name, "apiKey");
        assert_eq!(usage.env_var_name, "API_KEY");
        assert_eq!(usage.range, make_range(2, 10, 2, 16));
        assert_eq!(usage.declaration_range, make_range(0, 0, 0, 35));
    }

    #[test]
    fn test_resolved_binding_object_kind() {
        let binding = ResolvedBinding {
            binding_name: "env".into(),
            env_var_name: "process.env".into(),
            binding_range: make_range(0, 6, 0, 9),
            declaration_range: make_range(0, 0, 0, 25),
            kind: BindingKind::Object,
            is_usage: false,
        };

        let env_binding = binding.to_env_binding(make_range(0, 0, 100, 0));
        assert_eq!(env_binding.kind, BindingKind::Object);
    }

    #[test]
    fn test_resolve_direct_reference() {
        let mut graph = BindingGraph::new();

        graph.add_direct_reference(EnvReference {
            name: "DATABASE_URL".into(),
            full_range: make_range(0, 0, 0, 30),
            name_range: make_range(0, 12, 0, 24),
            access_type: crate::types::AccessType::Property,
            has_default: false,
            default_value: None,
        });

        let resolver = BindingResolver::new(&graph);

        let hit = resolver.env_at_position(Position::new(0, 15)).unwrap();
        assert!(matches!(hit, EnvHit::DirectReference(_)));
        assert_eq!(hit.env_var_name(), Some("DATABASE_URL".into()));
    }

    #[test]
    fn test_resolve_via_symbol() {
        let mut graph = BindingGraph::new();

        let _id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "dbUrl".into(),
            declaration_range: make_range(0, 0, 0, 40),
            name_range: make_range(0, 6, 0, 11),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "DATABASE_URL".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let resolver = BindingResolver::new(&graph);

        let hit = resolver.env_at_position(Position::new(0, 8)).unwrap();
        assert!(matches!(hit, EnvHit::ViaSymbol { .. }));
        assert_eq!(hit.env_var_name(), Some("DATABASE_URL".into()));
        assert_eq!(hit.binding_name(), Some(&"dbUrl".into()));
    }

    #[test]
    fn test_resolve_chain() {
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

        let _db_url_id = graph.add_symbol(Symbol {
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

        let resolver = BindingResolver::new(&graph);

        let hit = resolver.env_at_position(Position::new(2, 10)).unwrap();
        assert!(matches!(hit, EnvHit::ViaSymbol { .. }));
        assert_eq!(hit.env_var_name(), Some("DB_URL".into()));
    }

    #[test]
    fn test_binding_at_position_symbol() {
        let mut graph = BindingGraph::new();

        let _id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "dbUrl".into(),
            declaration_range: make_range(0, 0, 0, 40),
            name_range: make_range(0, 6, 0, 11),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "DATABASE_URL".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let resolver = BindingResolver::new(&graph);
        let binding = resolver.binding_at_position(Position::new(0, 8)).unwrap();

        assert_eq!(binding.binding_name, "dbUrl");
        assert_eq!(binding.env_var_name, "DATABASE_URL");
        assert!(!binding.is_usage);
        assert_eq!(binding.kind, BindingKind::Value);
    }

    #[test]
    fn test_binding_at_position_usage() {
        let mut graph = BindingGraph::new();

        let id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "apiKey".into(),
            declaration_range: make_range(0, 0, 0, 35),
            name_range: make_range(0, 6, 0, 12),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "API_KEY".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        graph.add_usage(SymbolUsage {
            symbol_id: id,
            range: make_range(2, 10, 2, 16),
            scope: ScopeId::root(),
            property_access: None,
            property_access_range: None,
        });

        let resolver = BindingResolver::new(&graph);
        let binding = resolver.binding_at_position(Position::new(2, 12)).unwrap();

        assert_eq!(binding.binding_name, "apiKey");
        assert_eq!(binding.env_var_name, "API_KEY");
        assert!(binding.is_usage);
    }

    #[test]
    fn test_binding_at_position_direct_reference_returns_none() {
        let mut graph = BindingGraph::new();

        graph.add_direct_reference(EnvReference {
            name: "DATABASE_URL".into(),
            full_range: make_range(0, 0, 0, 30),
            name_range: make_range(0, 12, 0, 24),
            access_type: AccessType::Property,
            has_default: false,
            default_value: None,
        });

        let resolver = BindingResolver::new(&graph);

        assert!(resolver.binding_at_position(Position::new(0, 15)).is_none());
    }

    #[test]
    fn test_direct_reference_at_position() {
        let mut graph = BindingGraph::new();

        graph.add_direct_reference(EnvReference {
            name: "API_KEY".into(),
            full_range: make_range(0, 0, 0, 25),
            name_range: make_range(0, 12, 0, 19),
            access_type: AccessType::Property,
            has_default: false,
            default_value: None,
        });

        let resolver = BindingResolver::new(&graph);

        let ref_at_full_only = resolver.direct_reference_at_position(Position::new(0, 5));
        assert!(
            ref_at_full_only.is_none(),
            "Position in full_range but outside name_range should return None"
        );

        let ref_at_name = resolver.direct_reference_at_position(Position::new(0, 15));
        assert!(ref_at_name.is_some());
        assert_eq!(ref_at_name.unwrap().name, "API_KEY");

        let ref_outside = resolver.direct_reference_at_position(Position::new(1, 0));
        assert!(ref_outside.is_none());
    }

    #[test]
    fn test_env_at_position_outside_any_env_returns_none() {
        let graph = BindingGraph::new();
        let resolver = BindingResolver::new(&graph);

        assert!(resolver.env_at_position(Position::new(0, 0)).is_none());
        assert!(resolver.env_at_position(Position::new(100, 100)).is_none());
    }

    #[test]
    fn test_env_at_position_with_destructure_key() {
        let mut graph = BindingGraph::new();

        let _id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "apiKey".into(),
            declaration_range: make_range(0, 0, 0, 40),
            name_range: make_range(0, 18, 0, 24),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "API_KEY".into(),
            },
            kind: SymbolKind::DestructuredProperty,
            is_valid: true,
            destructured_key_range: Some(make_range(0, 8, 0, 15)),
        });

        let resolver = BindingResolver::new(&graph);

        let hit = resolver.env_at_position(Position::new(0, 10));
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert_eq!(hit.env_var_name(), Some("API_KEY".into()));

        let hit2 = resolver.env_at_position(Position::new(0, 20));
        assert!(hit2.is_some());
    }

    #[test]
    fn test_find_usages() {
        let mut graph = BindingGraph::new();

        graph.add_direct_reference(EnvReference {
            name: "API_KEY".into(),
            full_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 12, 0, 19),
            access_type: crate::types::AccessType::Property,
            has_default: false,
            default_value: None,
        });

        let id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "apiKey".into(),
            declaration_range: make_range(1, 0, 1, 35),
            name_range: make_range(1, 6, 1, 12),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "API_KEY".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        graph.add_usage(SymbolUsage {
            symbol_id: id,
            range: make_range(2, 10, 2, 16),
            scope: ScopeId::root(),
            property_access: None,
            property_access_range: None,
        });

        let resolver = BindingResolver::new(&graph);

        let usages = resolver.find_env_var_usages("API_KEY");

        assert_eq!(usages.len(), 2);

        assert!(usages.iter().any(|u| u.kind == UsageKind::DirectReference));
        assert!(usages.iter().any(|u| u.kind == UsageKind::BindingUsage));
    }

    #[test]
    fn test_find_usages_destructured() {
        let mut graph = BindingGraph::new();

        graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "API_KEY".into(),
            declaration_range: make_range(0, 0, 0, 35),
            name_range: make_range(0, 8, 0, 15),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "API_KEY".into(),
            },
            kind: SymbolKind::DestructuredProperty,
            is_valid: true,
            destructured_key_range: None,
        });

        let resolver = BindingResolver::new(&graph);

        let usages = resolver.find_env_var_usages("API_KEY");

        assert_eq!(usages.len(), 1);
        assert!(usages
            .iter()
            .any(|u| u.kind == UsageKind::BindingDeclaration));
    }

    #[test]
    fn test_find_usages_destructured_with_rename() {
        let mut graph = BindingGraph::new();

        graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "apiKey".into(),
            declaration_range: make_range(0, 0, 0, 45),
            name_range: make_range(0, 18, 0, 24),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "API_KEY".into(),
            },
            kind: SymbolKind::DestructuredProperty,
            is_valid: true,
            destructured_key_range: Some(make_range(0, 8, 0, 15)),
        });

        let resolver = BindingResolver::new(&graph);

        let usages = resolver.find_env_var_usages("API_KEY");

        assert_eq!(usages.len(), 1);
        assert!(usages
            .iter()
            .any(|u| u.kind == UsageKind::BindingDeclaration));

        assert_eq!(usages[0].range, make_range(0, 8, 0, 15));
    }

    #[test]
    fn test_find_usages_property_access() {
        let mut graph = BindingGraph::new();

        let id = graph.add_symbol(Symbol {
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
            symbol_id: id,
            range: make_range(1, 0, 1, 16),
            scope: ScopeId::root(),
            property_access: Some("DATABASE_URL".into()),
            property_access_range: Some(make_range(1, 4, 1, 16)),
        });

        let resolver = BindingResolver::new(&graph);
        let usages = resolver.find_env_var_usages("DATABASE_URL");

        assert_eq!(usages.len(), 1);
        assert!(usages.iter().any(|u| u.kind == UsageKind::PropertyAccess));

        assert_eq!(usages[0].range, make_range(1, 4, 1, 16));
    }

    #[test]
    fn test_find_usages_property_access_no_range() {
        let mut graph = BindingGraph::new();

        let id = graph.add_symbol(Symbol {
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
            symbol_id: id,
            range: make_range(2, 5, 2, 16),
            scope: ScopeId::root(),
            property_access: Some("API_KEY".into()),
            property_access_range: None,
        });

        let resolver = BindingResolver::new(&graph);
        let usages = resolver.find_env_var_usages("API_KEY");

        assert_eq!(usages.len(), 1);

        assert_eq!(usages[0].range, make_range(2, 5, 2, 16));
    }

    #[test]
    fn test_find_usages_deduplication() {
        let mut graph = BindingGraph::new();

        graph.add_direct_reference(EnvReference {
            name: "DUPLICATE".into(),
            full_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 12, 0, 20),
            access_type: AccessType::Property,
            has_default: false,
            default_value: None,
        });
        graph.add_direct_reference(EnvReference {
            name: "DUPLICATE".into(),
            full_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 12, 0, 20),
            access_type: AccessType::Property,
            has_default: false,
            default_value: None,
        });

        let resolver = BindingResolver::new(&graph);
        let usages = resolver.find_env_var_usages("DUPLICATE");

        assert_eq!(usages.len(), 1);
    }

    #[test]
    fn test_find_usages_nonexistent_env_var() {
        let graph = BindingGraph::new();
        let resolver = BindingResolver::new(&graph);

        let usages = resolver.find_env_var_usages("NONEXISTENT");
        assert!(usages.is_empty());
    }

    #[test]
    fn test_all_env_vars() {
        let mut graph = BindingGraph::new();

        graph.add_direct_reference(EnvReference {
            name: "VAR_ONE".into(),
            full_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 12, 0, 19),
            access_type: AccessType::Property,
            has_default: false,
            default_value: None,
        });

        graph.add_direct_reference(EnvReference {
            name: "VAR_TWO".into(),
            full_range: make_range(1, 0, 1, 20),
            name_range: make_range(1, 12, 1, 19),
            access_type: AccessType::Property,
            has_default: false,
            default_value: None,
        });

        let _id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "varThree".into(),
            declaration_range: make_range(2, 0, 2, 35),
            name_range: make_range(2, 6, 2, 14),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "VAR_THREE".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let resolver = BindingResolver::new(&graph);
        let all_vars = resolver.all_env_vars();

        assert_eq!(all_vars.len(), 3);
        assert!(all_vars.iter().any(|v| v == "VAR_ONE"));
        assert!(all_vars.iter().any(|v| v == "VAR_TWO"));
        assert!(all_vars.iter().any(|v| v == "VAR_THREE"));
    }

    #[test]
    fn test_all_env_vars_empty() {
        let graph = BindingGraph::new();
        let resolver = BindingResolver::new(&graph);

        let all_vars = resolver.all_env_vars();
        assert!(all_vars.is_empty());
    }

    #[test]
    fn test_all_env_vars_excludes_objects() {
        let mut graph = BindingGraph::new();

        let _id = graph.add_symbol(Symbol {
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

        let resolver = BindingResolver::new(&graph);
        let all_vars = resolver.all_env_vars();

        assert!(all_vars.is_empty());
    }

    #[test]
    fn test_get_symbol() {
        let mut graph = BindingGraph::new();

        let id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "testVar".into(),
            declaration_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 6, 0, 13),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "TEST".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let resolver = BindingResolver::new(&graph);

        let symbol = resolver.get_symbol(id);
        assert!(symbol.is_some());
        assert_eq!(symbol.unwrap().name, "testVar");

        let fake_id = SymbolId::new(999).unwrap();
        assert!(resolver.get_symbol(fake_id).is_none());
    }

    #[test]
    fn test_lookup_symbol() {
        let mut graph = BindingGraph::new();

        let _id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "myVar".into(),
            declaration_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 6, 0, 11),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "MY_VAR".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let resolver = BindingResolver::new(&graph);

        let symbol = resolver.lookup_symbol("myVar", ScopeId::root());
        assert!(symbol.is_some());
        assert_eq!(symbol.unwrap().name, "myVar");

        let not_found = resolver.lookup_symbol("nonexistent", ScopeId::root());
        assert!(not_found.is_none());
    }

    #[test]
    fn test_scope_at_position() {
        let graph = BindingGraph::new();
        let resolver = BindingResolver::new(&graph);

        let scope = resolver.scope_at_position(Position::new(5, 10));
        assert_eq!(scope, ScopeId::root());
    }

    #[test]
    fn test_is_env_object() {
        let mut graph = BindingGraph::new();

        let env_obj_id = graph.add_symbol(Symbol {
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
            name: "dbUrl".into(),
            declaration_range: make_range(1, 0, 1, 35),
            name_range: make_range(1, 6, 1, 11),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "DATABASE_URL".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let resolver = BindingResolver::new(&graph);

        assert!(resolver.is_env_object(env_obj_id));
        assert!(!resolver.is_env_object(var_id));
    }

    #[test]
    fn test_get_env_reference_cloned() {
        let mut graph = BindingGraph::new();

        graph.add_direct_reference(EnvReference {
            name: "TEST_VAR".into(),
            full_range: make_range(0, 0, 0, 25),
            name_range: make_range(0, 12, 0, 20),
            access_type: AccessType::Property,
            has_default: true,
            default_value: Some("default".into()),
        });

        let resolver = BindingResolver::new(&graph);

        let cloned = resolver.get_env_reference_cloned(Position::new(0, 15));
        assert!(cloned.is_some());
        let cloned = cloned.unwrap();
        assert_eq!(cloned.name, "TEST_VAR");
        assert!(cloned.has_default);
        assert_eq!(cloned.default_value, Some("default".into()));
    }

    #[test]
    fn test_get_env_reference_cloned_from_property_access() {
        let mut graph = BindingGraph::new();

        let id = graph.add_symbol(Symbol {
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
            symbol_id: id,
            range: make_range(1, 0, 1, 10),
            scope: ScopeId::root(),
            property_access: Some("MY_VAR".into()),
            property_access_range: Some(make_range(1, 4, 1, 10)),
        });

        let resolver = BindingResolver::new(&graph);

        let cloned = resolver.get_env_reference_cloned(Position::new(1, 6));
        assert!(cloned.is_some());
        let cloned = cloned.unwrap();
        assert_eq!(cloned.name, "MY_VAR");
        assert_eq!(cloned.access_type, AccessType::Property);
    }

    #[test]
    fn test_get_env_reference_cloned_returns_none_for_binding() {
        let mut graph = BindingGraph::new();

        let _id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "dbUrl".into(),
            declaration_range: make_range(0, 0, 0, 40),
            name_range: make_range(0, 6, 0, 11),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "DATABASE_URL".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let resolver = BindingResolver::new(&graph);

        let cloned = resolver.get_env_reference_cloned(Position::new(0, 8));
        assert!(cloned.is_none());
    }

    #[test]
    fn test_get_env_binding_cloned() {
        let mut graph = BindingGraph::new();

        let scope_id = graph.add_scope(crate::types::Scope {
            id: ScopeId::new(1).unwrap(),
            parent: Some(ScopeId::root()),
            range: make_range(0, 0, 10, 0),
            kind: ScopeKind::Function,
        });

        let _id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "myVar".into(),
            declaration_range: make_range(1, 0, 1, 35),
            name_range: make_range(1, 6, 1, 11),
            scope: scope_id,
            origin: SymbolOrigin::EnvVar {
                name: "MY_VAR".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: Some(make_range(1, 6, 1, 11)),
        });

        let resolver = BindingResolver::new(&graph);

        let binding = resolver.get_env_binding_cloned(Position::new(1, 8));
        assert!(binding.is_some());
        let binding = binding.unwrap();
        assert_eq!(binding.binding_name, "myVar");
        assert_eq!(binding.env_var_name, "MY_VAR");
        assert_eq!(binding.scope_range, make_range(0, 0, 10, 0));
        assert!(binding.is_valid);
        assert_eq!(
            binding.destructured_key_range,
            Some(make_range(1, 6, 1, 11))
        );
    }

    #[test]
    fn test_get_env_binding_cloned_returns_none_for_direct_ref() {
        let mut graph = BindingGraph::new();

        graph.add_direct_reference(EnvReference {
            name: "DIRECT".into(),
            full_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 12, 0, 18),
            access_type: AccessType::Property,
            has_default: false,
            default_value: None,
        });

        let resolver = BindingResolver::new(&graph);

        let binding = resolver.get_env_binding_cloned(Position::new(0, 15));
        assert!(binding.is_none());
    }

    #[test]
    fn test_get_binding_usage_cloned() {
        let mut graph = BindingGraph::new();

        let id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "usedVar".into(),
            declaration_range: make_range(0, 0, 0, 30),
            name_range: make_range(0, 6, 0, 13),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "USED_VAR".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        graph.add_usage(SymbolUsage {
            symbol_id: id,
            range: make_range(2, 5, 2, 12),
            scope: ScopeId::root(),
            property_access: None,
            property_access_range: None,
        });

        let resolver = BindingResolver::new(&graph);

        let usage = resolver.get_binding_usage_cloned(Position::new(2, 8));
        assert!(usage.is_some());
        let usage = usage.unwrap();
        assert_eq!(usage.name, "usedVar");
        assert_eq!(usage.env_var_name, "USED_VAR");
        assert_eq!(usage.range, make_range(2, 5, 2, 12));
    }

    #[test]
    fn test_get_binding_usage_cloned_returns_none_for_declaration() {
        let mut graph = BindingGraph::new();

        let _id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "declVar".into(),
            declaration_range: make_range(0, 0, 0, 30),
            name_range: make_range(0, 6, 0, 13),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "DECL_VAR".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let resolver = BindingResolver::new(&graph);

        let usage = resolver.get_binding_usage_cloned(Position::new(0, 8));
        assert!(usage.is_none());
    }

    #[test]
    fn test_get_binding_kind() {
        let mut graph = BindingGraph::new();

        let _id1 = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "valueBinding".into(),
            declaration_range: make_range(0, 0, 0, 40),
            name_range: make_range(0, 6, 0, 18),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "VALUE".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let _id2 = graph.add_symbol(Symbol {
            id: SymbolId::new(2).unwrap(),
            name: "objectBinding".into(),
            declaration_range: make_range(1, 0, 1, 30),
            name_range: make_range(1, 6, 1, 19),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvObject {
                canonical_name: "process.env".into(),
            },
            kind: SymbolKind::EnvObject,
            is_valid: true,
            destructured_key_range: None,
        });

        let _id3 = graph.add_symbol(Symbol {
            id: SymbolId::new(3).unwrap(),
            name: "invalidBinding".into(),
            declaration_range: make_range(2, 0, 2, 40),
            name_range: make_range(2, 6, 2, 20),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "INVALID".into(),
            },
            kind: SymbolKind::Value,
            is_valid: false,
            destructured_key_range: None,
        });

        let resolver = BindingResolver::new(&graph);

        let kind1 = resolver.get_binding_kind("valueBinding");
        assert!(kind1.is_some());
        assert_eq!(kind1.unwrap(), BindingKind::Value);

        let kind2 = resolver.get_binding_kind("objectBinding");
        assert!(kind2.is_some());
        assert_eq!(kind2.unwrap(), BindingKind::Object);

        let kind3 = resolver.get_binding_kind("invalidBinding");
        assert!(kind3.is_none());

        let kind4 = resolver.get_binding_kind("nonexistent");
        assert!(kind4.is_none());
    }

    #[test]
    fn test_usage_kind_equality() {
        assert_eq!(UsageKind::DirectReference, UsageKind::DirectReference);
        assert_eq!(UsageKind::BindingDeclaration, UsageKind::BindingDeclaration);
        assert_eq!(UsageKind::BindingUsage, UsageKind::BindingUsage);
        assert_eq!(UsageKind::PropertyAccess, UsageKind::PropertyAccess);

        assert_ne!(UsageKind::DirectReference, UsageKind::BindingUsage);
        assert_ne!(UsageKind::BindingDeclaration, UsageKind::PropertyAccess);
    }

    #[test]
    fn test_usage_kind_copy() {
        let kind = UsageKind::DirectReference;
        let copied = kind;
        assert_eq!(kind, copied);
    }

    #[test]
    fn test_env_var_usage_location_clone() {
        let location = EnvVarUsageLocation {
            range: make_range(0, 0, 0, 10),
            kind: UsageKind::DirectReference,
            binding_name: Some("myBinding".into()),
        };

        let cloned = location.clone();
        assert_eq!(cloned.range, location.range);
        assert_eq!(cloned.kind, location.kind);
        assert_eq!(cloned.binding_name, location.binding_name);
    }

    #[test]
    fn test_env_var_usage_location_debug() {
        let location = EnvVarUsageLocation {
            range: make_range(1, 5, 1, 15),
            kind: UsageKind::BindingUsage,
            binding_name: None,
        };

        let debug_str = format!("{:?}", location);
        assert!(debug_str.contains("EnvVarUsageLocation"));
        assert!(debug_str.contains("BindingUsage"));
    }

    #[test]
    fn test_complex_chain_resolution() {
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

        let settings_id = graph.add_symbol(Symbol {
            id: SymbolId::new(3).unwrap(),
            name: "settings".into(),
            declaration_range: make_range(2, 0, 2, 25),
            name_range: make_range(2, 6, 2, 14),
            scope: ScopeId::root(),
            origin: SymbolOrigin::Symbol { target: config_id },
            kind: SymbolKind::Variable,
            is_valid: true,
            destructured_key_range: None,
        });

        let _db_id = graph.add_symbol(Symbol {
            id: SymbolId::new(4).unwrap(),
            name: "DB_URL".into(),
            declaration_range: make_range(3, 0, 3, 30),
            name_range: make_range(3, 8, 3, 14),
            scope: ScopeId::root(),
            origin: SymbolOrigin::DestructuredProperty {
                source: settings_id,
                key: "DB_URL".into(),
            },
            kind: SymbolKind::DestructuredProperty,
            is_valid: true,
            destructured_key_range: Some(make_range(3, 8, 3, 14)),
        });

        let resolver = BindingResolver::new(&graph);

        let hit = resolver.env_at_position(Position::new(3, 10)).unwrap();
        assert_eq!(hit.env_var_name(), Some("DB_URL".into()));

        let hit2 = resolver.env_at_position(Position::new(2, 10)).unwrap();
        assert!(hit2.is_env_object());
        assert_eq!(hit2.canonical_name().as_str(), "process.env");
    }

    #[test]
    fn test_multiple_env_vars_same_document() {
        let mut graph = BindingGraph::new();

        graph.add_direct_reference(EnvReference {
            name: "VAR_A".into(),
            full_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 12, 0, 17),
            access_type: AccessType::Property,
            has_default: false,
            default_value: None,
        });

        graph.add_direct_reference(EnvReference {
            name: "VAR_B".into(),
            full_range: make_range(1, 0, 1, 20),
            name_range: make_range(1, 12, 1, 17),
            access_type: AccessType::Property,
            has_default: false,
            default_value: None,
        });

        let _id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "varC".into(),
            declaration_range: make_range(2, 0, 2, 30),
            name_range: make_range(2, 6, 2, 10),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "VAR_C".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        let resolver = BindingResolver::new(&graph);

        let hit_a = resolver.env_at_position(Position::new(0, 15)).unwrap();
        assert_eq!(hit_a.canonical_name().as_str(), "VAR_A");

        let hit_b = resolver.env_at_position(Position::new(1, 15)).unwrap();
        assert_eq!(hit_b.canonical_name().as_str(), "VAR_B");

        let hit_c = resolver.env_at_position(Position::new(2, 8)).unwrap();
        assert_eq!(hit_c.canonical_name().as_str(), "VAR_C");

        let all_vars = resolver.all_env_vars();
        assert_eq!(all_vars.len(), 3);
    }

    #[test]
    fn test_binding_with_usage_at_different_scopes() {
        let mut graph = BindingGraph::new();

        let inner_scope = graph.add_scope(crate::types::Scope {
            id: ScopeId::new(1).unwrap(),
            parent: Some(ScopeId::root()),
            range: make_range(1, 0, 5, 0),
            kind: ScopeKind::Block,
        });

        let id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "rootVar".into(),
            declaration_range: make_range(0, 0, 0, 30),
            name_range: make_range(0, 6, 0, 13),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "ROOT_VAR".into(),
            },
            kind: SymbolKind::Value,
            is_valid: true,
            destructured_key_range: None,
        });

        graph.add_usage(SymbolUsage {
            symbol_id: id,
            range: make_range(2, 5, 2, 12),
            scope: inner_scope,
            property_access: None,
            property_access_range: None,
        });

        let resolver = BindingResolver::new(&graph);

        let hit = resolver.env_at_position(Position::new(2, 8)).unwrap();
        assert!(matches!(hit, EnvHit::ViaUsage { .. }));
        assert_eq!(hit.env_var_name(), Some("ROOT_VAR".into()));
    }

    #[test]
    fn test_binding_object_with_property_access_and_destructuring() {
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

        graph.add_usage(SymbolUsage {
            symbol_id: env_id,
            range: make_range(1, 0, 1, 11),
            scope: ScopeId::root(),
            property_access: Some("VAR_ONE".into()),
            property_access_range: Some(make_range(1, 4, 1, 11)),
        });

        let _var_two_id = graph.add_symbol(Symbol {
            id: SymbolId::new(2).unwrap(),
            name: "VAR_TWO".into(),
            declaration_range: make_range(2, 0, 2, 25),
            name_range: make_range(2, 8, 2, 15),
            scope: ScopeId::root(),
            origin: SymbolOrigin::DestructuredProperty {
                source: env_id,
                key: "VAR_TWO".into(),
            },
            kind: SymbolKind::DestructuredProperty,
            is_valid: true,
            destructured_key_range: Some(make_range(2, 8, 2, 15)),
        });

        let resolver = BindingResolver::new(&graph);

        let var_one_usages = resolver.find_env_var_usages("VAR_ONE");
        assert_eq!(var_one_usages.len(), 1);
        assert_eq!(var_one_usages[0].kind, UsageKind::PropertyAccess);

        let var_two_usages = resolver.find_env_var_usages("VAR_TWO");
        assert_eq!(var_two_usages.len(), 1);
        assert_eq!(var_two_usages[0].kind, UsageKind::BindingDeclaration);
    }
}
