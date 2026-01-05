//! Analysis Pipeline - Multi-phase analysis for building the BindingGraph.
//!
//! This module orchestrates the analysis of a document to build a comprehensive
//! BindingGraph that tracks environment variable bindings, chains, and usages.

use crate::analysis::binding_graph::BindingGraph;
use crate::analysis::query::QueryEngine;
use crate::languages::LanguageSupport;
use crate::types::{
    ImportContext, Scope, ScopeId, Symbol, SymbolId, SymbolKind, SymbolOrigin, SymbolUsage,
};
use tower_lsp::lsp_types::{Position, Range};
use tree_sitter::Tree;

/// Orchestrates multi-phase analysis to build a BindingGraph.
pub struct AnalysisPipeline;

impl AnalysisPipeline {
    /// Run full analysis on a document and build the BindingGraph.
    ///
    /// The analysis proceeds in 6 phases:
    /// 1. Extract scopes from the AST
    /// 2. Extract direct environment variable references
    /// 3. Extract bindings (both direct and chain assignments)
    /// 4. Resolve origin chains
    /// 5. Extract usages
    /// 6. Process reassignments
    pub async fn analyze(
        query_engine: &QueryEngine,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
        import_context: &ImportContext,
    ) -> BindingGraph {
        let mut graph = BindingGraph::new();

        // Set root scope range to the entire document
        let root_range = Self::ts_to_lsp_range(tree.root_node().range());
        graph.set_root_range(root_range);

        // Phase 1: Extract scopes
        Self::extract_scopes(language, tree, &mut graph);

        // Phase 2: Extract direct references
        Self::extract_direct_references(
            query_engine,
            language,
            tree,
            source,
            import_context,
            &mut graph,
        )
        .await;

        // Phase 3: Extract bindings
        Self::extract_bindings(query_engine, language, tree, source, &mut graph).await;

        // Phase 4: Resolve origins (build chains)
        Self::resolve_origins(&mut graph);

        // Phase 5: Extract usages
        Self::extract_usages(query_engine, language, tree, source, &mut graph).await;

        // Phase 6: Process reassignments
        Self::process_reassignments(query_engine, language, tree, source, &mut graph).await;

        graph
    }

    // =========================================================================
    // Phase 1: Extract Scopes
    // =========================================================================

    fn extract_scopes(language: &dyn LanguageSupport, tree: &Tree, graph: &mut BindingGraph) {
        Self::walk_for_scopes(tree.root_node(), language, graph, ScopeId::root());
    }

