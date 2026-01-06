//! Binding Resolver - Query-time resolution for LSP features.
//!
//! This module provides the resolver that uses the BindingGraph to answer
//! LSP queries like hover, go-to-definition, and find-references.

use crate::analysis::binding_graph::BindingGraph;
use crate::types::{
    BindingKind, EnvBinding, EnvBindingUsage, EnvReference, ResolvedEnv, ScopeId, Symbol, SymbolId,
    SymbolUsage,
};
use compact_str::CompactString;
use tower_lsp::lsp_types::{Position, Range};

/// Result of looking up an env var at a position using the new binding graph.
#[derive(Debug, Clone)]
pub enum EnvHit<'a> {
    /// Direct reference to an env var (e.g., process.env.DATABASE_URL).
    DirectReference(&'a EnvReference),

    /// Via a symbol in the binding graph.
    /// Contains the symbol and its resolved env var/object.
    ViaSymbol {
        symbol: &'a Symbol,
        resolved: ResolvedEnv,
    },

    /// Via a symbol usage (identifier that references a symbol).
    ViaUsage {
        usage: &'a SymbolUsage,
        symbol: &'a Symbol,
        resolved: ResolvedEnv,
    },
}

impl<'a> EnvHit<'a> {
    /// Get the env var name from this hit.
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

    /// Get the canonical name (env var name or object name).
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

    /// Get the range for this hit.
    pub fn range(&self) -> Range {
        match self {
            EnvHit::DirectReference(r) => r.full_range,
            EnvHit::ViaSymbol { symbol, .. } => symbol.name_range,
            EnvHit::ViaUsage { usage, .. } => usage.range,
        }
    }

    /// Check if this hit resolves to an env object (not a specific var).
    pub fn is_env_object(&self) -> bool {
        match self {
            EnvHit::DirectReference(_) => false,
            EnvHit::ViaSymbol { resolved, .. } | EnvHit::ViaUsage { resolved, .. } => {
                matches!(resolved, ResolvedEnv::Object(_))
            }
        }
    }

    /// Get the symbol name (for bindings/usages).
    pub fn binding_name(&self) -> Option<&CompactString> {
        match self {
            EnvHit::DirectReference(_) => None,
            EnvHit::ViaSymbol { symbol, .. } => Some(&symbol.name),
            EnvHit::ViaUsage { symbol, .. } => Some(&symbol.name),
        }
    }
}

/// Information about a resolved binding (for legacy handler compatibility).
#[derive(Debug, Clone)]
pub struct ResolvedBinding {
    /// The binding/variable name as declared.
    pub binding_name: CompactString,
    /// The env var name it resolves to.
    pub env_var_name: CompactString,
    /// Range of the binding identifier.
    pub binding_range: Range,
    /// Range of the entire declaration.
    pub declaration_range: Range,
    /// The kind of binding.
    pub kind: BindingKind,
    /// Whether this is a symbol usage (vs the declaration itself).
    pub is_usage: bool,
}

impl ResolvedBinding {
    /// Convert to the legacy EnvBinding type for backward compatibility.
    pub fn to_env_binding(&self, scope_range: Range) -> EnvBinding {
        EnvBinding {
            binding_name: self.binding_name.clone(),
            env_var_name: self.env_var_name.clone(),
            binding_range: self.binding_range,
            declaration_range: self.declaration_range,
            scope_range,
            is_valid: true,
            kind: self.kind.clone(),
            destructured_key_range: None, // Legacy compatibility - not tracked in ResolvedBinding
        }
    }

    /// Convert to the legacy EnvBindingUsage type for backward compatibility.
    pub fn to_binding_usage(&self) -> EnvBindingUsage {
        EnvBindingUsage {
            name: self.binding_name.clone(),
            range: self.binding_range,
            declaration_range: self.declaration_range,
            env_var_name: self.env_var_name.clone(),
        }
    }
}

/// Resolver for querying env vars from a BindingGraph.
pub struct BindingResolver<'a> {
    graph: &'a BindingGraph,
}

impl<'a> BindingResolver<'a> {
    /// Create a new resolver for the given binding graph.
    pub fn new(graph: &'a BindingGraph) -> Self {
        Self { graph }
    }

    // =========================================================================
    // Position-based queries (for hover, go-to-definition)
    // =========================================================================

    /// Find an env var at the given position.
    /// Checks direct references, symbol declarations, destructured keys, and symbol usages.
    pub fn env_at_position(&self, position: Position) -> Option<EnvHit<'a>> {
        // 1. Check direct references first (highest priority)
        for reference in self.graph.direct_references() {
            if BindingGraph::contains_position(reference.full_range, position)
                || BindingGraph::contains_position(reference.name_range, position)
            {
                return Some(EnvHit::DirectReference(reference));
            }
        }

