use crate::analysis::ts_to_lsp_range;
use crate::languages::LanguageSupport;
use crate::types::{
    AccessType, EnvReference, ExportResolution, FileExportEntry, ImportContext, ModuleExport,
};
use compact_str::CompactString;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::Range as LspRange;
use tree_sitter::{Parser, Query, QueryCursor, QueryMatch, Tree};

const MAX_PARSERS_PER_LANGUAGE: usize = 4;

const MAX_CURSORS: usize = 8;

pub struct ParserPool {
    parsers: HashMap<&'static str, Vec<Parser>>,
}

impl ParserPool {
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
        }
    }

    pub fn acquire(&mut self, language: &dyn LanguageSupport) -> Parser {
        if let Some(parsers) = self.parsers.get_mut(language.id()) {
            if let Some(parser) = parsers.pop() {
                return parser;
            }
        }

        let mut parser = Parser::new();
        parser
            .set_language(&language.grammar())
            .expect("Failed to set language");
        parser
    }

    pub fn release(&mut self, language_id: &'static str, mut parser: Parser) {
        parser.reset();
        let parsers = self.parsers.entry(language_id).or_default();

        if parsers.len() < MAX_PARSERS_PER_LANGUAGE {
            parsers.push(parser);
        }
    }
}

pub struct QueryEngine {
    parser_pool: Arc<Mutex<ParserPool>>,

    cursor_pool: Arc<Mutex<Vec<QueryCursor>>>,
}

