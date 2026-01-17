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
use tracing::error;

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
    /// For direct references, returns name_range (the variable name only),
    /// not full_range, so hover highlights just the var name.
    pub fn range(&self) -> Range {
        match self {
            EnvHit::DirectReference(r) => r.name_range,
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
        tracing::trace!(
            "Looking up env at position line={}, char={}",
            position.line,
            position.character
        );

        // 1. Check direct references first (highest priority)
        // Only match against name_range, not full_range, so hover/go-to-definition
        // only triggers when cursor is on the variable name (e.g., DB_URL),
        // not on other parts like "process.env" or quotes.
        for reference in self.graph.direct_references() {
            if BindingGraph::contains_position(reference.name_range, position) {
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
        // Using optimized range index for O(log n) lookup instead of O(n) iteration
        if let Some(symbol_id) = self.graph.symbol_at_destructure_key(position) {
            if let Some(symbol) = self.graph.get_symbol(symbol_id) {
                if let Some(resolved) = self.graph.resolve_to_env(symbol_id) {
                    return Some(EnvHit::ViaSymbol { symbol, resolved });
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

        tracing::debug!(
            "No env var found at position line={}, char={}",
            position.line,
            position.character
        );
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
    /// Only matches cursor position against name_range, not full_range,
    /// so lookup only succeeds when cursor is on the variable name.
    pub fn direct_reference_at_position(&self, position: Position) -> Option<&'a EnvReference> {
        for reference in self.graph.direct_references() {
            if BindingGraph::contains_position(reference.name_range, position) {
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
                // INVARIANT: scope_at_position always returns valid scopes during analysis,
                // so this should never fail. If it does, we have a data consistency bug.
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
    use crate::types::{AccessType, ScopeKind, SymbolKind, SymbolOrigin};

    fn make_range(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Range {
        Range::new(
            Position::new(start_line, start_char),
            Position::new(end_line, end_char),
        )
    }

    // =========================================================================
    // EnvHit Tests
    // =========================================================================

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

        // Test all EnvHit methods for DirectReference variant
        assert_eq!(hit.env_var_name(), Some("DATABASE_URL".into()));
        assert_eq!(hit.canonical_name().as_str(), "DATABASE_URL");
        assert_eq!(hit.range(), make_range(0, 12, 0, 24)); // name_range (not full_range)
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
        assert_eq!(hit.range(), make_range(0, 6, 0, 11)); // name_range
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

        assert!(hit.env_var_name().is_none()); // Object has no specific var
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

        // Usage with property access: env.DATABASE_URL
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
        // Property access should resolve to the variable name
        assert_eq!(hit.env_var_name(), Some("DATABASE_URL".into()));
        assert!(!hit.is_env_object()); // Because it's resolving to a variable now
    }

    // =========================================================================
    // ResolvedBinding Tests
    // =========================================================================

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

    // =========================================================================
    // BindingResolver Tests - Position-based queries
    // =========================================================================

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
        // binding_at_position returns None for direct references
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

        // Test at full range but outside name range - should NOT match
        // (e.g., cursor on "process" in "process.env.API_KEY")
        let ref_at_full_only = resolver.direct_reference_at_position(Position::new(0, 5));
        assert!(
            ref_at_full_only.is_none(),
            "Position in full_range but outside name_range should return None"
        );

        // Test at name range - should match
        let ref_at_name = resolver.direct_reference_at_position(Position::new(0, 15));
        assert!(ref_at_name.is_some());
        assert_eq!(ref_at_name.unwrap().name, "API_KEY");

        // Test outside both ranges
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

        // const { API_KEY: apiKey } = process.env
        // destructured_key_range is API_KEY, name_range is apiKey
        let _id = graph.add_symbol(Symbol {
            id: SymbolId::new(1).unwrap(),
            name: "apiKey".into(),
            declaration_range: make_range(0, 0, 0, 40),
            name_range: make_range(0, 18, 0, 24), // apiKey
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "API_KEY".into(),
            },
            kind: SymbolKind::DestructuredProperty,
            is_valid: true,
            destructured_key_range: Some(make_range(0, 8, 0, 15)), // API_KEY
        });

        let resolver = BindingResolver::new(&graph);

        // Hovering over "API_KEY" (destructure key) should find it
        let hit = resolver.env_at_position(Position::new(0, 10));
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert_eq!(hit.env_var_name(), Some("API_KEY".into()));

        // Hovering over "apiKey" (binding name) should also find it
        let hit2 = resolver.env_at_position(Position::new(0, 20));
        assert!(hit2.is_some());
    }

    // =========================================================================
    // BindingResolver Tests - Find all usages
    // =========================================================================

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
            name: "API_KEY".into(), // Same as env var name (shorthand destructuring)
            declaration_range: make_range(0, 0, 0, 35),
            name_range: make_range(0, 8, 0, 15),
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "API_KEY".into(),
            },
            kind: SymbolKind::DestructuredProperty,
            is_valid: true,
            destructured_key_range: None, // No separate key range for shorthand
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
            name: "apiKey".into(), // Local binding name
            declaration_range: make_range(0, 0, 0, 45),
            name_range: make_range(0, 18, 0, 24), // Range of "apiKey"
            scope: ScopeId::root(),
            origin: SymbolOrigin::EnvVar {
                name: "API_KEY".into(),
            },
            kind: SymbolKind::DestructuredProperty,
            is_valid: true,
            destructured_key_range: Some(make_range(0, 8, 0, 15)), // Range of "API_KEY"
        });

        let resolver = BindingResolver::new(&graph);

        let usages = resolver.find_env_var_usages("API_KEY");
        // Should include BindingDeclaration with the destructured_key_range
        assert_eq!(usages.len(), 1);
        assert!(usages.iter().any(|u| u.kind == UsageKind::BindingDeclaration));
        // The range should be the key range, not the binding name range
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

        // env.DATABASE_URL usage
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
        // Should use the property_access_range
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

        // env.API_KEY usage without specific property range
        graph.add_usage(SymbolUsage {
            symbol_id: id,
            range: make_range(2, 5, 2, 16),
            scope: ScopeId::root(),
            property_access: Some("API_KEY".into()),
            property_access_range: None, // No specific range
        });

        let resolver = BindingResolver::new(&graph);
        let usages = resolver.find_env_var_usages("API_KEY");

        assert_eq!(usages.len(), 1);
        // Should fall back to usage.range
        assert_eq!(usages[0].range, make_range(2, 5, 2, 16));
    }

    #[test]
    fn test_find_usages_deduplication() {
        let mut graph = BindingGraph::new();

        // Add same direct reference twice (to test dedup)
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
            name_range: make_range(0, 12, 0, 20), // Same range
            access_type: AccessType::Property,
            has_default: false,
            default_value: None,
        });

        let resolver = BindingResolver::new(&graph);
        let usages = resolver.find_env_var_usages("DUPLICATE");

        // Should be deduplicated
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

        // Object binding should not appear in all_env_vars
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

        // Object shouldn't be in the list
        assert!(all_vars.is_empty());
    }

    // =========================================================================
    // BindingResolver Tests - Symbol queries
    // =========================================================================

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

        // Nonexistent ID
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

        // Found in root scope
        let symbol = resolver.lookup_symbol("myVar", ScopeId::root());
        assert!(symbol.is_some());
        assert_eq!(symbol.unwrap().name, "myVar");

        // Not found
        let not_found = resolver.lookup_symbol("nonexistent", ScopeId::root());
        assert!(not_found.is_none());
    }

    #[test]
    fn test_scope_at_position() {
        let graph = BindingGraph::new();
        let resolver = BindingResolver::new(&graph);

        // With just root scope, everything returns root
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

    // =========================================================================
    // BindingResolver Tests - Legacy compatibility
    // =========================================================================

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

        // env.MY_VAR usage
        graph.add_usage(SymbolUsage {
            symbol_id: id,
            range: make_range(1, 0, 1, 10),
            scope: ScopeId::root(),
            property_access: Some("MY_VAR".into()),
            property_access_range: Some(make_range(1, 4, 1, 10)),
        });

        let resolver = BindingResolver::new(&graph);

        // Should synthesize an EnvReference from property access
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

        // Symbol declaration is not a direct reference
        let cloned = resolver.get_env_reference_cloned(Position::new(0, 8));
        assert!(cloned.is_none());
    }

    #[test]
    fn test_get_env_binding_cloned() {
        let mut graph = BindingGraph::new();

        // Add a scope first
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
        assert_eq!(binding.destructured_key_range, Some(make_range(1, 6, 1, 11)));
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

        // Declaration position, not usage
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

        // Invalid symbol (is_valid = false)
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

        // Invalid binding should not be found
        let kind3 = resolver.get_binding_kind("invalidBinding");
        assert!(kind3.is_none());

        // Nonexistent binding
        let kind4 = resolver.get_binding_kind("nonexistent");
        assert!(kind4.is_none());
    }

    // =========================================================================
    // UsageKind Tests
    // =========================================================================

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
        let copied = kind; // Copy trait
        assert_eq!(kind, copied);
    }

    // =========================================================================
    // EnvVarUsageLocation Tests
    // =========================================================================

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

    // =========================================================================
    // Integration Tests - Complex scenarios
    // =========================================================================

    #[test]
    fn test_complex_chain_resolution() {
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

        // const settings = config
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

        // const { DB_URL } = settings
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

        // Query at DB_URL - should resolve through entire chain
        let hit = resolver.env_at_position(Position::new(3, 10)).unwrap();
        assert_eq!(hit.env_var_name(), Some("DB_URL".into()));

        // Query at settings - should resolve to process.env (object)
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

        // Each position should resolve to correct var
        let hit_a = resolver.env_at_position(Position::new(0, 15)).unwrap();
        assert_eq!(hit_a.canonical_name().as_str(), "VAR_A");

        let hit_b = resolver.env_at_position(Position::new(1, 15)).unwrap();
        assert_eq!(hit_b.canonical_name().as_str(), "VAR_B");

        let hit_c = resolver.env_at_position(Position::new(2, 8)).unwrap();
        assert_eq!(hit_c.canonical_name().as_str(), "VAR_C");

        // All env vars should be collected
        let all_vars = resolver.all_env_vars();
        assert_eq!(all_vars.len(), 3);
    }

    #[test]
    fn test_binding_with_usage_at_different_scopes() {
        let mut graph = BindingGraph::new();

        // Create inner scope
        let inner_scope = graph.add_scope(crate::types::Scope {
            id: ScopeId::new(1).unwrap(),
            parent: Some(ScopeId::root()),
            range: make_range(1, 0, 5, 0),
            kind: ScopeKind::Block,
        });

        // Declaration in root scope
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

        // Usage in inner scope
        graph.add_usage(SymbolUsage {
            symbol_id: id,
            range: make_range(2, 5, 2, 12),
            scope: inner_scope,
            property_access: None,
            property_access_range: None,
        });

        let resolver = BindingResolver::new(&graph);

        // Usage should resolve correctly
        let hit = resolver.env_at_position(Position::new(2, 8)).unwrap();
        assert!(matches!(hit, EnvHit::ViaUsage { .. }));
        assert_eq!(hit.env_var_name(), Some("ROOT_VAR".into()));
    }

    #[test]
    fn test_binding_object_with_property_access_and_destructuring() {
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

        // Property access: env.VAR_ONE
        graph.add_usage(SymbolUsage {
            symbol_id: env_id,
            range: make_range(1, 0, 1, 11),
            scope: ScopeId::root(),
            property_access: Some("VAR_ONE".into()),
            property_access_range: Some(make_range(1, 4, 1, 11)),
        });

        // Destructuring: const { VAR_TWO } = env
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

        // Find usages for VAR_ONE (property access)
        let var_one_usages = resolver.find_env_var_usages("VAR_ONE");
        assert_eq!(var_one_usages.len(), 1);
        assert_eq!(var_one_usages[0].kind, UsageKind::PropertyAccess);

        // Find usages for VAR_TWO (destructuring)
        let var_two_usages = resolver.find_env_var_usages("VAR_TWO");
        assert_eq!(var_two_usages.len(), 1);
        assert_eq!(var_two_usages[0].kind, UsageKind::BindingDeclaration);
    }
}
