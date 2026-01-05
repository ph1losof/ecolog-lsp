use crate::languages::LanguageSupport;
use crate::types::{AccessType, EnvReference, ImportContext};
use compact_str::CompactString;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tree_sitter::{Parser, Query, QueryCursor, QueryMatch, Tree};

/// Pool of parsers to reuse allocations
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

        // Create new parser
        let mut parser = Parser::new();
        parser
            .set_language(&language.grammar())
            .expect("Failed to set language");
        parser
    }

    pub fn release(&mut self, language_id: &'static str, mut parser: Parser) {
        parser.reset();
        self.parsers.entry(language_id).or_default().push(parser);
    }
}

/// Executes tree-sitter queries and extracts structured data
pub struct QueryEngine {
    /// Parser pool for reuse
    parser_pool: Arc<Mutex<ParserPool>>,

    /// Query cursor pool to reduce allocations
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
        let mut pool = self.parser_pool.lock().await;
        let mut parser = pool.acquire(language);

        let tree = parser.parse(content, old_tree);

        pool.release(language.id(), parser);
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
        drop(cursor_guard); // Release lock immediately

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
        cursor_guard.push(cursor);

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

        // Pre-calculate indices to avoid string comparison in loop
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
                // Validate imports if object_name is present
                if let Some(obj) = object_name {
                    let is_std = language.is_standard_env_object(&obj);

                    if !is_std {
                        // Check aliases in ImportContext
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

                // Convert tree-sitter Range to LSP Range
                let full_lsp = tower_lsp::lsp_types::Range::new(
                    tower_lsp::lsp_types::Position::new(
                        full.start_point.row as u32,
                        full.start_point.column as u32,
                    ),
                    tower_lsp::lsp_types::Position::new(
                        full.end_point.row as u32,
                        full.end_point.column as u32,
                    ),
                );
                let name_lsp = tower_lsp::lsp_types::Range::new(
                    tower_lsp::lsp_types::Position::new(
                        name_r.start_point.row as u32,
                        name_r.start_point.column as u32,
                    ),
                    tower_lsp::lsp_types::Position::new(
                        name_r.end_point.row as u32,
                        name_r.end_point.column as u32,
                    ),
                );

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

        // Convert LSP position (0-based) to tree-sitter Point
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

                        // Strict check: point must be within [start, end]
                        // BUT: if we just typed a trigger char (like '.'), the cursor might be
                        // exactly 1 char after the node end.
                        // Example: "process.env." -> node "process.env" ends at '.', cursor is after '.'
                        // Actually tree-sitter often excludes the dot from the expression if incomplete.

                        let valid_end = if point.row == end.row {
                            point.column <= end.column + 1
                        } else {
                            point <= end
                        };

                        if point >= start && valid_end {
                            is_target = true;
                        }
                    } else if idx == idx_object {
                        obj_name = capture
                            .node
                            .utf8_text(src)
                            .ok()
                            .map(|s| CompactString::from(s));
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
            // Return the first match.
            // In case of nested matches, usually the last one (innermost) is what we want?
            // But execute_query returns in order.
            // If we have `process.env.|`, `process.env` is the target.
            // If `process.env` matches `member_expression` AND `ERROR`?
            // We'll take the first one.
            object_name.into_iter().next()
        } else {
            None
        }
    }