impl QueryEngine {
    pub fn new() -> Self {
        Self {
            parser_pool: Arc::new(Mutex::new(ParserPool::new())),
            cursor_pool: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn parse(
        &self,
        language: &dyn LanguageSupport,
        content: &str,
        old_tree: Option<&Tree>,
    ) -> Option<Tree> {
        let mut parser = {
            let mut pool = self.parser_pool.lock().await;
            pool.acquire(language)
        };

        let language_id = language.id();

        let tree = parser.parse(content, old_tree);

        {
            let mut pool = self.parser_pool.lock().await;
            pool.release(language_id, parser);
        }

        tree
    }

    pub async fn execute_query<'a, F, T>(
        &self,
        query: &Query,
        tree: &'a Tree,
        source: &'a [u8],
        mut extractor: F,
    ) -> Vec<T>
    where
        F: FnMut(&QueryMatch<'_, 'a>, &[u8]) -> Option<T>,
    {
        let mut cursor_guard = self.cursor_pool.lock().await;
        let mut cursor = cursor_guard.pop().unwrap_or_else(QueryCursor::new);
        drop(cursor_guard);

        let mut results = Vec::new();

        {
            use streaming_iterator::StreamingIterator;
            let mut matches = cursor.matches(query, tree.root_node(), source);
            while let Some(m) = matches.next() {
                if let Some(item) = extractor(m, source) {
                    results.push(item);
                }
            }
        }

        let mut cursor_guard = self.cursor_pool.lock().await;
        if cursor_guard.len() < MAX_CURSORS {
            cursor_guard.push(cursor);
        }

        results
    }

    pub async fn extract_references(
        &self,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
        import_ctx: &ImportContext,
    ) -> Vec<EnvReference> {
        let query = language.reference_query();

        let idx_env_access = query.capture_index_for_name("env_access");
        let idx_env_var_name = query.capture_index_for_name("env_var_name");
        let idx_env_default_value = query.capture_index_for_name("env_default_value");
        let idx_object = query.capture_index_for_name("object");
        let idx_module = query.capture_index_for_name("module");

        self.execute_query(query, tree, source, |m, src| {
            let mut full_range = None;
            let mut name_range = None;
            let mut var_name = None;
            let mut _default_value: Option<CompactString> = None;
            let mut object_name: Option<CompactString> = None;
            let access_type = AccessType::Property;

            for capture in m.captures {
                let idx = Some(capture.index);

                if idx == idx_env_access {
                    full_range = Some(capture.node.range());
                } else if idx == idx_env_var_name {
                    name_range = Some(capture.node.range());
                    var_name = language.extract_var_name(capture.node, src);
                } else if idx == idx_env_default_value {
                    if let Ok(text) = capture.node.utf8_text(src) {
                        let clean_text = language.strip_quotes(text);
                        _default_value = Some(CompactString::from(clean_text));
                    }
                } else if idx == idx_object || idx == idx_module {
                    object_name = capture
                        .node
                        .utf8_text(src)
                        .ok()
                        .map(|s| CompactString::from(s));
                }
            }

            if let (Some(full), Some(name_r), Some(name)) = (full_range, name_range, var_name) {
                if let Some(obj) = object_name {
                    let is_std = language.is_standard_env_object(&obj);

                    if !is_std {
                        let mut is_valid_alias = false;
                        if let Some((module, _orig)) = import_ctx.aliases.get(&obj) {
                            if language.known_env_modules().contains(&module.as_str()) {
                                is_valid_alias = true;
                            }
                        }

                        if !is_valid_alias {
                            return None;
                        }
                    }
                }

                let full_lsp = ts_to_lsp_range(full);
                let name_lsp = ts_to_lsp_range(name_r);

                Some(EnvReference {
                    name,
                    full_range: full_lsp,
                    name_range: name_lsp,
                    access_type,
                    has_default: _default_value.is_some(),
                    default_value: _default_value,
                })
            } else {
                None
            }
        })
        .await
    }

    pub async fn check_completion_context(
        &self,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
        position: tower_lsp::lsp_types::Position,
    ) -> Option<CompactString> {
        let query = match language.completion_query() {
            Some(q) => q,
            None => return None,
        };

        let point = tree_sitter::Point::new(position.line as usize, position.character as usize);

        let idx_completion_target = query.capture_index_for_name("completion_target");
        let idx_object = query.capture_index_for_name("object");

        let object_name = self
            .execute_query(query, tree, source, |m, src| {
                let mut is_target = false;
                let mut obj_name = None;

                for capture in m.captures {
                    let idx = Some(capture.index);

                    if idx == idx_completion_target {
                        let start = capture.node.start_position();
                        let end = capture.node.end_position();

                        let valid_end = if point.row == end.row {
                            point.column <= end.column + 1
                        } else {
                            point <= end
                        };

                        if point >= start && valid_end {
                            is_target = true;
                        }
                    } else if idx == idx_object {
                        let node_text = capture.node.utf8_text(src).ok();

                        if let Some(text) = &node_text {
                            if text.contains('\n') {
                                continue;
                            }
                        }

                        if let Some(text) = &node_text {
                            if !language.is_standard_env_object(text) {
                                if let Some(parent) = capture.node.parent() {
                                    if parent.kind() == "member_expression" {
                                        if let Some(parent_text) = parent.utf8_text(src).ok() {
                                            if !parent_text.contains('\n')
                                                && language.is_standard_env_object(parent_text)
                                            {
                                                obj_name = Some(CompactString::from(parent_text));
                                                continue;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        obj_name = node_text.map(|s| CompactString::from(s));
                    }
                }

                if is_target {
                    obj_name
                } else {
                    None
                }
            })
            .await;

        if !object_name.is_empty() {
            object_name.into_iter().next()
        } else {
            None
        }
    }

    pub async fn extract_bindings(
        &self,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
    ) -> Vec<crate::types::EnvBinding> {
        let query = match language.binding_query() {
            Some(q) => q,
            None => return Vec::new(),
        };

        let idx_binding_name = query.capture_index_for_name("binding_name");
        let idx_bound_env_var = query.capture_index_for_name("bound_env_var");
        let idx_env_binding = query.capture_index_for_name("env_binding");
        let idx_env_object_binding = query.capture_index_for_name("env_object_binding");

        self.execute_query(query, tree, source, |m, src| {
            let mut binding_name: Option<CompactString> = None;
            let mut env_var_name: Option<CompactString> = None;
            let mut binding_range = None;
            let mut declaration_range = None;
            let mut is_object_binding = false;
            let mut bound_env_var_range = None;

            for capture in m.captures {
                let idx = Some(capture.index);

                if idx == idx_binding_name {
                    binding_range = Some(capture.node.range());
                    binding_name = language.extract_identifier(capture.node, src);
                } else if idx == idx_bound_env_var {
                    env_var_name = language.extract_var_name(capture.node, src);

                    bound_env_var_range = Some(capture.node.range());
                } else if idx == idx_env_binding {
                    declaration_range = Some(capture.node.range());
                } else if idx == idx_env_object_binding {
                    declaration_range = Some(capture.node.range());
                    is_object_binding = true;
                }
            }

            if is_object_binding && env_var_name.is_none() {
                if let Some(default_obj) = language.default_env_object_name() {
                    env_var_name = Some(default_obj.into());
                }
            }

            if let (Some(bind_name), Some(env_name), Some(bind_r), Some(decl_r)) =
                (binding_name, env_var_name, binding_range, declaration_range)
            {
                let binding_lsp = ts_to_lsp_range(bind_r);
                let decl_lsp = ts_to_lsp_range(decl_r);

                let mut scope_range = LspRange::default();
                let maybe_node = tree
                    .root_node()
                    .descendant_for_byte_range(bind_r.start_byte, bind_r.end_byte);

                if let Some(mut node) = maybe_node {
                    let mut found_scope = false;
                    while let Some(parent) = node.parent() {
                        if language.is_scope_node(parent) {
                            scope_range = ts_to_lsp_range(parent.range());
                            found_scope = true;
                            break;
                        }
                        node = parent;
                    }
                    if !found_scope {
                        scope_range = ts_to_lsp_range(tree.root_node().range());
                    }
                }

                let kind = if is_object_binding {
                    crate::types::BindingKind::Object
                } else {
                    crate::types::BindingKind::Value
                };

                let destructured_key_range = bound_env_var_range.map(ts_to_lsp_range);

                Some(crate::types::EnvBinding {
                    binding_name: bind_name,
                    env_var_name: env_name,
                    binding_range: binding_lsp,
                    declaration_range: decl_lsp,
                    scope_range,
                    is_valid: true,
                    kind,
                    destructured_key_range,
                })
            } else {
                None
            }
        })
        .await
    }

    pub async fn extract_imports(
        &self,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
    ) -> Vec<crate::types::ImportAlias> {
        let query = match language.import_query() {
            Some(q) => q,
            None => return Vec::new(),
        };

        let idx_import_path = query.capture_index_for_name("import_path");
        let idx_original_name = query.capture_index_for_name("original_name");
        let idx_alias_name = query.capture_index_for_name("alias_name");
        let idx_import_stmt = query.capture_index_for_name("import_stmt");

        self.execute_query(query, tree, source, |m, src| {
            let mut module_path: Option<CompactString> = None;
            let mut original_name: Option<CompactString> = None;
            let mut alias: Option<CompactString> = None;
            let mut stmt_range = None;

            for capture in m.captures {
                let idx = Some(capture.index);

                if idx == idx_import_path {
                    module_path = capture
                        .node
                        .utf8_text(src)
                        .ok()
                        .map(|s| CompactString::from(language.strip_quotes(s)));
                } else if idx == idx_original_name {
                    original_name = capture
                        .node
                        .utf8_text(src)
                        .ok()
                        .map(|s| CompactString::from(s));
                } else if idx == idx_alias_name {
                    alias = capture
                        .node
                        .utf8_text(src)
                        .ok()
                        .map(|s| CompactString::from(s));
                } else if idx == idx_import_stmt {
                    stmt_range = Some(capture.node.range());
                }
            }

            let orig = original_name.or_else(|| module_path.clone());

            if let (Some(path), Some(orig_name), Some(range)) = (module_path, orig, stmt_range) {
                Some(crate::types::ImportAlias {
                    module_path: path,
                    original_name: orig_name,
                    alias,
                    range: ts_to_lsp_range(range),
                })
            } else {
                None
            }
        })
        .await
    }

    pub async fn extract_reassignments(
        &self,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
    ) -> std::collections::HashSet<CompactString> {
        let query = match language.reassignment_query() {
            Some(q) => q,
            None => return std::collections::HashSet::new(),
        };

        let idx_reassigned_name = query.capture_index_for_name("reassigned_name");

        let reassignments = self
            .execute_query(query, tree, source, |m, src| {
                for capture in m.captures {
                    if Some(capture.index) == idx_reassigned_name {
                        return capture
                            .node
                            .utf8_text(src)
                            .ok()
                            .map(|s| CompactString::from(s));
                    }
                }
                None
            })
            .await;

        reassignments.into_iter().collect()
    }

    pub async fn extract_reassignments_with_positions(
        &self,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
    ) -> Vec<(CompactString, tower_lsp::lsp_types::Range)> {
        let query = match language.reassignment_query() {
            Some(q) => q,
            None => return Vec::new(),
        };

        let idx_reassigned_name = query.capture_index_for_name("reassigned_name");

        self.execute_query(query, tree, source, |m, src| {
            for capture in m.captures {
                if Some(capture.index) == idx_reassigned_name {
                    let name = capture.node.utf8_text(src).ok()?;
                    return Some((
                        CompactString::from(name),
                        ts_to_lsp_range(capture.node.range()),
                    ));
                }
            }
            None
        })
        .await
    }

    pub async fn extract_identifiers(
        &self,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
    ) -> Vec<(CompactString, tower_lsp::lsp_types::Range)> {
        let query = match language.identifier_query() {
            Some(q) => q,
            None => return Vec::new(),
        };

        let idx_identifier = query.capture_index_for_name("identifier");

        self.execute_query(query, tree, source, |m, src| {
            for capture in m.captures {
                if Some(capture.index) == idx_identifier {
                    if let Some(name) = language.extract_identifier(capture.node, src) {
                        return Some((name, ts_to_lsp_range(capture.node.range())));
                    }
                }
            }
            None
        })
        .await
    }

    pub async fn extract_assignments(
        &self,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
    ) -> Vec<(CompactString, tower_lsp::lsp_types::Range, CompactString)> {
        let query = match language.assignment_query() {
            Some(q) => q,
            None => return Vec::new(),
        };

        let idx_target = query.capture_index_for_name("assignment_target");
        let idx_source = query.capture_index_for_name("assignment_source");

        self.execute_query(query, tree, source, |m, src| {
            let mut target_name = None;
            let mut target_range = None;
            let mut source_name = None;

            for capture in m.captures {
                if Some(capture.index) == idx_target {
                    if let Some(name) = language.extract_identifier(capture.node, src) {
                        target_name = Some(name);
                        target_range = Some(ts_to_lsp_range(capture.node.range()));
                    }
                } else if Some(capture.index) == idx_source {
                    if let Some(name) = language.extract_identifier(capture.node, src) {
                        source_name = Some(name);
                    }
                }
            }

            match (target_name, target_range, source_name) {
                (Some(t), Some(r), Some(s)) => Some((t, r, s)),
                _ => None,
            }
        })
        .await
    }

    pub async fn extract_destructures(
        &self,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
    ) -> Vec<(
        CompactString,
        tower_lsp::lsp_types::Range,
        CompactString,
        tower_lsp::lsp_types::Range,
        CompactString,
    )> {
        let query = match language.destructure_query() {
            Some(q) => q,
            None => return Vec::new(),
        };

        let idx_target = query.capture_index_for_name("destructure_target");
        let idx_key = query.capture_index_for_name("destructure_key");
        let idx_source = query.capture_index_for_name("destructure_source");

        self.execute_query(query, tree, source, |m, src| {
            let mut target_name = None;
            let mut target_range = None;
            let mut key_name = None;
            let mut key_range = None;
            let mut source_name = None;

            for capture in m.captures {
                if Some(capture.index) == idx_target {
                    if let Some(name) = language.extract_identifier(capture.node, src) {
                        target_name = Some(name);
                        target_range = Some(ts_to_lsp_range(capture.node.range()));
                    }
                } else if Some(capture.index) == idx_key {
                    key_name = language.extract_destructure_key(capture.node, src);

                    key_range = Some(ts_to_lsp_range(capture.node.range()));
                } else if Some(capture.index) == idx_source {
                    if let Some(name) = language.extract_identifier(capture.node, src) {
                        source_name = Some(name);
                    }
                }
            }

            match (target_name, target_range, key_name, key_range, source_name) {
                (Some(t), Some(r), Some(k), Some(kr), Some(s)) => Some((t, r, k, kr, s)),
                _ => None,
            }
        })
        .await
    }

    pub async fn extract_exports(
        &self,
        language: &dyn LanguageSupport,
        tree: &Tree,
        source: &[u8],
    ) -> FileExportEntry {
        let query = match language.export_query() {
            Some(q) => q,
            None => return FileExportEntry::new(),
        };

        let idx_export_name = query.capture_index_for_name("export_name");
        let idx_export_value = query.capture_index_for_name("export_value");
        let idx_local_name = query.capture_index_for_name("local_name");
        let idx_reexport_source = query.capture_index_for_name("reexport_source");
        let idx_wildcard_source = query.capture_index_for_name("wildcard_source");
        let idx_export_stmt = query.capture_index_for_name("export_stmt");
        let idx_default_export = query.capture_index_for_name("default_export");
        let idx_cjs_default_export = query.capture_index_for_name("cjs_default_export");
        let idx_cjs_named_export = query.capture_index_for_name("cjs_named_export");

        #[derive(Debug)]
        struct RawExport {
            export_name: Option<CompactString>,
            local_name: Option<CompactString>,
            reexport_source: Option<CompactString>,
            wildcard_source: Option<CompactString>,
            declaration_range: Option<LspRange>,
            is_default: bool,
        }

        let raw_exports: Vec<RawExport> = self
            .execute_query(query, tree, source, |m, src| {
                let mut export_name: Option<CompactString> = None;
                let mut local_name: Option<CompactString> = None;
                let mut reexport_source: Option<CompactString> = None;
                let mut wildcard_source: Option<CompactString> = None;
                let mut declaration_range: Option<LspRange> = None;
                let mut is_default = false;

                for capture in m.captures {
                    let idx = Some(capture.index);

                    if idx == idx_export_name {
                        export_name = language.extract_identifier(capture.node, src);
                    } else if idx == idx_export_value {
                        if export_name.is_none() && capture.node.kind() == "identifier" {
                            export_name = language.extract_identifier(capture.node, src);
                        }
                    } else if idx == idx_local_name {
                        local_name = language.extract_identifier(capture.node, src);
                    } else if idx == idx_reexport_source {
                        reexport_source = capture
                            .node
                            .utf8_text(src)
                            .ok()
                            .map(|s| CompactString::from(language.strip_quotes(s)));
                    } else if idx == idx_wildcard_source {
                        wildcard_source = capture
                            .node
                            .utf8_text(src)
                            .ok()
                            .map(|s| CompactString::from(language.strip_quotes(s)));
                    } else if idx == idx_export_stmt {
                        declaration_range = Some(ts_to_lsp_range(capture.node.range()));
                    } else if idx == idx_default_export {
                        is_default = true;
                        declaration_range = Some(ts_to_lsp_range(capture.node.range()));
                    } else if idx == idx_cjs_default_export {
                        is_default = true;
                        declaration_range = Some(ts_to_lsp_range(capture.node.range()));
                    } else if idx == idx_cjs_named_export {
                        declaration_range = Some(ts_to_lsp_range(capture.node.range()));
                    }
                }

                if export_name.is_some()
                    || wildcard_source.is_some()
                    || (is_default && declaration_range.is_some())
                {
                    Some(RawExport {
                        export_name,
                        local_name,
                        reexport_source,
                        wildcard_source,
                        declaration_range,
                        is_default,
                    })
                } else {
                    None
                }
            })
            .await;

        let mut entry = FileExportEntry::new();

        for raw in raw_exports {
            if let Some(wildcard) = raw.wildcard_source {
                entry.wildcard_reexports.push(wildcard);
                continue;
            }

            let exported_name = match raw.export_name {
                Some(name) => name,
                None => {
                    if raw.is_default && raw.declaration_range.is_some() {
                        let export = ModuleExport {
                            exported_name: CompactString::from("default"),
                            local_name: None,
                            resolution: ExportResolution::Unknown,
                            declaration_range: raw.declaration_range.unwrap(),
                            is_default: true,
                        };
                        entry.default_export = Some(export);
                    }
                    continue;
                }
            };

            let declaration_range = match raw.declaration_range {
                Some(r) => r,
                None => continue,
            };

            let resolution = if let Some(source_module) = raw.reexport_source {
                ExportResolution::ReExport {
                    source_module,
                    original_name: raw
                        .local_name
                        .clone()
                        .unwrap_or_else(|| exported_name.clone()),
                }
            } else {
                ExportResolution::Unknown
            };

            let export = ModuleExport {
                exported_name: exported_name.clone(),
                local_name: raw.local_name,
                resolution,
                declaration_range,
                is_default: raw.is_default,
            };

            if raw.is_default {
                entry.default_export = Some(export);
            } else {
                entry.named_exports.insert(exported_name, export);
            }
        }

        entry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::go::Go;
    use crate::languages::javascript::JavaScript;
    use crate::languages::python::Python;
    use crate::languages::rust::Rust;
    use crate::languages::typescript::TypeScript;
    use crate::languages::LanguageSupport;

    fn create_engine() -> QueryEngine {
        QueryEngine::new()
    }

    fn parse_js(_engine: &QueryEngine, code: &str) -> Tree {
        let js = JavaScript;
        let mut parser = Parser::new();
        parser.set_language(&js.grammar()).unwrap();
        parser.parse(code, None).unwrap()
    }

    fn parse_ts(_engine: &QueryEngine, code: &str) -> Tree {
        let ts = TypeScript;
        let mut parser = Parser::new();
        parser.set_language(&ts.grammar()).unwrap();
        parser.parse(code, None).unwrap()
    }

    fn parse_python(_engine: &QueryEngine, code: &str) -> Tree {
        let py = Python;
        let mut parser = Parser::new();
        parser.set_language(&py.grammar()).unwrap();
        parser.parse(code, None).unwrap()
    }

    fn parse_go(_engine: &QueryEngine, code: &str) -> Tree {
        let go = Go;
        let mut parser = Parser::new();
        parser.set_language(&go.grammar()).unwrap();
        parser.parse(code, None).unwrap()
    }

    fn parse_rust(_engine: &QueryEngine, code: &str) -> Tree {
        let rs = Rust;
        let mut parser = Parser::new();
        parser.set_language(&rs.grammar()).unwrap();
        parser.parse(code, None).unwrap()
    }

    #[test]
    fn test_parser_pool_new() {
        let pool = ParserPool::new();
        assert!(pool.parsers.is_empty());
    }

    #[test]
    fn test_parser_pool_acquire_and_release() {
        let mut pool = ParserPool::new();
        let js = JavaScript;

        let parser = pool.acquire(&js);
        assert!(
            pool.parsers.get(js.id()).is_none() || pool.parsers.get(js.id()).unwrap().is_empty()
        );

        pool.release(js.id(), parser);
        assert_eq!(pool.parsers.get(js.id()).unwrap().len(), 1);
    }

    #[test]
    fn test_parser_pool_max_capacity() {
        let mut pool = ParserPool::new();
        let js = JavaScript;

        for _ in 0..=MAX_PARSERS_PER_LANGUAGE {
            let parser = pool.acquire(&js);
            pool.release(js.id(), parser);
        }

        assert!(pool.parsers.get(js.id()).unwrap().len() <= MAX_PARSERS_PER_LANGUAGE);
    }

    #[tokio::test]
    async fn test_parse_javascript() {
        let engine = create_engine();
        let js = JavaScript;
        let code = "const x = 1;";

        let tree = engine.parse(&js, code, None).await;
        assert!(tree.is_some());
    }

    #[tokio::test]
    async fn test_parse_typescript() {
        let engine = create_engine();
        let ts = TypeScript;
        let code = "const x: number = 1;";

        let tree = engine.parse(&ts, code, None).await;
        assert!(tree.is_some());
    }

    #[tokio::test]
    async fn test_parse_python() {
        let engine = create_engine();
        let py = Python;
        let code = "x = 1";

        let tree = engine.parse(&py, code, None).await;
        assert!(tree.is_some());
    }

    #[tokio::test]
    async fn test_parse_go() {
        let engine = create_engine();
        let go = Go;
        let code = "package main\nfunc main() {}";

        let tree = engine.parse(&go, code, None).await;
        assert!(tree.is_some());
    }

    #[tokio::test]
    async fn test_parse_rust() {
        let engine = create_engine();
        let rs = Rust;
        let code = "fn main() {}";

        let tree = engine.parse(&rs, code, None).await;
        assert!(tree.is_some());
    }

    #[tokio::test]
    async fn test_extract_references_javascript() {
        let engine = create_engine();
        let js = JavaScript;
        let code = "const db = process.env.DATABASE_URL;";
        let tree = parse_js(&engine, code);
        let import_ctx = ImportContext::new();

        let refs = engine
            .extract_references(&js, &tree, code.as_bytes(), &import_ctx)
            .await;
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "DATABASE_URL");
    }

    #[tokio::test]
    async fn test_extract_references_multiple() {
        let engine = create_engine();
        let js = JavaScript;
        let code = r#"const db = process.env.DATABASE_URL;
const api = process.env.API_KEY;"#;
        let tree = parse_js(&engine, code);
        let import_ctx = ImportContext::new();

        let refs = engine
            .extract_references(&js, &tree, code.as_bytes(), &import_ctx)
            .await;
        assert_eq!(refs.len(), 2);
    }

    #[tokio::test]
    async fn test_extract_references_typescript() {
        let engine = create_engine();
        let ts = TypeScript;
        let code = "const db: string = process.env.DATABASE_URL || '';";
        let tree = parse_ts(&engine, code);
        let import_ctx = ImportContext::new();

        let refs = engine
            .extract_references(&ts, &tree, code.as_bytes(), &import_ctx)
            .await;
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "DATABASE_URL");
    }

    #[tokio::test]
    async fn test_extract_bindings_destructure() {
        let engine = create_engine();
        let js = JavaScript;
        let code = "const { DATABASE_URL } = process.env;";
        let tree = parse_js(&engine, code);

        let bindings = engine.extract_bindings(&js, &tree, code.as_bytes()).await;
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].binding_name, "DATABASE_URL");
        assert_eq!(bindings[0].env_var_name, "DATABASE_URL");
    }

    #[tokio::test]
    async fn test_extract_bindings_destructure_with_rename() {
        let engine = create_engine();
        let js = JavaScript;
        let code = "const { DATABASE_URL: dbUrl } = process.env;";
        let tree = parse_js(&engine, code);

        let bindings = engine.extract_bindings(&js, &tree, code.as_bytes()).await;
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].binding_name, "dbUrl");
        assert_eq!(bindings[0].env_var_name, "DATABASE_URL");
    }

    #[tokio::test]
    async fn test_extract_bindings_object_alias() {
        let engine = create_engine();
        let js = JavaScript;
        let code = "const env = process.env;";
        let tree = parse_js(&engine, code);

        let bindings = engine.extract_bindings(&js, &tree, code.as_bytes()).await;
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].binding_name, "env");
        assert_eq!(bindings[0].kind, crate::types::BindingKind::Object);
    }

