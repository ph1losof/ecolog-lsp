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
use compact_str::CompactString;
use tower_lsp::lsp_types::{Position, Range};
use tree_sitter::Tree;

/// Candidate property access collected during tree walk, to be processed after symbol resolution.
#[derive(Debug)]
struct PropertyAccessCandidate {
    /// Object name (e.g., "env" in env.API_KEY)
    object_name: CompactString,
    /// Property name (e.g., "API_KEY" in env.API_KEY)
    property_name: CompactString,
    /// Range of the entire expression
    usage_range: Range,
    /// Range of just the property name
    property_range: Range,
    /// Position of the object (for scope lookup)
    object_position: Position,
}

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
        let root_range = ts_to_lsp_range(tree.root_node().range());
        graph.set_root_range(root_range);

        // Phase 1: Extract scopes AND collect property access candidates in a single tree walk
        // This optimization reduces tree traversals from 2 to 1 by combining:
        // - Scope extraction (Phase 1)
        // - Property access candidate collection (previously done in Phase 5)
        let property_candidates =
            Self::extract_scopes_and_collect_property_accesses(language, tree, source, &mut graph);

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

        // Phase 5: Extract usages (identifiers only, property accesses already collected)
        Self::extract_usages(query_engine, language, tree, source, &mut graph).await;

        // Phase 5b: Process collected property access candidates (now that symbol table is populated)
        Self::process_property_access_candidates(&property_candidates, &mut graph);

        // Phase 6: Process reassignments
        Self::process_reassignments(query_engine, language, tree, source, &mut graph).await;

        // Phase 7: Rebuild range index for fast destructure key lookups
        graph.rebuild_range_index();

        graph
    }

    // =========================================================================
    // Phase 1: Combined Scope Extraction and Property Access Collection
    // =========================================================================

    /// Combined tree walk that extracts scopes AND collects property access candidates.
    /// This optimization reduces tree traversals from 2 to 1.
    fn extract_scopes_and_collect_property_accesses(
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
        graph: &mut BindingGraph,
    ) -> Vec<PropertyAccessCandidate> {
        let mut candidates = Vec::new();
        Self::walk_combined(
            tree.root_node(),
            language,
            source,
            graph,
            ScopeId::root(),
            &mut candidates,
        );
        candidates
    }

    fn walk_combined(
        node: tree_sitter::Node,
        language: &dyn LanguageSupport,
        source: &[u8],
        graph: &mut BindingGraph,
        parent_scope: ScopeId,
        candidates: &mut Vec<PropertyAccessCandidate>,
    ) {
        // Check if this node creates a new scope
        // Skip root nodes (program/source_file/module) as they are already the root scope
        let current_scope = if language.is_scope_node(node) && !language.is_root_node(node) {
            let scope_kind = language.node_to_scope_kind(node.kind());
            let scope = Scope {
                id: ScopeId::root(), // Placeholder, will be overwritten by add_scope
                parent: Some(parent_scope),
                range: ts_to_lsp_range(node.range()),
                kind: scope_kind,
            };
            graph.add_scope(scope)
        } else {
            parent_scope
        };

        // Collect property access candidates (member_expression: obj.property)
        if node.kind() == "member_expression" {
            if let Some(candidate) = Self::extract_member_expression_candidate(node, source) {
                candidates.push(candidate);
            }
        }

        // Collect subscript expression candidates (obj["property"])
        if node.kind() == "subscript_expression" {
            if let Some(candidate) =
                Self::extract_subscript_expression_candidate(node, source, language)
            {
                candidates.push(candidate);
            }
        }

        // Recurse to children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_combined(child, language, source, graph, current_scope, candidates);
        }
    }

    /// Extract a property access candidate from a member_expression node.
    fn extract_member_expression_candidate(
        node: tree_sitter::Node,
        source: &[u8],
    ) -> Option<PropertyAccessCandidate> {
        let object = node.child_by_field_name("object")?;
        let property = node.child_by_field_name("property")?;

        // Only handle simple identifier objects
        if object.kind() != "identifier" {
            return None;
        }

        let obj_name = object.utf8_text(source).ok()?;
        let prop_name = property.utf8_text(source).ok()?;

        Some(PropertyAccessCandidate {
            object_name: obj_name.into(),
            property_name: prop_name.into(),
            usage_range: ts_to_lsp_range(node.range()),
            property_range: ts_to_lsp_range(property.range()),
            object_position: Position::new(
                object.start_position().row as u32,
                object.start_position().column as u32,
            ),
        })
    }

    /// Extract a property access candidate from a subscript_expression node.
    fn extract_subscript_expression_candidate(
        node: tree_sitter::Node,
        source: &[u8],
        language: &dyn LanguageSupport,
    ) -> Option<PropertyAccessCandidate> {
        let object = node.child_by_field_name("object")?;
        let index = node.child_by_field_name("index")?;

        // Only handle simple identifier objects
        if object.kind() != "identifier" {
            return None;
        }

        // Only handle string literal indices
        if index.kind() != "string" {
            return None;
        }

        let obj_name = object.utf8_text(source).ok()?;
        let raw = index.utf8_text(source).ok()?;
        let prop_name = language.strip_quotes(raw);

        // Calculate the range of just the string content (without quotes)
        let index_range = index.range();
        let prop_range = Range {
            start: Position {
                line: index_range.start_point.row as u32,
                character: index_range.start_point.column as u32 + 1, // skip opening quote
            },
            end: Position {
                line: index_range.end_point.row as u32,
                character: index_range.end_point.column as u32 - 1, // skip closing quote
            },
        };

        Some(PropertyAccessCandidate {
            object_name: obj_name.into(),
            property_name: prop_name.into(),
            usage_range: ts_to_lsp_range(node.range()),
            property_range: prop_range,
            object_position: Position::new(
                object.start_position().row as u32,
                object.start_position().column as u32,
            ),
        })
    }

    /// Process collected property access candidates after symbol table is populated.
    fn process_property_access_candidates(
        candidates: &[PropertyAccessCandidate],
        graph: &mut BindingGraph,
    ) {
        for candidate in candidates {
            let scope = graph.scope_at_position(candidate.object_position);

            // Look up the object symbol
            if let Some(symbol) = graph.lookup_symbol(&candidate.object_name, scope) {
                // Check if this symbol resolves to an env object
                if graph.resolves_to_env_object(symbol.id) {
                    let usage = SymbolUsage {
                        symbol_id: symbol.id,
                        range: candidate.usage_range,
                        scope,
                        property_access: Some(candidate.property_name.clone()),
                        property_access_range: Some(candidate.property_range),
                    };
                    graph.add_usage(usage);
                }
            }
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
        // Collect symbols needing resolution (to avoid borrow issues)
        // Uses BindingGraph's existing name_index via lookup_symbol_id instead of
        // creating a temporary HashMap, reducing memory churn.
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

        // Resolve each unresolved symbol using graph's lookup methods
        for (symbol_id, scope, origin) in symbols_to_resolve {
            let new_origin = match origin {
                SymbolOrigin::UnresolvedSymbol { source_name } => {
                    // Use BindingGraph's built-in lookup which walks the scope chain
                    graph
                        .lookup_symbol_id(&source_name, scope)
                        .map(|target| SymbolOrigin::Symbol { target })
                        .unwrap_or(SymbolOrigin::Unresolvable)
                }
                SymbolOrigin::UnresolvedDestructure { source_name, key } => {
                    // Use BindingGraph's built-in lookup which walks the scope chain
                    graph
                        .lookup_symbol_id(&source_name, scope)
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
                            property_access_range: None,
                        };
                        graph.add_usage(usage);
                    }
                }
            }
        }

        // Note: Property access handling (env.VAR, env["VAR"]) is now done via
        // candidates collected during Phase 1's combined tree walk and processed
        // in Phase 5b (process_property_access_candidates).
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

}