        // 2. Check symbol declarations (name range)
        if let Some(symbol) = self.graph.symbol_at_position(position) {
            if let Some(resolved) = self.graph.resolve_to_env(symbol.id) {
                return Some(EnvHit::ViaSymbol { symbol, resolved });
            }
        }

        // 3. Check destructured property keys (e.g., hovering over API_KEY in `{ API_KEY: apiKey }`)
        // This is separate from symbol_at_position because the key range is different from the name range
        for symbol in self.graph.symbols() {
            if let Some(key_range) = symbol.destructured_key_range {
                if BindingGraph::contains_position(key_range, position) {
                    if let Some(resolved) = self.graph.resolve_to_env(symbol.id) {
                        return Some(EnvHit::ViaSymbol { symbol, resolved });
                    }
                }
            }
        }

        // 4. Check symbol usages
        if let Some(usage) = self.graph.usage_at_position(position) {
            if let Some(symbol) = self.graph.get_symbol(usage.symbol_id) {
                // Handle property access on object aliases (env.VAR)
                if let Some(property) = &usage.property_access {
                    // Resolve the symbol first
                    if let Some(resolved) = self.graph.resolve_to_env(usage.symbol_id) {
                        if matches!(resolved, ResolvedEnv::Object(_)) {
                            // This is a property access on an env object
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

        None
    }

    /// Get resolved binding information at a position.
    /// Returns None for direct references; use for bindings and usages.
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

    /// Get direct reference at position (if any).
    pub fn direct_reference_at_position(&self, position: Position) -> Option<&'a EnvReference> {
        for reference in self.graph.direct_references() {
            if BindingGraph::contains_position(reference.full_range, position)
                || BindingGraph::contains_position(reference.name_range, position)
            {
                return Some(reference);
            }
        }
        None
    }

    // =========================================================================
    // Find all usages (for find-references)
    // =========================================================================

    /// Find all locations where an env var is used (directly or through bindings).
    pub fn find_env_var_usages(&self, env_var_name: &str) -> Vec<EnvVarUsageLocation> {
        let mut locations = Vec::new();
        let mut seen_ranges = std::collections::HashSet::new();

        // 1. Direct references
        for reference in self.graph.direct_references() {
            if reference.name == env_var_name {
                // Deduplicate by range
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

        // 2. Symbol declarations that resolve to this env var
        // Only include if we have a proper env var range to rename
        for symbol in self.graph.symbols() {
            if let Some(resolved) = self.graph.resolve_to_env(symbol.id) {
                if let ResolvedEnv::Variable(name) = &resolved {
                    if name == env_var_name {
                        // Determine the correct range for renaming:
                        // - If destructured with rename (e.g., { VAR: v }), use destructured_key_range
                        // - If shorthand destructure (e.g., { VAR }), the symbol name IS the env var
                        // - If regular binding (e.g., const a = process.env.VAR), skip - DirectReference covers it
                        let rename_range = if let Some(key_range) = symbol.destructured_key_range {
                            // Renamed destructuring: { ENV_VAR: localName }
                            Some(key_range)
                        } else if symbol.name.as_str() == env_var_name {
                            // Shorthand destructuring: { ENV_VAR } - the binding name IS the env var
                            Some(symbol.name_range)
                        } else {
                            // Regular binding like `const a = process.env.VAR`
                            // The env var is already covered by DirectReference, skip the binding
                            None
                        };

                        if let Some(range) = rename_range {
                            // Deduplicate by range
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

        // 3. Symbol usages (identifiers referencing symbols that resolve to env var)
        for usage in self.graph.usages() {
            if let Some(resolved) = self.graph.resolve_to_env(usage.symbol_id) {
                match &resolved {
                    ResolvedEnv::Variable(name) if name == env_var_name => {
                        // Deduplicate by range
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
                        // Check property access
                        if let Some(prop) = &usage.property_access {
                            if prop == env_var_name {
                                // Use property_access_range if available (for rename),
                                // otherwise fall back to usage.range
                                let range = usage.property_access_range.unwrap_or(usage.range);
                                // Deduplicate by range
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

    /// Find all env vars referenced in the document.
    pub fn all_env_vars(&self) -> Vec<CompactString> {
        let mut vars = std::collections::HashSet::new();

        // Direct references
        for reference in self.graph.direct_references() {
            vars.insert(reference.name.clone());
        }

        // Through symbols
        for symbol in self.graph.symbols() {
            if let Some(ResolvedEnv::Variable(name)) = self.graph.resolve_to_env(symbol.id) {
                vars.insert(name);
            }
        }

        vars.into_iter().collect()
    }

    // =========================================================================
    // Symbol queries
    // =========================================================================

    /// Get a symbol by ID.
    pub fn get_symbol(&self, id: SymbolId) -> Option<&'a Symbol> {
        self.graph.get_symbol(id)
    }

    /// Lookup symbol by name in scope.
    pub fn lookup_symbol(&self, name: &str, scope: ScopeId) -> Option<&'a Symbol> {
        self.graph.lookup_symbol(name, scope)
    }

    /// Get the scope at a position.
    pub fn scope_at_position(&self, position: Position) -> ScopeId {
        self.graph.scope_at_position(position)
    }

    /// Check if a symbol resolves to an env object.
    pub fn is_env_object(&self, symbol_id: SymbolId) -> bool {
        self.graph.resolves_to_env_object(symbol_id)
    }

    // =========================================================================
    // Legacy compatibility methods
    // =========================================================================

    /// Get a cloned EnvReference at position (legacy compatibility).
    /// This handles both direct references and property access on env object aliases.
    pub fn get_env_reference_cloned(&self, position: Position) -> Option<EnvReference> {
        // First check for direct reference
        if let Some(reference) = self.direct_reference_at_position(position) {
            return Some(reference.clone());
        }

        // Check for property access on an env object alias (e.g., env.API_KEY)
        if let Some(usage) = self.graph.usage_at_position(position) {
            if let Some(property) = &usage.property_access {
                if let Some(resolved) = self.graph.resolve_to_env(usage.symbol_id) {
                    if matches!(resolved, ResolvedEnv::Object(_)) {
                        // Synthesize an EnvReference for this property access
                        return Some(EnvReference {
                            name: property.clone(),
                            full_range: usage.range,
                            name_range: usage.range, // Best approximation
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

    /// Get a cloned EnvBinding at position (legacy compatibility).
    pub fn get_env_binding_cloned(&self, position: Position) -> Option<EnvBinding> {
        // Use env_at_position which checks name_range AND destructured_key_range
        let hit = self.env_at_position(position)?;

        match hit {
            EnvHit::DirectReference(_) => None,
            EnvHit::ViaUsage { .. } => None, // This is a usage, not a declaration

            EnvHit::ViaSymbol { symbol, resolved } => {
                let env_var_name = match &resolved {
                    ResolvedEnv::Variable(name) => name.clone(),
                    ResolvedEnv::Object(name) => name.clone(),
                };

                let kind = match &resolved {
                    ResolvedEnv::Variable(_) => BindingKind::Value,
                    ResolvedEnv::Object(_) => BindingKind::Object,
                };

                // Get scope range from the symbol's scope
                let scope = self.graph.get_scope(symbol.scope)?;

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

    /// Get a cloned EnvBindingUsage at position (legacy compatibility).
    pub fn get_binding_usage_cloned(&self, position: Position) -> Option<EnvBindingUsage> {
        let binding = self.binding_at_position(position)?;
        if !binding.is_usage {
            return None; // This is a declaration, not a usage
        }
        Some(binding.to_binding_usage())
    }

    /// Get binding kind for a symbol name (legacy compatibility).
    pub fn get_binding_kind(&self, name: &str) -> Option<BindingKind> {
        // Search through all symbols for one with this name
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

/// Location of an env var usage in the document.
#[derive(Debug, Clone)]
pub struct EnvVarUsageLocation {
    /// Range of the usage.
    pub range: Range,
    /// Kind of usage.
    pub kind: UsageKind,
    /// Binding name if accessed through a binding.
    pub binding_name: Option<CompactString>,
}

/// Kind of env var usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageKind {
    /// Direct reference (e.g., process.env.VAR).
    DirectReference,
    /// Binding declaration (e.g., const x = process.env.VAR).
    BindingDeclaration,
    /// Usage of a binding (e.g., using x after above declaration).
    BindingUsage,
    /// Property access on an env object alias (e.g., env.VAR).
    PropertyAccess,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{SymbolKind, SymbolOrigin};

    fn make_range(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Range {
        Range::new(
            Position::new(start_line, start_char),
            Position::new(end_line, end_char),
        )
    }

    #[test]
    fn test_resolve_direct_reference() {
        let mut graph = BindingGraph::new();

        // Add a direct reference
        graph.add_direct_reference(EnvReference {
            name: "DATABASE_URL".into(),
            full_range: make_range(0, 0, 0, 30),
            name_range: make_range(0, 12, 0, 24),
            access_type: crate::types::AccessType::Property,
            has_default: false,
            default_value: None,
        });

        let resolver = BindingResolver::new(&graph);

        // Query at the reference position
        let hit = resolver.env_at_position(Position::new(0, 15)).unwrap();
        assert!(matches!(hit, EnvHit::DirectReference(_)));
        assert_eq!(hit.env_var_name(), Some("DATABASE_URL".into()));
    }

    #[test]
    fn test_resolve_via_symbol() {
        let mut graph = BindingGraph::new();

        // Add a symbol: const dbUrl = process.env.DATABASE_URL
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

        // Query at the symbol name position
        let hit = resolver.env_at_position(Position::new(0, 8)).unwrap();
        assert!(matches!(hit, EnvHit::ViaSymbol { .. }));
        assert_eq!(hit.env_var_name(), Some("DATABASE_URL".into()));
        assert_eq!(hit.binding_name(), Some(&"dbUrl".into()));
    }

    #[test]
    fn test_resolve_chain() {
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
            destructured_key_range: Some(make_range(2, 8, 2, 14)), // Same as name in this case
        });

        let resolver = BindingResolver::new(&graph);

        // Query at DB_URL symbol
        let hit = resolver.env_at_position(Position::new(2, 10)).unwrap();
        assert!(matches!(hit, EnvHit::ViaSymbol { .. }));
        assert_eq!(hit.env_var_name(), Some("DB_URL".into()));
    }

    #[test]
    fn test_find_usages() {
        let mut graph = BindingGraph::new();

        // Add direct reference
        graph.add_direct_reference(EnvReference {
            name: "API_KEY".into(),
            full_range: make_range(0, 0, 0, 20),
            name_range: make_range(0, 12, 0, 19),
            access_type: crate::types::AccessType::Property,
            has_default: false,
            default_value: None,
        });

        // Add binding
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

        // Add usage of binding
        graph.add_usage(SymbolUsage {
            symbol_id: id,
            range: make_range(2, 10, 2, 16),
            scope: ScopeId::root(),
            property_access: None,
            property_access_range: None,
        });

        let resolver = BindingResolver::new(&graph);

        let usages = resolver.find_env_var_usages("API_KEY");
        // Only 2 usages now:
        // - DirectReference: for the actual env var access
        // - BindingUsage: for usages of the binding
        // BindingDeclaration is excluded because the binding name ("apiKey") != env var name ("API_KEY")
        // and there's no destructured_key_range. This is correct for rename - we don't want to
        // rename the local variable "apiKey" when renaming "API_KEY".
        assert_eq!(usages.len(), 2);

        assert!(usages.iter().any(|u| u.kind == UsageKind::DirectReference));
        assert!(usages.iter().any(|u| u.kind == UsageKind::BindingUsage));
    }

    #[test]
    fn test_find_usages_destructured() {
        let mut graph = BindingGraph::new();

        // Add binding with shorthand destructuring: const { API_KEY } = process.env
        // Here binding name == env var name
        graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "API_KEY".into(),  // Same as env var name (shorthand destructuring)
            declaration_range: make_range(0, 0, 0, 35),
            name_range: make_range(0, 8, 0, 15),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "API_KEY".into(),
            },
            kind: SymbolKind::DestructuredProperty,
            is_valid: true,
            destructured_key_range: None,  // No separate key range for shorthand
        });

        let resolver = BindingResolver::new(&graph);

        let usages = resolver.find_env_var_usages("API_KEY");
        // Should include BindingDeclaration because binding name == env var name
        assert_eq!(usages.len(), 1);
        assert!(usages.iter().any(|u| u.kind == UsageKind::BindingDeclaration));
    }

    #[test]
    fn test_find_usages_destructured_with_rename() {
        let mut graph = BindingGraph::new();

        // Add binding with renamed destructuring: const { API_KEY: apiKey } = process.env
        graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "apiKey".into(),  // Local binding name
            declaration_range: make_range(0, 0, 0, 45),
            name_range: make_range(0, 18, 0, 24),  // Range of "apiKey"
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "API_KEY".into(),
            },
            kind: SymbolKind::DestructuredProperty,
            is_valid: true,
            destructured_key_range: Some(make_range(0, 8, 0, 15)),  // Range of "API_KEY"
        });

        let resolver = BindingResolver::new(&graph);

        let usages = resolver.find_env_var_usages("API_KEY");
        // Should include BindingDeclaration with the destructured_key_range
        assert_eq!(usages.len(), 1);
        assert!(usages.iter().any(|u| u.kind == UsageKind::BindingDeclaration));
        // The range should be the key range, not the binding name range
        assert_eq!(usages[0].range, make_range(0, 8, 0, 15));
    }
}
