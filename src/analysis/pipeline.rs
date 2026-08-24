use crate::analysis::graph::BindingGraph;
use crate::analysis::query::QueryEngine;
use crate::languages::LanguageSupport;
use crate::types::{
    ImportContext, PropertyAccess, Scope, ScopeId, Symbol, SymbolId, SymbolKind, SymbolOrigin,
    SymbolUsage,
};
use compact_str::CompactString;
use tower_lsp::lsp_types::{Position, Range};
use tree_sitter::Tree;

/// How much of the binding graph a caller needs.
///
/// Workspace indexing only asks "which env vars does this file touch?", while
/// interactive requests (hover, rename, inlay hints) need positional indices and
/// per-symbol usages. Building the latter for every file in a large repository is
/// the bulk of indexing cost, so indexing opts out of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisMode {
    /// Everything: symbol usages and all interval trees.
    Full,
    /// Only what is needed to enumerate a file's env vars and exports.
    Index,
}

impl AnalysisMode {
    #[inline]
    fn needs_usages(self) -> bool {
        matches!(self, AnalysisMode::Full)
    }
}

/// Flattened range, for cheap set membership.
#[inline]
fn range_key(range: Range) -> (u32, u32, u32, u32) {
    (
        range.start.line,
        range.start.character,
        range.end.line,
        range.end.character,
    )
}

pub struct AnalysisPipeline;

impl AnalysisPipeline {
    pub async fn analyze(
        query_engine: &QueryEngine,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
        import_context: &ImportContext,
    ) -> BindingGraph {
        Self::analyze_with_mode(
            query_engine,
            language,
            tree,
            source,
            import_context,
            AnalysisMode::Full,
        )
        .await
    }

    /// Analyze a file, building only the parts of the graph that `mode` requires.
    pub async fn analyze_with_mode(
        query_engine: &QueryEngine,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
        import_context: &ImportContext,
        mode: AnalysisMode,
    ) -> BindingGraph {
        let mut graph = BindingGraph::new();

        let root_range = ts_to_lsp_range(tree.root_node().range());
        graph.set_root_range(root_range);

        // The symbol-producing queries are scope-free, so run them first. If a
        // file declares no symbols at all -- the common case across a repository,
        // since the binding queries are anchored on `process.env` and friends --
        // there is nothing for a scope tree to be useful for.
        let bindings = query_engine.extract_bindings(language, tree, source).await;
        let assignments = query_engine
            .extract_assignments(language, tree, source)
            .await;
        let destructures = query_engine
            .extract_destructures(language, tree, source)
            .await;

        let has_symbols =
            !bindings.is_empty() || !assignments.is_empty() || !destructures.is_empty();

        // An env-object alias (`const e = process.env`) turns every `e.KEY` read
        // into an env var reference. Those are recorded as usages, so even index
        // mode has to collect property accesses when such a binding exists --
        // otherwise the file is indexed as referencing nothing.
        let has_env_object_binding = bindings
            .iter()
            .any(|b| b.kind == crate::types::BindingKind::Object);

        // Walking every node to build the scope tree is the single most expensive
        // step of indexing. Skip it when no symbol will ever be looked up in a
        // scope. `Full` mode still builds it: open documents answer positional
        // requests (completion, hover) against the scope index.
        let build_scopes = has_symbols || mode.needs_usages();

        let collect_property_accesses = mode.needs_usages() || has_env_object_binding;

        let property_candidates = if build_scopes {
            let candidates = Self::extract_scopes_and_collect_property_accesses(
                language,
                tree,
                source,
                &mut graph,
                collect_property_accesses,
            );
            graph.rebuild_scope_range_index();
            candidates
        } else {
            Vec::new()
        };

        Self::extract_direct_references(
            query_engine,
            language,
            tree,
            source,
            import_context,
            &mut graph,
        )
        .await;

        Self::apply_bindings(bindings, assignments, destructures, language, &mut graph);

        Self::resolve_origins(&mut graph);

        if mode.needs_usages() {
            Self::extract_usages(query_engine, language, tree, source, &mut graph).await;
        }

        if collect_property_accesses {
            Self::process_property_access_candidates(&property_candidates, &mut graph);
        }

        // Reassignments only invalidate existing symbols, so with no symbols the
        // query and the pass over its results are both dead work.
        if has_symbols {
            Self::process_reassignments(query_engine, language, tree, source, &mut graph).await;
        }

        if mode.needs_usages() {
            graph.rebuild_range_index();
        }

        graph
    }