// =============================================================================
// Utilities
// =============================================================================

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::QueryEngine;
    use crate::languages::javascript::JavaScript;
    use crate::languages::typescript::TypeScript;
    use crate::languages::LanguageSupport;
    use crate::types::{ScopeKind, ResolvedEnv};

    #[test]
    fn test_ts_to_lsp_range() {
        let ts_range = tree_sitter::Range {
            start_byte: 0,
            end_byte: 10,
            start_point: tree_sitter::Point { row: 5, column: 10 },
            end_point: tree_sitter::Point { row: 5, column: 20 },
        };

        let lsp_range = ts_to_lsp_range(ts_range);

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

    fn parse_with_lang<L: LanguageSupport>(lang: &L, code: &str) -> Tree {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang.grammar()).unwrap();
        parser.parse(code, None).unwrap()
    }

    #[tokio::test]
    async fn test_analyze_direct_reference() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = "const db = process.env.DATABASE_URL;";
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph = AnalysisPipeline::analyze(
            &query_engine,
            &js,
            &tree,
            code.as_bytes(),
            &import_ctx,
        ).await;

        // Should have one direct reference
        assert_eq!(graph.direct_references().len(), 1);
        assert_eq!(graph.direct_references()[0].name, "DATABASE_URL");
    }

    #[tokio::test]
    async fn test_analyze_multiple_references() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = r#"const db = process.env.DATABASE_URL;