    /// Extract environment variable bindings from a document
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
                    // Capture the range of the property key for destructured bindings
                    bound_env_var_range = Some(capture.node.range());
                } else if idx == idx_env_binding {
                    declaration_range = Some(capture.node.range());
                } else if idx == idx_env_object_binding {
                    declaration_range = Some(capture.node.range());
                    is_object_binding = true;
                }
            }

            // FIXED: Only use default object name if no env_var_name was captured
            // For destructuring like `const { DB_URL2: dbUrl } = process.env`,
            // env_var_name should already contain "DB_URL2" from the query
            if is_object_binding && env_var_name.is_none() {
                // This handles cases like: const env = process.env
                // where we're binding the entire env object, not a specific property
                if let Some(default_obj) = language.default_env_object_name() {
                    env_var_name = Some(default_obj.into());
                }
            }

            if let (Some(bind_name), Some(env_name), Some(bind_r), Some(decl_r)) =
                (binding_name, env_var_name, binding_range, declaration_range)
            {
                let binding_lsp = tower_lsp::lsp_types::Range::new(
                    tower_lsp::lsp_types::Position::new(
                        bind_r.start_point.row as u32,
                        bind_r.start_point.column as u32,
                    ),
                    tower_lsp::lsp_types::Position::new(
                        bind_r.end_point.row as u32,
                        bind_r.end_point.column as u32,
                    ),
                );
                let decl_lsp = tower_lsp::lsp_types::Range::new(
                    tower_lsp::lsp_types::Position::new(
                        decl_r.start_point.row as u32,
                        decl_r.start_point.column as u32,
                    ),
                    tower_lsp::lsp_types::Position::new(
                        decl_r.end_point.row as u32,
                        decl_r.end_point.column as u32,
                    ),
                );

                let mut scope_range = tower_lsp::lsp_types::Range::default();
                let maybe_node = tree
                    .root_node()
                    .descendant_for_byte_range(bind_r.start_byte, bind_r.end_byte);

                if let Some(mut node) = maybe_node {
                    let mut found_scope = false;
                    while let Some(parent) = node.parent() {
                        if language.is_scope_node(parent) {
                            let range = parent.range();
                            scope_range = tower_lsp::lsp_types::Range::new(
                                tower_lsp::lsp_types::Position::new(
                                    range.start_point.row as u32,
                                    range.start_point.column as u32,
                                ),
                                tower_lsp::lsp_types::Position::new(
                                    range.end_point.row as u32,
                                    range.end_point.column as u32,
                                ),
                            );
                            found_scope = true;
                            break;
                        }
                        node = parent;
                    }
                    if !found_scope {
                        let range = tree.root_node().range();
                        scope_range = tower_lsp::lsp_types::Range::new(
                            tower_lsp::lsp_types::Position::new(
                                range.start_point.row as u32,
                                range.start_point.column as u32,
                            ),
                            tower_lsp::lsp_types::Position::new(
                                range.end_point.row as u32,
                                range.end_point.column as u32,
                            ),
                        );
                    }
                }

                // Determine the binding kind based on what was captured
                let kind = if is_object_binding {
                    // If env_var_name is a specific variable (from destructuring), it's Object type
                    // If it's the default object name, it's also Object type
                    crate::types::BindingKind::Object
                } else {
                    crate::types::BindingKind::Value
                };

                // Convert the bound_env_var range to LSP range if present
                // This is the range of the property key in destructured bindings
                // (e.g., for `{ API_KEY: apiKey }`, this is the range of `API_KEY`)
                let destructured_key_range = bound_env_var_range.map(|r| {
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(
                            r.start_point.row as u32,
                            r.start_point.column as u32,
                        ),
                        tower_lsp::lsp_types::Position::new(
                            r.end_point.row as u32,
                            r.end_point.column as u32,
                        ),
                    )
                });

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

    /// Extract import statements from a document
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
                let range_lsp = tower_lsp::lsp_types::Range::new(
                    tower_lsp::lsp_types::Position::new(
                        range.start_point.row as u32,
                        range.start_point.column as u32,
                    ),
                    tower_lsp::lsp_types::Position::new(
                        range.end_point.row as u32,
                        range.end_point.column as u32,
                    ),
                );

                Some(crate::types::ImportAlias {
                    module_path: path,
                    original_name: orig_name,
                    alias,
                    range: range_lsp,
                })
            } else {
                None
            }
        })
        .await
    }

    /// Extract reassigned variable names from a document
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

    /// Extract reassigned variable names with their positions from a document.
    /// Used for scope-aware reassignment invalidation.
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
                    let range = capture.node.range();
                    let lsp_range = tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(
                            range.start_point.row as u32,
                            range.start_point.column as u32,
                        ),
                        tower_lsp::lsp_types::Position::new(
                            range.end_point.row as u32,
                            range.end_point.column as u32,
                        ),
                    );
                    return Some((CompactString::from(name), lsp_range));
                }
            }
            None
        })
        .await
    }

    /// Extract generic identifiers from the document
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
                        let range = capture.node.range();
                        let range_lsp = tower_lsp::lsp_types::Range::new(
                            tower_lsp::lsp_types::Position::new(
                                range.start_point.row as u32,
                                range.start_point.column as u32,
                            ),
                            tower_lsp::lsp_types::Position::new(
                                range.end_point.row as u32,
                                range.end_point.column as u32,
                            ),
                        );
                        return Some((name, range_lsp));
                    }
                }
            }
            None
        })
        .await
    }

    /// Extract chain assignments (const b = a) for binding chain tracking.
    /// Returns tuples of (target_name, target_range, source_name).
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
                        let range = capture.node.range();
                        target_name = Some(name);
                        target_range = Some(tower_lsp::lsp_types::Range::new(
                            tower_lsp::lsp_types::Position::new(
                                range.start_point.row as u32,
                                range.start_point.column as u32,
                            ),
                            tower_lsp::lsp_types::Position::new(
                                range.end_point.row as u32,
                                range.end_point.column as u32,
                            ),
                        ));
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

    /// Extract destructuring patterns from identifiers.
    /// Returns tuples of (target_name, target_range, key_name, key_range, source_name).
    /// For `const { KEY: alias } = obj`, returns (alias, range, KEY, key_range, obj).
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
                        let range = capture.node.range();
                        target_name = Some(name);
                        target_range = Some(tower_lsp::lsp_types::Range::new(
                            tower_lsp::lsp_types::Position::new(
                                range.start_point.row as u32,
                                range.start_point.column as u32,
                            ),
                            tower_lsp::lsp_types::Position::new(
                                range.end_point.row as u32,
                                range.end_point.column as u32,
                            ),
                        ));
                    }
                } else if Some(capture.index) == idx_key {
                    key_name = language.extract_destructure_key(capture.node, src);
                    // Capture the key range for hover on the property key
                    let range = capture.node.range();
                    key_range = Some(tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(
                            range.start_point.row as u32,
                            range.start_point.column as u32,
                        ),
                        tower_lsp::lsp_types::Position::new(
                            range.end_point.row as u32,
                            range.end_point.column as u32,
                        ),
                    ));
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
}