    fn walk_for_scopes(
        node: tree_sitter::Node,
        language: &dyn LanguageSupport,
        graph: &mut BindingGraph,
        parent_scope: ScopeId,
    ) {
        // Check if this node creates a new scope
        // Skip root nodes (program/source_file/module) as they are already the root scope
        let current_scope = if language.is_scope_node(node) && !language.is_root_node(node) {
            let scope_kind = language.node_to_scope_kind(node.kind());
            let scope = Scope {
                id: ScopeId::root(), // Placeholder, will be overwritten by add_scope
                parent: Some(parent_scope),
                range: Self::ts_to_lsp_range(node.range()),
                kind: scope_kind,
            };
            graph.add_scope(scope)
        } else {
            parent_scope
        };

        // Recurse to children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_for_scopes(child, language, graph, current_scope);
        }
    }

    // =========================================================================
    // Phase 2: Extract Direct References
    // =========================================================================

    async fn extract_direct_references(
        query_engine: &QueryEngine,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
        import_context: &ImportContext,
        graph: &mut BindingGraph,
    ) {
        // Use existing extract_references from QueryEngine
        let references = query_engine
            .extract_references(language, tree, source, import_context)
            .await;

        for reference in references {
            graph.add_direct_reference(reference);
        }
    }

    // =========================================================================
    // Phase 3: Extract Bindings
    // =========================================================================

    async fn extract_bindings(
        query_engine: &QueryEngine,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
        graph: &mut BindingGraph,
    ) {
        // Extract bindings using existing query
        let bindings = query_engine.extract_bindings(language, tree, source).await;

        for binding in bindings {
            // Determine the scope for this binding
            let scope = graph.scope_at_position(binding.binding_range.start);

            // Determine origin based on binding kind
            let (origin, kind) = match binding.kind {
                crate::types::BindingKind::Object => {
                    // Check if this is a specific env var from destructuring or the env object itself
                    let is_env_object = language
                        .default_env_object_name()
                        .map(|name| binding.env_var_name == name)
                        .unwrap_or(false);

                    if is_env_object {
                        // This is an object alias: const env = process.env
                        (
                            SymbolOrigin::EnvObject {
                                canonical_name: binding.env_var_name.clone(),
                            },
                            SymbolKind::EnvObject,
                        )
                    } else {
                        // This is a destructured property: const { VAR } = process.env
                        (
                            SymbolOrigin::EnvVar {
                                name: binding.env_var_name.clone(),
                            },
                            SymbolKind::DestructuredProperty,
                        )
                    }
                }
                crate::types::BindingKind::Value => {
                    // Direct env var binding: const x = process.env.VAR
                    (
                        SymbolOrigin::EnvVar {
                            name: binding.env_var_name.clone(),
                        },
                        SymbolKind::Value,
                    )
                }
            };

            let symbol = Symbol {
                id: SymbolId::new(1).unwrap(), // Placeholder, will be overwritten
                name: binding.binding_name.clone(),
                declaration_range: binding.declaration_range,
                name_range: binding.binding_range,
                scope,
                origin,
                kind,
                is_valid: true,
                destructured_key_range: binding.destructured_key_range,
            };

            graph.add_symbol(symbol);
        }

        // Extract chain assignments (const b = a) using assignment_query
        let assignments = query_engine
            .extract_assignments(language, tree, source)
            .await;

        for (target_name, target_range, source_name) in assignments {
            let scope = graph.scope_at_position(target_range.start);

            // Create symbol with unknown origin - will be resolved in Phase 4
            let symbol = Symbol {
                id: SymbolId::new(1).unwrap(),
                name: target_name,
                declaration_range: target_range,
                name_range: target_range,
                scope,
                origin: SymbolOrigin::Unknown, // Will resolve to source_name
                kind: SymbolKind::Variable,
                is_valid: true,
                destructured_key_range: None, // Not a destructured binding
            };

            let symbol_id = graph.add_symbol(symbol);

            // Try to find the source symbol and link to it
            let source_id = graph.lookup_symbol_id(&source_name, scope);
            if let Some(target_id) = source_id {
                if let Some(sym) = graph.get_symbol_mut(symbol_id) {
                    sym.origin = SymbolOrigin::Symbol { target: target_id };
                }
            } else {
                // Source not found yet - mark for forward reference resolution in Phase 4
                if let Some(sym) = graph.get_symbol_mut(symbol_id) {
                    sym.origin = SymbolOrigin::UnresolvedSymbol { source_name };
                }
            }
        }

        // Extract destructures from identifiers (const { X } = obj)
        let destructures = query_engine
            .extract_destructures(language, tree, source)
            .await;

        for (target_name, target_range, key_name, key_range, source_name) in destructures {
            let scope = graph.scope_at_position(target_range.start);

            // Find the source symbol ID first
            let source_id = graph.lookup_symbol_id(&source_name, scope);

            // Create symbol with destructured property origin
            let origin = if let Some(src_id) = source_id {
                SymbolOrigin::DestructuredProperty {
                    source: src_id,
                    key: key_name,
                }
            } else {
                // Source not found yet - mark for forward reference resolution in Phase 4
                SymbolOrigin::UnresolvedDestructure {
                    source_name,
                    key: key_name,
                }
            };

            let symbol = Symbol {
                id: SymbolId::new(1).unwrap(),
                name: target_name,
                declaration_range: target_range,
                name_range: target_range,
                scope,
                origin,
                kind: SymbolKind::DestructuredProperty,
                is_valid: true,
                destructured_key_range: Some(key_range), // Range of the property key
            };

            graph.add_symbol(symbol);
        }
    }

    // =========================================================================
    // Phase 4: Resolve Origins
    // =========================================================================

    fn resolve_origins(graph: &mut BindingGraph) {
        // Build a name -> symbol ID map for the entire graph
        // This allows us to resolve forward references
        let mut name_to_id: std::collections::HashMap<
            (compact_str::CompactString, ScopeId),
            SymbolId,
        > = std::collections::HashMap::new();

        for symbol in graph.symbols() {
            let key = (symbol.name.clone(), symbol.scope);
            name_to_id.insert(key, symbol.id);
        }

        // Collect symbols needing resolution (to avoid borrow issues)
        let symbols_to_resolve: Vec<(SymbolId, ScopeId, SymbolOrigin)> = graph
            .symbols()
            .iter()
            .filter(|s| {
                matches!(
                    s.origin,
                    SymbolOrigin::UnresolvedSymbol { .. }
                        | SymbolOrigin::UnresolvedDestructure { .. }
                )
            })
            .map(|s| (s.id, s.scope, s.origin.clone()))
            .collect();

        // Resolve each unresolved symbol
        for (symbol_id, scope, origin) in symbols_to_resolve {
            let new_origin = match origin {
                SymbolOrigin::UnresolvedSymbol { source_name } => {
                    // Try to find source symbol by walking up the scope chain
                    Self::lookup_in_scope_chain(&name_to_id, graph, &source_name, scope)
                        .map(|target| SymbolOrigin::Symbol { target })
                        .unwrap_or(SymbolOrigin::Unresolvable)
                }
                SymbolOrigin::UnresolvedDestructure { source_name, key } => {
                    // Try to find source symbol by walking up the scope chain
                    Self::lookup_in_scope_chain(&name_to_id, graph, &source_name, scope)
                        .map(|source| SymbolOrigin::DestructuredProperty { source, key })
                        .unwrap_or(SymbolOrigin::Unresolvable)
                }
                _ => continue, // Already resolved
            };

            // Update the symbol's origin
            if let Some(sym) = graph.get_symbol_mut(symbol_id) {
                sym.origin = new_origin;
            }
        }
    }

    /// Look up a symbol name in the scope chain, trying current scope first,
    /// then walking up to parent scopes.
    fn lookup_in_scope_chain(
        name_to_id: &std::collections::HashMap<(compact_str::CompactString, ScopeId), SymbolId>,
        graph: &BindingGraph,
        name: &str,
        start_scope: ScopeId,
    ) -> Option<SymbolId> {
        let name = compact_str::CompactString::from(name);
        let mut current_scope = Some(start_scope);

        while let Some(scope_id) = current_scope {
            // Try to find in current scope
            let key = (name.clone(), scope_id);
            if let Some(&symbol_id) = name_to_id.get(&key) {
                return Some(symbol_id);
            }

            // Walk up to parent scope
            current_scope = graph.get_scope(scope_id).and_then(|s| s.parent);
        }

        None
    }

    // =========================================================================
    // Phase 5: Extract Usages
    // =========================================================================

    async fn extract_usages(
        query_engine: &QueryEngine,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
        graph: &mut BindingGraph,
    ) {
        // Extract all identifiers
        let identifiers = query_engine
            .extract_identifiers(language, tree, source)
            .await;

        for (name, range) in identifiers {
            // Find the scope at this position
            let scope = graph.scope_at_position(range.start);

            // Try to find a symbol with this name in the scope chain
            if let Some(symbol) = graph.lookup_symbol(&name, scope) {
                // Only record if the usage is after the declaration
                // and the symbol is env-related
                if range.start.line > symbol.declaration_range.end.line
                    || (range.start.line == symbol.declaration_range.end.line
                        && range.start.character > symbol.declaration_range.end.character)
                {
                    // Skip if this is the declaration itself
                    if range != symbol.name_range {
                        let usage = SymbolUsage {
                            symbol_id: symbol.id,
                            range,
                            scope,
                            property_access: None,
                        };
                        graph.add_usage(usage);
                    }
                }
            }
        }

        // Handle property access on object aliases (env.VAR, env["VAR"])
        Self::extract_property_accesses(language, tree, source, graph);
    }

    /// Extract property accesses on env object aliases (e.g., env.API_KEY, env["SECRET"])
    fn extract_property_accesses(
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
        graph: &mut BindingGraph,
    ) {
        Self::walk_for_property_accesses(language, tree.root_node(), source, graph);
    }

    fn walk_for_property_accesses(
        language: &dyn LanguageSupport,
        node: tree_sitter::Node,
        source: &[u8],
        graph: &mut BindingGraph,
    ) {
        // Check for member_expression: obj.property
        if node.kind() == "member_expression" {
            if let (Some(object), Some(property)) = (
                node.child_by_field_name("object"),
                node.child_by_field_name("property"),
            ) {
                // Check if object is an identifier that resolves to an env object
                if object.kind() == "identifier" {
                    if let Ok(obj_name) = object.utf8_text(source) {
                        let scope = graph.scope_at_position(Position::new(
                            object.start_position().row as u32,
                            object.start_position().column as u32,
                        ));

                        if let Some(symbol) = graph.lookup_symbol(obj_name, scope) {
                            // Check if this symbol resolves to an env object
                            if graph.resolves_to_env_object(symbol.id) {
                                if let Ok(prop_name) = property.utf8_text(source) {
                                    // Create a usage with property access
                                    let usage_range = Self::ts_to_lsp_range(node.range());
                                    let usage = SymbolUsage {
                                        symbol_id: symbol.id,
                                        range: usage_range,
                                        scope,
                                        property_access: Some(prop_name.into()),
                                    };
                                    graph.add_usage(usage);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check for subscript_expression: obj["property"]
        if node.kind() == "subscript_expression" {
            if let (Some(object), Some(index)) = (
                node.child_by_field_name("object"),
                node.child_by_field_name("index"),
            ) {
                // Check if object is an identifier that resolves to an env object
                if object.kind() == "identifier" {
                    if let Ok(obj_name) = object.utf8_text(source) {
                        let scope = graph.scope_at_position(Position::new(
                            object.start_position().row as u32,
                            object.start_position().column as u32,
                        ));

                        if let Some(symbol) = graph.lookup_symbol(obj_name, scope) {
                            // Check if this symbol resolves to an env object
                            if graph.resolves_to_env_object(symbol.id) {
                                // Extract property name from string literal
                                if index.kind() == "string" {
                                    // Get string content (without quotes)
                                    if let Ok(raw) = index.utf8_text(source) {
                                        let prop_name = language.strip_quotes(raw);
                                        let usage_range = Self::ts_to_lsp_range(node.range());
                                        let usage = SymbolUsage {
                                            symbol_id: symbol.id,
                                            range: usage_range,
                                            scope,
                                            property_access: Some(prop_name.into()),
                                        };
                                        graph.add_usage(usage);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_for_property_accesses(language, child, source, graph);
        }
    }

    // =========================================================================
    // Phase 6: Process Reassignments
    // =========================================================================

    async fn process_reassignments(
        query_engine: &QueryEngine,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
        graph: &mut BindingGraph,
    ) {
        let reassignments = query_engine
            .extract_reassignments_with_positions(language, tree, source)
            .await;

        // First, collect all symbols that should be invalidated (to avoid borrow conflicts)
        let mut symbols_to_invalidate: Vec<SymbolId> = Vec::new();

        for (name, range) in &reassignments {
            let reassignment_scope = graph.scope_at_position(range.start);

            // Find symbols to invalidate:
            // 1. They have the same name
            // 2. The reassignment is in the same scope OR a parent scope of the symbol
            //    (i.e., the reassignment is visible from the symbol's scope)
            for symbol in graph.symbols() {
                if &symbol.name == name {
                    // Check if the reassignment scope is same or parent of symbol's scope
                    if Self::is_scope_visible(graph, symbol.scope, reassignment_scope) {
                        symbols_to_invalidate.push(symbol.id);
                    }
                }
            }
        }

        // Now apply the invalidations
        for symbol_id in symbols_to_invalidate {
            if let Some(symbol) = graph.get_symbol_mut(symbol_id) {
                symbol.is_valid = false;
            }
        }
    }

    /// Check if a scope is visible from another scope.
    /// Returns true if target_scope is the same as or an ancestor of from_scope.
    fn is_scope_visible(graph: &BindingGraph, from_scope: ScopeId, target_scope: ScopeId) -> bool {
        let mut current = Some(from_scope);
        while let Some(scope_id) = current {
            if scope_id == target_scope {
                return true;
            }
            current = graph.get_scope(scope_id).and_then(|s| s.parent);
        }
        false
    }

    // =========================================================================
    // Utilities
    // =========================================================================

    /// Convert tree-sitter Range to LSP Range.
    #[inline]
    pub fn ts_to_lsp_range(range: tree_sitter::Range) -> Range {
        Range::new(
            Position::new(
                range.start_point.row as u32,
                range.start_point.column as u32,
            ),
            Position::new(range.end_point.row as u32, range.end_point.column as u32),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::javascript::JavaScript;
    use crate::types::ScopeKind;

    #[test]
    fn test_ts_to_lsp_range() {
        let ts_range = tree_sitter::Range {
            start_byte: 0,
            end_byte: 10,
            start_point: tree_sitter::Point { row: 5, column: 10 },
            end_point: tree_sitter::Point { row: 5, column: 20 },
        };

        let lsp_range = AnalysisPipeline::ts_to_lsp_range(ts_range);

        assert_eq!(lsp_range.start.line, 5);
        assert_eq!(lsp_range.start.character, 10);
        assert_eq!(lsp_range.end.line, 5);
        assert_eq!(lsp_range.end.character, 20);
    }

    #[test]
    fn test_node_to_scope_kind() {
        // Use JavaScript as a concrete implementation to test the trait's default method
        let js = JavaScript;

        assert_eq!(
            js.node_to_scope_kind("function_declaration"),
            ScopeKind::Function
        );
        assert_eq!(js.node_to_scope_kind("arrow_function"), ScopeKind::Function);
        assert_eq!(
            js.node_to_scope_kind("class_declaration"),
            ScopeKind::Class
        );
        assert_eq!(js.node_to_scope_kind("for_statement"), ScopeKind::Loop);
        assert_eq!(
            js.node_to_scope_kind("if_statement"),
            ScopeKind::Conditional
        );
        assert_eq!(js.node_to_scope_kind("statement_block"), ScopeKind::Block);
    }
}