const api = process.env.API_KEY;
const secret = process.env.SECRET;"#;
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph = AnalysisPipeline::analyze(
            &query_engine,
            &js,
            &tree,
            code.as_bytes(),
            &import_ctx,
        ).await;

        assert_eq!(graph.direct_references().len(), 3);
    }

    #[tokio::test]
    async fn test_analyze_object_binding() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = "const env = process.env;";
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph = AnalysisPipeline::analyze(
            &query_engine,
            &js,
            &tree,
            code.as_bytes(),
            &import_ctx,
        ).await;

        // Should have a symbol for 'env'
        assert!(!graph.symbols().is_empty());
        let env_symbol = graph.symbols().iter().find(|s| s.name == "env");
        assert!(env_symbol.is_some());

        // Should resolve to env object
        let env_symbol = env_symbol.unwrap();
        let resolved = graph.resolve_to_env(env_symbol.id);
        assert!(matches!(resolved, Some(ResolvedEnv::Object(_))));
    }

    #[tokio::test]
    async fn test_analyze_destructuring() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = "const { DATABASE_URL } = process.env;";
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph = AnalysisPipeline::analyze(
            &query_engine,
            &js,
            &tree,
            code.as_bytes(),
            &import_ctx,
        ).await;

        // Should have a symbol for DATABASE_URL
        let db_symbol = graph.symbols().iter().find(|s| s.name == "DATABASE_URL");
        assert!(db_symbol.is_some());

        // Should resolve to env var
        let db_symbol = db_symbol.unwrap();
        let resolved = graph.resolve_to_env(db_symbol.id);
        assert!(matches!(resolved, Some(ResolvedEnv::Variable(name)) if name == "DATABASE_URL"));
    }

    #[tokio::test]
    async fn test_analyze_chain_binding() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = r#"const env = process.env;
const config = env;"#;
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph = AnalysisPipeline::analyze(
            &query_engine,
            &js,
            &tree,
            code.as_bytes(),
            &import_ctx,
        ).await;

        // Should have symbols for both env and config
        let env_symbol = graph.symbols().iter().find(|s| s.name == "env");
        let config_symbol = graph.symbols().iter().find(|s| s.name == "config");
        assert!(env_symbol.is_some());
        assert!(config_symbol.is_some());

        // config should resolve through chain to env object
        let config_symbol = config_symbol.unwrap();
        let resolved = graph.resolve_to_env(config_symbol.id);
        assert!(matches!(resolved, Some(ResolvedEnv::Object(_))));
    }

    #[tokio::test]
    async fn test_analyze_destructure_from_chain() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = r#"const env = process.env;
const { API_KEY } = env;"#;
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph = AnalysisPipeline::analyze(
            &query_engine,
            &js,
            &tree,
            code.as_bytes(),
            &import_ctx,
        ).await;

        // Should have symbol for API_KEY
        let api_symbol = graph.symbols().iter().find(|s| s.name == "API_KEY");
        assert!(api_symbol.is_some());

        // Should resolve to env var
        let api_symbol = api_symbol.unwrap();
        let resolved = graph.resolve_to_env(api_symbol.id);
        assert!(matches!(resolved, Some(ResolvedEnv::Variable(name)) if name == "API_KEY"));
    }

    #[tokio::test]
    async fn test_analyze_scopes() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = r#"function test() {
    const db = process.env.DATABASE_URL;
}
const api = process.env.API_KEY;"#;
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph = AnalysisPipeline::analyze(
            &query_engine,
            &js,
            &tree,
            code.as_bytes(),
            &import_ctx,
        ).await;

        // Should have multiple scopes (root + function)
        assert!(graph.scopes().len() >= 2);

        // Should have references
        assert_eq!(graph.direct_references().len(), 2);
    }

    #[tokio::test]
    async fn test_analyze_usages() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = r#"const env = process.env;
console.log(env.DATABASE_URL);"#;
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph = AnalysisPipeline::analyze(
            &query_engine,
            &js,
            &tree,
            code.as_bytes(),
            &import_ctx,
        ).await;

        // Should have usages for property access on env
        assert!(!graph.usages().is_empty());
    }

    #[tokio::test]
    async fn test_analyze_reassignment_invalidates() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = r#"let db = process.env.DATABASE_URL;
