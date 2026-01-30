use crate::analysis::binding_graph::BindingGraph;
use crate::analysis::query::QueryEngine;
use crate::languages::LanguageSupport;
use crate::types::{
    ImportContext, Scope, ScopeId, Symbol, SymbolId, SymbolKind, SymbolOrigin, SymbolUsage,
};
use compact_str::CompactString;
use tower_lsp::lsp_types::{Position, Range};
use tree_sitter::Tree;

#[derive(Debug)]
struct PropertyAccessCandidate {
    object_name: CompactString,

    property_name: CompactString,

    usage_range: Range,

    property_range: Range,

    object_position: Position,
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
        let mut graph = BindingGraph::new();

        let root_range = ts_to_lsp_range(tree.root_node().range());
        graph.set_root_range(root_range);

        let property_candidates =
            Self::extract_scopes_and_collect_property_accesses(language, tree, source, &mut graph);

        // Build scope tree early so scope_at_position works correctly during binding extraction
        graph.rebuild_scope_range_index();

        Self::extract_direct_references(
            query_engine,
            language,
            tree,
            source,
            import_context,
            &mut graph,
        )
        .await;

        Self::extract_bindings(query_engine, language, tree, source, &mut graph).await;

        Self::resolve_origins(&mut graph);

        Self::extract_usages(query_engine, language, tree, source, &mut graph).await;

        Self::process_property_access_candidates(&property_candidates, &mut graph);

        Self::process_reassignments(query_engine, language, tree, source, &mut graph).await;

        graph.rebuild_range_index();

        graph
    }

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
        let current_scope = if language.is_scope_node(node) && !language.is_root_node(node) {
            let scope_kind = language.node_to_scope_kind(node.kind());
            let scope = Scope {
                id: ScopeId::root(),
                parent: Some(parent_scope),
                range: ts_to_lsp_range(node.range()),
                kind: scope_kind,
            };
            graph.add_scope(scope)
        } else {
            parent_scope
        };

        if node.kind() == "member_expression" {
            if let Some(candidate) = Self::extract_member_expression_candidate(node, source) {
                candidates.push(candidate);
            }
        }

        if node.kind() == "subscript_expression" {
            if let Some(candidate) =
                Self::extract_subscript_expression_candidate(node, source, language)
            {
                candidates.push(candidate);
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk_combined(child, language, source, graph, current_scope, candidates);
        }
    }

    fn extract_member_expression_candidate(
        node: tree_sitter::Node,
        source: &[u8],
    ) -> Option<PropertyAccessCandidate> {
        let object = node.child_by_field_name("object")?;
        let property = node.child_by_field_name("property")?;

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

    fn extract_subscript_expression_candidate(
        node: tree_sitter::Node,
        source: &[u8],
        language: &dyn LanguageSupport,
    ) -> Option<PropertyAccessCandidate> {
        let object = node.child_by_field_name("object")?;
        let index = node.child_by_field_name("index")?;

        if object.kind() != "identifier" {
            return None;
        }

        if index.kind() != "string" {
            return None;
        }

        let obj_name = object.utf8_text(source).ok()?;
        let raw = index.utf8_text(source).ok()?;
        let prop_name = language.strip_quotes(raw);

        let index_range = index.range();
        let prop_range = Range {
            start: Position {
                line: index_range.start_point.row as u32,
                character: index_range.start_point.column as u32 + 1,
            },
            end: Position {
                line: index_range.end_point.row as u32,
                character: index_range.end_point.column as u32 - 1,
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

    fn process_property_access_candidates(
        candidates: &[PropertyAccessCandidate],
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

    async fn extract_bindings(
        query_engine: &QueryEngine,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
        graph: &mut BindingGraph,
    ) {
        let bindings = query_engine.extract_bindings(language, tree, source).await;

        for binding in bindings {
            let scope = graph.scope_at_position(binding.binding_range.start);

            let (origin, kind) = match binding.kind {
                crate::types::BindingKind::Object => {
                    let is_env_object = language
                        .default_env_object_name()
                        .map(|name| binding.env_var_name == name)
                        .unwrap_or(false);

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

        let assignments = query_engine
            .extract_assignments(language, tree, source)
            .await;

        for (target_name, target_range, source_name) in assignments {
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
                if let Some(sym) = graph.get_symbol_mut(symbol_id) {
                    sym.origin = SymbolOrigin::Symbol { target: target_id };
                }
            } else if let Some(sym) = graph.get_symbol_mut(symbol_id) {
                sym.origin = SymbolOrigin::UnresolvedSymbol { source_name };
            }
        }

        let destructures = query_engine
            .extract_destructures(language, tree, source)
            .await;

        for (target_name, target_range, key_name, key_range, source_name) in destructures {
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

            if let Some(sym) = graph.get_symbol_mut(symbol_id) {
                sym.origin = new_origin;
            }
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
                    if Self::is_scope_visible(graph, symbol.scope, reassignment_scope) {
                        symbols_to_invalidate.push(symbol_id);
                    }
                }
            }
        }

        for symbol_id in symbols_to_invalidate {
            if let Some(symbol) = graph.get_symbol_mut(symbol_id) {
                symbol.is_valid = false;
            }
        }
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
}