    fn extract_scopes_and_collect_property_accesses(
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
        graph: &mut BindingGraph,
        collect_candidates: bool,
    ) -> Vec<PropertyAccess> {
        let mut candidates = Vec::new();
        Self::walk_combined(
            tree,
            language,
            source,
            graph,
            &mut candidates,
            collect_candidates,
        );
        candidates
    }

    /// Single-pass pre-order traversal that assigns scopes and (in `Full` mode)
    /// collects property-access candidates.
    ///
    /// Uses one reusable `TreeCursor` for the whole tree. The previous recursive
    /// version allocated and freed a cursor per node via `Node::walk()`, which
    /// dominated indexing profiles.
    fn walk_combined(
        tree: &Tree,
        language: &dyn LanguageSupport,
        source: &[u8],
        graph: &mut BindingGraph,
        candidates: &mut Vec<PropertyAccess>,
        collect_candidates: bool,
    ) {
        let mut cursor = tree.walk();
        // Scope stack: the scope in effect for the children at each depth.
        let mut scope_stack: Vec<ScopeId> = vec![ScopeId::root()];

        loop {
            let node = cursor.node();
            let parent_scope = *scope_stack.last().expect("scope stack is never empty");

            let current_scope = if language.is_scope_node(node) && !language.is_root_node(node) {
                let scope_kind = language.node_to_scope_kind(node.kind());
                graph.add_scope(Scope {
                    id: ScopeId::root(),
                    parent: Some(parent_scope),
                    range: ts_to_lsp_range(node.range()),
                    kind: scope_kind,
                })
            } else {
                parent_scope
            };

            if collect_candidates {
                if let Some(candidate) = language.property_access_at(node, source) {
                    candidates.push(candidate);
                }
            }

            if cursor.goto_first_child() {
                scope_stack.push(current_scope);
                continue;
            }

            // Leaf: advance to the next sibling, unwinding as far as needed.
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return;
                }
                scope_stack.pop();
            }
        }
    }

    fn process_property_access_candidates(
        candidates: &[PropertyAccess],
        graph: &mut BindingGraph,
    ) {
        for candidate in candidates {
            let scope = graph.scope_at_position(candidate.object_position);

            if let Some(symbol) = graph.lookup_symbol(&candidate.object_name, scope) {
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

    async fn extract_direct_references(
        query_engine: &QueryEngine,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
        import_context: &ImportContext,
        graph: &mut BindingGraph,
    ) {
        let references = query_engine
            .extract_references(language, tree, source, import_context)
            .await;

        for reference in references {
            graph.add_direct_reference(reference);
        }
    }

    /// Applies previously collected query results to the graph.
    ///
    /// Split from the queries themselves so callers can decide whether a scope
    /// tree is needed before any symbol is inserted.
    fn apply_bindings(
        bindings: Vec<crate::types::EnvBinding>,
        assignments: Vec<(CompactString, Range, CompactString)>,
        destructures: Vec<(CompactString, Range, CompactString, Range, CompactString)>,
        language: &dyn LanguageSupport,
        graph: &mut BindingGraph,
    ) {
        let mut bound_ranges: rustc_hash::FxHashSet<(u32, u32, u32, u32)> =
            rustc_hash::FxHashSet::default();

        for binding in &bindings {
            bound_ranges.insert(range_key(binding.binding_range));
        }

        for binding in bindings {
            let scope = graph.scope_at_position(binding.binding_range.start);

            let (origin, kind) = match binding.kind {
                crate::types::BindingKind::Object => {
                    // A language can expose several env objects (`process.env` and
                    // `import.meta.env`; `$_ENV` and `$_SERVER`), so ask whether
                    // this name is one of them rather than comparing against the
                    // single default.
                    let is_env_object = language
                        .is_standard_env_object(binding.env_var_name.as_str())
                        || language.default_env_object_name()
                            == Some(binding.env_var_name.as_str());

                    if is_env_object {
                        (
                            SymbolOrigin::EnvObject {
                                canonical_name: binding.env_var_name.clone(),
                            },
                            SymbolKind::EnvObject,
                        )
                    } else {
                        (
                            SymbolOrigin::EnvVar {
                                name: binding.env_var_name.clone(),
                            },
                            SymbolKind::DestructuredProperty,
                        )
                    }
                }
                crate::types::BindingKind::Value => (
                    SymbolOrigin::EnvVar {
                        name: binding.env_var_name.clone(),
                    },
                    SymbolKind::Value,
                ),
            };

            let symbol = Symbol {
                id: SymbolId::new(1).unwrap(),
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

        // Languages without a distinct declaration syntax (`$e = $_ENV;` in PHP)
        // match both the binding query and the generic assignment query at the
        // same site. The generic match resolves to nothing and, being added
        // later, would win `lookup_symbol`, so skip sites already bound.
        for (target_name, target_range, source_name) in assignments {
            if bound_ranges.contains(&range_key(target_range)) {
                continue;
            }
            let scope = graph.scope_at_position(target_range.start);

            let symbol = Symbol {
                id: SymbolId::new(1).unwrap(),
                name: target_name,
                declaration_range: target_range,
                name_range: target_range,
                scope,
                origin: SymbolOrigin::Unknown,
                kind: SymbolKind::Variable,
                is_valid: true,
                destructured_key_range: None,
            };

            let symbol_id = graph.add_symbol(symbol);

            let source_id = graph.lookup_symbol_id(&source_name, scope);
            if let Some(target_id) = source_id {
                graph.update_symbol_origin(symbol_id, SymbolOrigin::Symbol { target: target_id });
            } else {
                graph.update_symbol_origin(symbol_id, SymbolOrigin::UnresolvedSymbol { source_name });
            }
        }

        for (target_name, target_range, key_name, key_range, source_name) in destructures {
            if bound_ranges.contains(&range_key(target_range)) {
                continue;
            }
            let scope = graph.scope_at_position(target_range.start);

            let source_id = graph.lookup_symbol_id(&source_name, scope);

            let origin = if let Some(src_id) = source_id {
                SymbolOrigin::DestructuredProperty {
                    source: src_id,
                    key: key_name,
                }
            } else {
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
                destructured_key_range: Some(key_range),
            };

            graph.add_symbol(symbol);
        }
    }

    fn resolve_origins(graph: &mut BindingGraph) {
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

        for (symbol_id, scope, origin) in symbols_to_resolve {
            let new_origin = match origin {
                SymbolOrigin::UnresolvedSymbol { source_name } => graph
                    .lookup_symbol_id(&source_name, scope)
                    .map(|target| SymbolOrigin::Symbol { target })
                    .unwrap_or(SymbolOrigin::Unresolvable),
                SymbolOrigin::UnresolvedDestructure { source_name, key } => graph
                    .lookup_symbol_id(&source_name, scope)
                    .map(|source| SymbolOrigin::DestructuredProperty { source, key })
                    .unwrap_or(SymbolOrigin::Unresolvable),
                _ => continue,
            };

            graph.update_symbol_origin(symbol_id, new_origin);
        }
    }

    async fn extract_usages(
        query_engine: &QueryEngine,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
        graph: &mut BindingGraph,
    ) {
        let identifiers = query_engine
            .extract_identifiers(language, tree, source)
            .await;

        for (name, range) in identifiers {
            let scope = graph.scope_at_position(range.start);

            if let Some(symbol) = graph.lookup_symbol(&name, scope) {
                if (range.start.line > symbol.declaration_range.end.line || (range.start.line == symbol.declaration_range.end.line
                        && range.start.character > symbol.declaration_range.end.character)) && range != symbol.name_range {
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

        let mut symbols_to_invalidate: Vec<SymbolId> = Vec::new();

        for (name, range) in &reassignments {
            let reassignment_scope = graph.scope_at_position(range.start);

            // Use name-only index for O(1) lookup instead of scanning all symbols
            for symbol_id in graph.lookup_symbols_by_name(name) {
                if let Some(symbol) = graph.get_symbol(symbol_id) {
                    // In languages where a declaration *is* an assignment
                    // (`$e = $_ENV;`), the binding's own declaration matches the
                    // reassignment query. It must not invalidate itself.
                    if symbol.name_range == *range {
                        continue;
                    }

                    // An assignment before the declaration says nothing about the
                    // value the declaration binds.
                    if !Self::is_after(range.start, symbol.declaration_range.end) {
                        continue;
                    }

                    if Self::is_scope_visible(graph, symbol.scope, reassignment_scope) {
                        symbols_to_invalidate.push(symbol_id);
                    }
                }
            }
        }

        for symbol_id in symbols_to_invalidate {
            graph.invalidate_symbol(symbol_id);
        }
    }

    /// Whether `pos` lies strictly after `mark`.
    #[inline]
    fn is_after(pos: Position, mark: Position) -> bool {
        pos.line > mark.line || (pos.line == mark.line && pos.character > mark.character)
    }

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
    use crate::types::{ResolvedEnv, ScopeKind};

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
        let js = JavaScript;

        assert_eq!(
            js.node_to_scope_kind("function_declaration"),
            ScopeKind::Function
        );
        assert_eq!(js.node_to_scope_kind("arrow_function"), ScopeKind::Function);
        assert_eq!(js.node_to_scope_kind("class_declaration"), ScopeKind::Class);
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

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

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

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        assert_eq!(graph.direct_references().len(), 3);
    }

    #[tokio::test]
    async fn test_analyze_object_binding() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = "const env = process.env;";
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        assert!(!graph.symbols().is_empty());
        let env_symbol = graph.symbols().iter().find(|s| s.name == "env");
        assert!(env_symbol.is_some());

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

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        let db_symbol = graph.symbols().iter().find(|s| s.name == "DATABASE_URL");
        assert!(db_symbol.is_some());

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

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        let env_symbol = graph.symbols().iter().find(|s| s.name == "env");
        let config_symbol = graph.symbols().iter().find(|s| s.name == "config");
        assert!(env_symbol.is_some());
        assert!(config_symbol.is_some());

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

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        let api_symbol = graph.symbols().iter().find(|s| s.name == "API_KEY");
        assert!(api_symbol.is_some());

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

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        assert!(graph.scopes().len() >= 2);

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

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

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

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        let db_symbol = graph.symbols().iter().find(|s| s.name == "db");

        assert!(db_symbol.is_none() || !db_symbol.unwrap().is_valid);
    }

    #[tokio::test]
    async fn test_analyze_typescript() {
        let query_engine = QueryEngine::new();
        let ts = TypeScript;
        let code = "const db: string = process.env.DATABASE_URL || '';";
        let tree = parse_with_lang(&ts, code);
        let import_ctx = ImportContext::new();

        let graph =
            AnalysisPipeline::analyze(&query_engine, &ts, &tree, code.as_bytes(), &import_ctx)
                .await;

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

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

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

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

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

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        assert!(graph.scopes().len() >= 3);

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

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        let db_symbol = graph.symbols().iter().find(|s| s.name == "dbUrl");
        assert!(db_symbol.is_some());

        let db_symbol = db_symbol.unwrap();
        let resolved = graph.resolve_to_env(db_symbol.id);
        assert!(matches!(resolved, Some(ResolvedEnv::Variable(name)) if name == "DATABASE_URL"));

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

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        assert!(!graph.usages().is_empty());
        let usage = graph.usages().iter().find(|u| u.property_access.is_some());
        assert!(usage.is_some());
        assert_eq!(
            usage.unwrap().property_access.as_ref().unwrap(),
            "DATABASE_URL"
        );
    }

    #[test]
    fn test_is_scope_visible() {
        let mut graph = BindingGraph::new();
        graph.set_root_range(Range::new(Position::new(0, 0), Position::new(100, 0)));

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

        assert!(AnalysisPipeline::is_scope_visible(
            &graph,
            inner_scope,
            ScopeId::root()
        ));
        assert!(AnalysisPipeline::is_scope_visible(
            &graph,
            func_scope,
            ScopeId::root()
        ));

        assert!(AnalysisPipeline::is_scope_visible(
            &graph,
            inner_scope,
            func_scope
        ));

        assert!(AnalysisPipeline::is_scope_visible(
            &graph, func_scope, func_scope
        ));
    }


    #[tokio::test]
    async fn test_analyze_multiple_destructuring_same_line() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = "const { API_KEY, DB_URL, DEBUG } = process.env;";
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        // Should have 3 symbols for destructured properties
        let env_var_symbols: Vec<_> = graph
            .symbols()
            .iter()
            .filter(|s| matches!(&s.origin, SymbolOrigin::EnvVar { .. }))
            .collect();
        assert_eq!(env_var_symbols.len(), 3);
    }

    #[tokio::test]
    async fn test_analyze_deep_chain() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = r#"const env = process.env;
const cfg = env;
const settings = cfg;
const opts = settings;
const { PORT } = opts;"#;
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        let port = graph.symbols().iter().find(|s| s.name == "PORT");
        assert!(port.is_some());

        let port = port.unwrap();
        let resolved = graph.resolve_to_env(port.id);
        assert!(matches!(resolved, Some(ResolvedEnv::Variable(name)) if name == "PORT"));
    }

    #[tokio::test]
    async fn test_analyze_comments_ignored() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = r#"// process.env.COMMENTED
/* process.env.BLOCK_COMMENT */
const real = process.env.REAL_VAR;"#;
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        // Should only find REAL_VAR
        assert_eq!(graph.direct_references().len(), 1);
        assert_eq!(graph.direct_references()[0].name, "REAL_VAR");
    }

    #[tokio::test]
    async fn test_analyze_template_literal_env_access() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = r#"const url = `${process.env.BASE_URL}/api`;"#;
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        assert_eq!(graph.direct_references().len(), 1);
        assert_eq!(graph.direct_references()[0].name, "BASE_URL");
    }

    #[tokio::test]
    async fn test_analyze_ternary_env_access() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = "const val = process.env.VAR1 ? process.env.VAR1 : process.env.VAR2;";
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        // Should find VAR1 twice (condition and true branch) and VAR2 once
        let var1_refs: Vec<_> = graph
            .direct_references()
            .iter()
            .filter(|r| r.name == "VAR1")
            .collect();
        let var2_refs: Vec<_> = graph
            .direct_references()
            .iter()
            .filter(|r| r.name == "VAR2")
            .collect();
        assert_eq!(var1_refs.len(), 2);
        assert_eq!(var2_refs.len(), 1);
    }

    #[tokio::test]
    async fn test_analyze_logical_or_default() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = "const port = process.env.PORT || 3000;";
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        assert_eq!(graph.direct_references().len(), 1);
        assert_eq!(graph.direct_references()[0].name, "PORT");
    }

    #[tokio::test]
    async fn test_analyze_nullish_coalescing_default() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = "const port = process.env.PORT ?? 3000;";
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        assert_eq!(graph.direct_references().len(), 1);
        assert_eq!(graph.direct_references()[0].name, "PORT");
    }

    #[tokio::test]
    async fn test_analyze_arrow_function_scope() {
        let query_engine = QueryEngine::new();
        let js = JavaScript;
        let code = r#"const outer = process.env.OUTER;
const fn = () => {
    const inner = process.env.INNER;
};"#;
        let tree = parse_with_lang(&js, code);
        let import_ctx = ImportContext::new();

        let graph =
            AnalysisPipeline::analyze(&query_engine, &js, &tree, code.as_bytes(), &import_ctx)
                .await;

        assert_eq!(graph.direct_references().len(), 2);
        // Should have arrow function scope
        assert!(graph.scopes().len() >= 2);
    }
}