db = "new_value";"#;
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph = AnalysisPipeline::analyze(
            &query_engine,
            &js,
            &tree,
            code.as_bytes(),
            &import_ctx,
        ).await;

        // The db binding should be invalidated by reassignment
        // (Check the is_valid flag)
        let db_symbol = graph.symbols().iter().find(|s| s.name == "db");
        // Depending on implementation, the binding may or may not exist in symbols
        // What we're testing is that reassignment tracking works
        assert!(db_symbol.is_none() || !db_symbol.unwrap().is_valid);
    }

    #[tokio::test]
    async fn test_analyze_typescript() {
        let query_engine = QueryEngine::new();
        let ts = TypeScript;
        let code = "const db: string = process.env.DATABASE_URL || '';";
        let tree = parse_with_lang(&ts, code);
        let import_ctx = ImportContext::new();

        let graph = AnalysisPipeline::analyze(
            &query_engine,
            &ts,
            &tree,
            code.as_bytes(),
            &import_ctx,
        ).await;

        assert_eq!(graph.direct_references().len(), 1);
        assert_eq!(graph.direct_references()[0].name, "DATABASE_URL");
    }

    #[tokio::test]
    async fn test_analyze_empty_source() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = "";
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph = AnalysisPipeline::analyze(
            &query_engine,
            &js,
            &tree,
            code.as_bytes(),
            &import_ctx,
        ).await;

        assert!(graph.direct_references().is_empty());
        assert!(graph.symbols().is_empty());
    }

    #[tokio::test]
    async fn test_analyze_no_env_vars() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = "const x = 1 + 2; const y = 'hello';";
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph = AnalysisPipeline::analyze(
            &query_engine,
            &js,
            &tree,
            code.as_bytes(),
            &import_ctx,
        ).await;

        assert!(graph.direct_references().is_empty());
    }

    #[tokio::test]
    async fn test_analyze_nested_functions() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = r#"function outer() {
    const env = process.env;
    function inner() {
        const db = env.DATABASE_URL;
    }
}"#;
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph = AnalysisPipeline::analyze(
            &query_engine,
            &js,
            &tree,
            code.as_bytes(),
            &import_ctx,
        ).await;

        // Should have multiple scopes
        assert!(graph.scopes().len() >= 3); // root + outer + inner

        // Should have env binding
        let env_symbol = graph.symbols().iter().find(|s| s.name == "env");
        assert!(env_symbol.is_some());
    }

    #[tokio::test]
    async fn test_analyze_destructure_with_rename() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = "const { DATABASE_URL: dbUrl } = process.env;";
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph = AnalysisPipeline::analyze(
            &query_engine,
            &js,
            &tree,
            code.as_bytes(),
            &import_ctx,
        ).await;

        // Should have symbol for dbUrl
        let db_symbol = graph.symbols().iter().find(|s| s.name == "dbUrl");
        assert!(db_symbol.is_some());

        // Should resolve to DATABASE_URL env var
        let db_symbol = db_symbol.unwrap();
        let resolved = graph.resolve_to_env(db_symbol.id);
        assert!(matches!(resolved, Some(ResolvedEnv::Variable(name)) if name == "DATABASE_URL"));

        // Should have destructured key range
        assert!(db_symbol.destructured_key_range.is_some());
    }

    #[tokio::test]
    async fn test_analyze_subscript_access() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = r#"const env = process.env;
const db = env["DATABASE_URL"];"#;
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph = AnalysisPipeline::analyze(
            &query_engine,
            &js,
            &tree,
            code.as_bytes(),
            &import_ctx,
        ).await;

        // Should have usages for subscript access
        assert!(!graph.usages().is_empty());
        let usage = graph.usages().iter().find(|u| u.property_access.is_some());
        assert!(usage.is_some());
        assert_eq!(usage.unwrap().property_access.as_ref().unwrap(), "DATABASE_URL");
    }

    #[test]
    fn test_is_scope_visible() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(Range::new(Position::new(0, 0), Position::new(100, 0)));

        // Add nested scopes
        let func_scope = graph.add_scope(Scope {
            id: ScopeId::root(),
            parent: Some(ScopeId::root()),
            range: Range::new(Position::new(1, 0), Position::new(10, 0)),
            kind: ScopeKind::Function,
        });

        let inner_scope = graph.add_scope(Scope {
            id: ScopeId::root(),
            parent: Some(func_scope),
            range: Range::new(Position::new(2, 0), Position::new(8, 0)),
            kind: ScopeKind::Block,
        });

        // Root is visible from any scope
        assert!(AnalysisPipeline::is_scope_visible(&graph, inner_scope, ScopeId::root()));
        assert!(AnalysisPipeline::is_scope_visible(&graph, func_scope, ScopeId::root()));

        // Parent scopes are visible from child
        assert!(AnalysisPipeline::is_scope_visible(&graph, inner_scope, func_scope));

        // Scope is visible from itself
        assert!(AnalysisPipeline::is_scope_visible(&graph, func_scope, func_scope));
    }
}