    #[tokio::test]
    async fn test_extract_imports_javascript() {
        let engine = create_engine();
        let js = JavaScript;
        let code = "import { env } from 'process';";
        let tree = parse_js(&engine, code);

        let imports = engine.extract_imports(&js, &tree, code.as_bytes()).await;
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].module_path, "process");
    }

    #[tokio::test]
    async fn test_extract_imports_typescript() {
        let engine = create_engine();
        let ts = TypeScript;
        let code = "import * as process from 'process';";
        let tree = parse_ts(&engine, code);

        let imports = engine.extract_imports(&ts, &tree, code.as_bytes()).await;
        assert!(!imports.is_empty());
    }

    #[tokio::test]
    async fn test_extract_imports_python() {
        let engine = create_engine();
        let py = Python;
        let code = "import os";
        let tree = parse_python(&engine, code);

        let imports = engine.extract_imports(&py, &tree, code.as_bytes()).await;
        assert!(!imports.is_empty());
    }

    #[tokio::test]
    async fn test_extract_reassignments() {
        let engine = create_engine();
        let js = JavaScript;
        let code = r#"let db = process.env.DATABASE_URL;
db = "new_value";"#;
        let tree = parse_js(&engine, code);

        let reassignments = engine
            .extract_reassignments(&js, &tree, code.as_bytes())
            .await;
        assert!(reassignments.contains(&CompactString::from("db")));
    }

    #[tokio::test]
    async fn test_extract_reassignments_with_positions() {
        let engine = create_engine();
        let js = JavaScript;
        let code = r#"let db = process.env.DATABASE_URL;
db = "new_value";"#;
        let tree = parse_js(&engine, code);

        let reassignments = engine
            .extract_reassignments_with_positions(&js, &tree, code.as_bytes())
            .await;
        assert!(!reassignments.is_empty());
        let (name, range) = &reassignments[0];
        assert_eq!(name, "db");
        assert_eq!(range.start.line, 1);
    }

    #[tokio::test]
    async fn test_extract_identifiers() {
        let engine = create_engine();
        let js = JavaScript;
        let code = "const x = 1; console.log(x);";
        let tree = parse_js(&engine, code);

        let identifiers = engine
            .extract_identifiers(&js, &tree, code.as_bytes())
            .await;
        assert!(identifiers.iter().any(|(name, _)| name == "x"));
        assert!(identifiers.iter().any(|(name, _)| name == "console"));
    }

    #[tokio::test]
    async fn test_extract_assignments() {
        let engine = create_engine();
        let js = JavaScript;
        let code = r#"const env = process.env;
const config = env;"#;
        let tree = parse_js(&engine, code);

        let assignments = engine
            .extract_assignments(&js, &tree, code.as_bytes())
            .await;

        assert!(assignments
            .iter()
            .any(|(target, _, source)| target == "config" && source == "env"));
    }

    #[tokio::test]
    async fn test_extract_destructures() {
        let engine = create_engine();
        let js = JavaScript;
        let code = r#"const env = process.env;
const { API_KEY } = env;"#;
        let tree = parse_js(&engine, code);

        let destructures = engine
            .extract_destructures(&js, &tree, code.as_bytes())
            .await;

        assert!(destructures
            .iter()
            .any(|(target, _, key, _, source)| target == "API_KEY"
                && key == "API_KEY"
                && source == "env"));
    }

    #[tokio::test]
    async fn test_extract_destructures_with_rename() {
        let engine = create_engine();
        let js = JavaScript;
        let code = r#"const env = process.env;
const { API_KEY: apiKey } = env;"#;
        let tree = parse_js(&engine, code);

        let destructures = engine
            .extract_destructures(&js, &tree, code.as_bytes())
            .await;

        assert!(destructures
            .iter()
            .any(|(target, _, key, _, source)| target == "apiKey"
                && key == "API_KEY"
                && source == "env"));
    }

    #[tokio::test]
    async fn test_check_completion_context() {
        let engine = create_engine();
        let js = JavaScript;

        let code = "process.env.";
        let tree = parse_js(&engine, code);
        let pos = tower_lsp::lsp_types::Position::new(0, 12);

        let context = engine
            .check_completion_context(&js, &tree, code.as_bytes(), pos)
            .await;
        assert!(context.is_some());
        assert_eq!(context.unwrap(), "process.env");
    }

    #[tokio::test]
    async fn test_check_completion_context_no_match() {
        let engine = create_engine();
        let js = JavaScript;
        let code = "const x = foo.";
        let tree = parse_js(&engine, code);
        let pos = tower_lsp::lsp_types::Position::new(0, 14);

        let _context = engine
            .check_completion_context(&js, &tree, code.as_bytes(), pos)
            .await;
    }

    #[tokio::test]
    async fn test_extract_exports_named() {
        let engine = create_engine();
        let js = JavaScript;
        let code = "export const API_KEY = process.env.API_KEY;";
        let tree = parse_js(&engine, code);

        let exports = engine.extract_exports(&js, &tree, code.as_bytes()).await;
        assert!(exports
            .named_exports
            .contains_key(&CompactString::from("API_KEY")));
    }

    #[tokio::test]
    async fn test_extract_exports_default() {
        let engine = create_engine();
        let js = JavaScript;
        let code = "export default process.env;";
        let tree = parse_js(&engine, code);

        let exports = engine.extract_exports(&js, &tree, code.as_bytes()).await;
        assert!(exports.default_export.is_some());
    }

    #[tokio::test]
    async fn test_execute_query_generic() {
        let engine = create_engine();
        let js = JavaScript;
        let code = "const x = 1;";
        let tree = parse_js(&engine, code);

        let query = js.identifier_query().unwrap();
        let results: Vec<String> = engine
            .execute_query(query, &tree, code.as_bytes(), |m, src| {
                m.captures
                    .first()
                    .and_then(|c| c.node.utf8_text(src).ok().map(|s| s.to_string()))
            })
            .await;

        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_go_extract_references() {
        let engine = create_engine();
        let go = Go;
        let code = r#"package main
import "os"
func main() {
    x := os.Getenv("API_KEY")
}"#;
        let tree = parse_go(&engine, code);
        let import_ctx = ImportContext::new();

        let refs = engine
            .extract_references(&go, &tree, code.as_bytes(), &import_ctx)
            .await;
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "API_KEY");
    }

    #[tokio::test]
    async fn test_rust_extract_references() {
        let engine = create_engine();
        let rs = Rust;
        let code = r#"fn main() {
    let x = std::env::var("API_KEY").unwrap();
}"#;
        let tree = parse_rust(&engine, code);
        let import_ctx = ImportContext::new();

        let refs = engine
            .extract_references(&rs, &tree, code.as_bytes(), &import_ctx)
            .await;
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "API_KEY");
    }

    #[tokio::test]
    async fn test_python_extract_references() {
        let engine = create_engine();
        let py = Python;
        let code = r#"import os
x = os.environ.get("API_KEY")"#;
        let tree = parse_python(&engine, code);
        let import_ctx = ImportContext::new();

        let refs = engine
            .extract_references(&py, &tree, code.as_bytes(), &import_ctx)
            .await;
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "API_KEY");
    }

    #[tokio::test]
    async fn test_empty_source() {
        let engine = create_engine();
        let js = JavaScript;
        let code = "";
        let tree = parse_js(&engine, code);
        let import_ctx = ImportContext::new();

        let refs = engine
            .extract_references(&js, &tree, code.as_bytes(), &import_ctx)
            .await;
        assert!(refs.is_empty());
    }

    #[tokio::test]
    async fn test_no_env_vars() {
        let engine = create_engine();
        let js = JavaScript;
        let code = "const x = 1 + 2;";
        let tree = parse_js(&engine, code);
        let import_ctx = ImportContext::new();

        let refs = engine
            .extract_references(&js, &tree, code.as_bytes(), &import_ctx)
            .await;
        assert!(refs.is_empty());
    }

    #[tokio::test]
    async fn test_nested_env_access() {
        let engine = create_engine();
        let js = JavaScript;
        let code = "const config = { db: process.env.DATABASE_URL, api: process.env.API_KEY };";
        let tree = parse_js(&engine, code);
        let import_ctx = ImportContext::new();

        let refs = engine
            .extract_references(&js, &tree, code.as_bytes(), &import_ctx)
            .await;
        assert_eq!(refs.len(), 2);
    }

    #[tokio::test]
    async fn test_conditional_env_access() {
        let engine = create_engine();
        let js = JavaScript;
        let code = "const db = process.env.DATABASE_URL || 'default';";
        let tree = parse_js(&engine, code);
        let import_ctx = ImportContext::new();

        let refs = engine
            .extract_references(&js, &tree, code.as_bytes(), &import_ctx)
            .await;
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "DATABASE_URL");
    }
}
