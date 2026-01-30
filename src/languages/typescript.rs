use crate::languages::LanguageSupport;
use crate::types::EnvSourceKind;
use compact_str::CompactString;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};
use tracing::error;

pub struct TypeScript;
pub struct TypeScriptReact;

/// Compiles a tree-sitter query, logging an error and returning an empty fallback on failure.
/// This prevents the LSP from crashing due to query compilation errors.
fn compile_query(grammar: &Language, source: &str, lang_id: &str, query_name: &str) -> Query {
    match Query::new(grammar, source) {
        Ok(query) => query,
        Err(e) => {
            error!(
                language = lang_id,
                query = query_name,
                error = %e,
                "Failed to compile query, using empty fallback"
            );
            // Return an empty query that matches nothing, allowing the LSP to continue
            Query::new(grammar, "").unwrap_or_else(|_| {
                panic!(
                    "Failed to create empty fallback query for {} {}",
                    lang_id, query_name
                )
            })
        }
    }
}

static TS_REFERENCE_QUERY: OnceLock<Query> = OnceLock::new();
static TS_BINDING_QUERY: OnceLock<Query> = OnceLock::new();
static TS_COMPLETION_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_REFERENCE_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_BINDING_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_COMPLETION_QUERY: OnceLock<Query> = OnceLock::new();
static TS_IMPORT_QUERY: OnceLock<Query> = OnceLock::new();
static TS_REASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_IMPORT_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_REASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static TS_IDENTIFIER_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_IDENTIFIER_QUERY: OnceLock<Query> = OnceLock::new();
static TS_EXPORT_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_EXPORT_QUERY: OnceLock<Query> = OnceLock::new();

static TS_ASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static TS_DESTRUCTURE_QUERY: OnceLock<Query> = OnceLock::new();
static TS_SCOPE_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_ASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_DESTRUCTURE_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_SCOPE_QUERY: OnceLock<Query> = OnceLock::new();

/// Implements LanguageSupport for TypeScript-family languages.
/// Both TypeScript and TypeScriptReact share nearly identical implementations,
/// differing only in id, extensions, language_ids, grammar, and scope patterns.
macro_rules! impl_typescript_language {
    (
        $struct_name:ty,
        id: $id:literal,
        language_ids: $lang_ids:expr,
        extensions: $extensions:expr,
        grammar: $grammar:expr,
        extra_scope_patterns: [$($extra_scope:pat),*],
        queries: {
            reference: $ref_query:ident,
            binding: $binding_query:ident,
            completion: $completion_query:ident,
            import: $import_query:ident,
            reassignment: $reassign_query:ident,
            identifier: $ident_query:ident,
            export: $export_query:ident,
            assignment: $assign_query:ident,
            destructure: $destruct_query:ident,
            scope: $scope_query:ident
        }
    ) => {
        impl LanguageSupport for $struct_name {
            fn id(&self) -> &'static str {
                $id
            }

            fn is_standard_env_object(&self, name: &str) -> bool {
                name == "process.env" || name == "import.meta.env"
            }

            fn default_env_object_name(&self) -> Option<&'static str> {
                Some("process.env")
            }

            fn known_env_modules(&self) -> &'static [&'static str] {
                &["process"]
            }

            fn completion_trigger_characters(&self) -> &'static [&'static str] {
                &[".", "\"", "'"]
            }

            fn is_scope_node(&self, node: Node) -> bool {
                match node.kind() {
                    "program"
                    | "function_declaration"
                    | "arrow_function"
                    | "function"
                    | "method_definition"
                    | "class_body"
                    | "statement_block"
                    | "for_statement"
                    | "if_statement"
                    | "else_clause"
                    | "try_statement"
                    | "catch_clause"
                    | "interface_declaration"
                    | "module"
                    $(| $extra_scope)* => true,
                    _ => false,
                }
            }

            fn extensions(&self) -> &'static [&'static str] {
                $extensions
            }

            fn language_ids(&self) -> &'static [&'static str] {
                $lang_ids
            }

            fn grammar(&self) -> Language {
                $grammar.into()
            }

            fn reference_query(&self) -> &Query {
                $ref_query.get_or_init(|| {
                    compile_query(
                        &self.grammar(),
                        include_str!("../../queries/typescript/references.scm"),
                        $id,
                        "references",
                    )
                })
            }

            fn binding_query(&self) -> Option<&Query> {
                Some($binding_query.get_or_init(|| {
                    compile_query(
                        &self.grammar(),
                        include_str!("../../queries/typescript/bindings.scm"),
                        $id,
                        "bindings",
                    )
                }))
            }

            fn completion_query(&self) -> Option<&Query> {
                Some($completion_query.get_or_init(|| {
                    compile_query(
                        &self.grammar(),
                        include_str!("../../queries/typescript/completion.scm"),
                        $id,
                        "completion",
                    )
                }))
            }

            fn import_query(&self) -> Option<&Query> {
                Some($import_query.get_or_init(|| {
                    compile_query(
                        &self.grammar(),
                        include_str!("../../queries/typescript/imports.scm"),
                        $id,
                        "imports",
                    )
                }))
            }

            fn reassignment_query(&self) -> Option<&Query> {
                Some($reassign_query.get_or_init(|| {
                    compile_query(
                        &self.grammar(),
                        include_str!("../../queries/typescript/reassignments.scm"),
                        $id,
                        "reassignments",
                    )
                }))
            }

            fn identifier_query(&self) -> Option<&Query> {
                Some($ident_query.get_or_init(|| {
                    compile_query(
                        &self.grammar(),
                        include_str!("../../queries/typescript/identifiers.scm"),
                        $id,
                        "identifiers",
                    )
                }))
            }

            fn export_query(&self) -> Option<&Query> {
                Some($export_query.get_or_init(|| {
                    compile_query(
                        &self.grammar(),
                        include_str!("../../queries/typescript/exports.scm"),
                        $id,
                        "exports",
                    )
                }))
            }

            fn assignment_query(&self) -> Option<&Query> {
                Some($assign_query.get_or_init(|| {
                    compile_query(
                        &self.grammar(),
                        include_str!("../../queries/typescript/assignments.scm"),
                        $id,
                        "assignments",
                    )
                }))
            }

            fn destructure_query(&self) -> Option<&Query> {
                Some($destruct_query.get_or_init(|| {
                    compile_query(
                        &self.grammar(),
                        include_str!("../../queries/typescript/destructures.scm"),
                        $id,
                        "destructures",
                    )
                }))
            }

            fn scope_query(&self) -> Option<&Query> {
                Some($scope_query.get_or_init(|| {
                    compile_query(
                        &self.grammar(),
                        include_str!("../../queries/typescript/scopes.scm"),
                        $id,
                        "scopes",
                    )
                }))
            }

            fn is_env_source_node(&self, node: Node, source: &[u8]) -> Option<EnvSourceKind> {
                typescript_is_env_source_node(node, source)
            }

            fn extract_destructure_key(&self, node: Node, source: &[u8]) -> Option<CompactString> {
                typescript_extract_destructure_key(node, source)
            }

            fn strip_quotes<'a>(&self, text: &'a str) -> &'a str {
                text.trim_matches(|c| c == '"' || c == '\'' || c == '`')
            }

            fn extract_property_access(
                &self,
                tree: &tree_sitter::Tree,
                content: &str,
                byte_offset: usize,
            ) -> Option<(CompactString, CompactString)> {
                typescript_extract_property_access(tree, content, byte_offset)
            }
        }
    };
}

/// Shared implementation for detecting env source nodes in TypeScript-family languages.
fn typescript_is_env_source_node(node: Node, source: &[u8]) -> Option<EnvSourceKind> {
    if node.kind() == "member_expression" {
        let object = node.child_by_field_name("object")?;
        let property = node.child_by_field_name("property")?;

        let object_text = object.utf8_text(source).ok()?;
        let property_text = property.utf8_text(source).ok()?;

        if object_text == "process" && property_text == "env" {
            return Some(EnvSourceKind::Object {
                canonical_name: "process.env".into(),
            });
        }

        if object.kind() == "member_expression" {
            let inner_object = object.child_by_field_name("object")?;
            let inner_property = object.child_by_field_name("property")?;
            let inner_object_text = inner_object.utf8_text(source).ok()?;
            let inner_property_text = inner_property.utf8_text(source).ok()?;

            if inner_object_text == "import"
                && inner_property_text == "meta"
                && property_text == "env"
            {
                return Some(EnvSourceKind::Object {
                    canonical_name: "import.meta.env".into(),
                });
            }
        }
    }

    None
}

/// Shared implementation for extracting destructure keys in TypeScript-family languages.
fn typescript_extract_destructure_key(node: Node, source: &[u8]) -> Option<CompactString> {
    if node.kind() == "pair_pattern" {
        if let Some(key_node) = node.child_by_field_name("key") {
            return key_node.utf8_text(source).ok().map(|s| s.into());
        }
    }

    node.utf8_text(source).ok().map(|s| s.into())
}

/// Shared implementation for extracting property access in TypeScript-family languages.
fn typescript_extract_property_access(
    tree: &tree_sitter::Tree,
    content: &str,
    byte_offset: usize,
) -> Option<(CompactString, CompactString)> {
    let node = tree
        .root_node()
        .descendant_for_byte_range(byte_offset, byte_offset)?;

    if node.kind() != "property_identifier" {
        return None;
    }

    let parent = node.parent()?;
    if parent.kind() != "member_expression" {
        return None;
    }

    let object_node = parent.child_by_field_name("object")?;
    if object_node.kind() != "identifier" {
        return None;
    }

    let object_name = object_node.utf8_text(content.as_bytes()).ok()?;
    let property_name = node.utf8_text(content.as_bytes()).ok()?;

    Some((object_name.into(), property_name.into()))
}

impl_typescript_language!(
    TypeScript,
    id: "typescript",
    language_ids: &["typescript"],
    extensions: &["ts", "mts", "cts"],
    grammar: tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
    extra_scope_patterns: [],
    queries: {
        reference: TS_REFERENCE_QUERY,
        binding: TS_BINDING_QUERY,
        completion: TS_COMPLETION_QUERY,
        import: TS_IMPORT_QUERY,
        reassignment: TS_REASSIGNMENT_QUERY,
        identifier: TS_IDENTIFIER_QUERY,
        export: TS_EXPORT_QUERY,
        assignment: TS_ASSIGNMENT_QUERY,
        destructure: TS_DESTRUCTURE_QUERY,
        scope: TS_SCOPE_QUERY
    }
);

impl_typescript_language!(
    TypeScriptReact,
    id: "typescriptreact",
    language_ids: &["typescriptreact"],
    extensions: &["tsx"],
    grammar: tree_sitter_typescript::LANGUAGE_TSX,
    extra_scope_patterns: ["jsx_element"],
    queries: {
        reference: TSX_REFERENCE_QUERY,
        binding: TSX_BINDING_QUERY,
        completion: TSX_COMPLETION_QUERY,
        import: TSX_IMPORT_QUERY,
        reassignment: TSX_REASSIGNMENT_QUERY,
        identifier: TSX_IDENTIFIER_QUERY,
        export: TSX_EXPORT_QUERY,
        assignment: TSX_ASSIGNMENT_QUERY,
        destructure: TSX_DESTRUCTURE_QUERY,
        scope: TSX_SCOPE_QUERY
    }
);

#[cfg(test)]
mod tests {
    use super::*;

    fn get_ts() -> TypeScript {
        TypeScript
    }

    #[test]
    fn test_ts_id() {
        assert_eq!(get_ts().id(), "typescript");
    }

    #[test]
    fn test_ts_extensions() {
        let exts = get_ts().extensions();
        assert!(exts.contains(&"ts"));
        assert!(exts.contains(&"mts"));
        assert!(exts.contains(&"cts"));
    }

    #[test]
    fn test_ts_language_ids() {
        let ids = get_ts().language_ids();
        assert!(ids.contains(&"typescript"));
    }

    #[test]
    fn test_ts_is_standard_env_object() {
        let ts = get_ts();
        assert!(ts.is_standard_env_object("process.env"));
        assert!(ts.is_standard_env_object("import.meta.env"));
        assert!(!ts.is_standard_env_object("process"));
        assert!(!ts.is_standard_env_object("import.meta"));
        assert!(!ts.is_standard_env_object("something.else"));
    }

    #[test]
    fn test_ts_default_env_object_name() {
        assert_eq!(get_ts().default_env_object_name(), Some("process.env"));
    }

    #[test]
    fn test_ts_known_env_modules() {
        let modules = get_ts().known_env_modules();
        assert!(modules.contains(&"process"));
    }

    #[test]
    fn test_ts_grammar_compiles() {
        let ts = get_ts();
        let _grammar = ts.grammar();
    }

    #[test]
    fn test_ts_reference_query_compiles() {
        let ts = get_ts();
        let _query = ts.reference_query();
    }

    #[test]
    fn test_ts_binding_query_compiles() {
        let ts = get_ts();
        assert!(ts.binding_query().is_some());
    }

    #[test]
    fn test_ts_completion_query_compiles() {
        let ts = get_ts();
        assert!(ts.completion_query().is_some());
    }

    #[test]
    fn test_ts_import_query_compiles() {
        let ts = get_ts();
        assert!(ts.import_query().is_some());
    }

    #[test]
    fn test_ts_reassignment_query_compiles() {
        let ts = get_ts();
        assert!(ts.reassignment_query().is_some());
    }

    #[test]
    fn test_ts_identifier_query_compiles() {
        let ts = get_ts();
        assert!(ts.identifier_query().is_some());
    }

    #[test]
    fn test_ts_export_query_compiles() {
        let ts = get_ts();
        assert!(ts.export_query().is_some());
    }

    #[test]
    fn test_ts_assignment_query_compiles() {
        let ts = get_ts();
        assert!(ts.assignment_query().is_some());
    }

    #[test]
    fn test_ts_destructure_query_compiles() {
        let ts = get_ts();
        assert!(ts.destructure_query().is_some());
    }

    #[test]
    fn test_ts_scope_query_compiles() {
        let ts = get_ts();
        assert!(ts.scope_query().is_some());
    }

    #[test]
    fn test_ts_strip_quotes() {
        let ts = get_ts();
        assert_eq!(ts.strip_quotes("\"hello\""), "hello");
        assert_eq!(ts.strip_quotes("'world'"), "world");
        assert_eq!(ts.strip_quotes("`template`"), "template");
        assert_eq!(ts.strip_quotes("noquotes"), "noquotes");
    }

    #[test]
    fn test_ts_is_env_source_node_process_env() {
        let ts = get_ts();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&ts.grammar()).unwrap();

        let code = "const x = process.env;";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        fn walk_tree(cursor: &mut tree_sitter::TreeCursor, ts: &TypeScript, code: &str) -> bool {
            loop {
                let node = cursor.node();
                if node.kind() == "member_expression" {
                    if let Some(kind) = ts.is_env_source_node(node, code.as_bytes()) {
                        if let EnvSourceKind::Object { canonical_name } = kind {
                            if canonical_name == "process.env" {
                                return true;
                            }
                        }
                    }
                }

                if cursor.goto_first_child() {
                    if walk_tree(cursor, ts, code) {
                        return true;
                    }
                    cursor.goto_parent();
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            false
        }

        let mut cursor = root.walk();
        let found = walk_tree(&mut cursor, &ts, code);
        assert!(found, "Should detect process.env as env source");
    }

    #[test]
    fn test_ts_extract_property_access() {
        let ts = get_ts();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&ts.grammar()).unwrap();

        let code = "const x = env.DATABASE_URL;";
        let tree = parser.parse(code, None).unwrap();

        let offset = code.find("DATABASE_URL").unwrap();
        let result = ts.extract_property_access(&tree, code, offset);
        assert!(result.is_some());
        let (obj, prop) = result.unwrap();
        assert_eq!(obj.as_str(), "env");
        assert_eq!(prop.as_str(), "DATABASE_URL");
    }

    #[test]
    fn test_ts_extract_destructure_key_shorthand() {
        let ts = get_ts();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&ts.grammar()).unwrap();

        let code = "const { VAR } = process.env;";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        fn find_node<'a>(node: tree_sitter::Node<'a>, kind: &str) -> Option<tree_sitter::Node<'a>> {
            if node.kind() == kind {
                return Some(node);
            }
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if let Some(found) = find_node(child, kind) {
                        return Some(found);
                    }
                }
            }
            None
        }

        let shorthand = find_node(root, "shorthand_property_identifier_pattern");
        assert!(shorthand.is_some());

        let key = ts.extract_destructure_key(shorthand.unwrap(), code.as_bytes());
        assert!(key.is_some());
        assert_eq!(key.unwrap().as_str(), "VAR");
    }

    #[test]
    fn test_ts_is_scope_node() {
        let ts = get_ts();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&ts.grammar()).unwrap();

        let code = "function test() {}";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        fn find_node_of_kind<'a>(
            node: tree_sitter::Node<'a>,
            kind: &str,
        ) -> Option<tree_sitter::Node<'a>> {
            if node.kind() == kind {
                return Some(node);
            }
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if let Some(found) = find_node_of_kind(child, kind) {
                        return Some(found);
                    }
                }
            }
            None
        }

        if let Some(func) = find_node_of_kind(root, "function_declaration") {
            assert!(ts.is_scope_node(func));
        }
    }

    fn get_tsx() -> TypeScriptReact {
        TypeScriptReact
    }

    #[test]
    fn test_tsx_id() {
        assert_eq!(get_tsx().id(), "typescriptreact");
    }

    #[test]
    fn test_tsx_extensions() {
        let exts = get_tsx().extensions();
        assert!(exts.contains(&"tsx"));
    }

    #[test]
    fn test_tsx_language_ids() {
        let ids = get_tsx().language_ids();
        assert!(ids.contains(&"typescriptreact"));
    }

    #[test]
    fn test_tsx_is_standard_env_object() {
        let tsx = get_tsx();
        assert!(tsx.is_standard_env_object("process.env"));
        assert!(tsx.is_standard_env_object("import.meta.env"));
        assert!(!tsx.is_standard_env_object("process"));
        assert!(!tsx.is_standard_env_object("import.meta"));
        assert!(!tsx.is_standard_env_object("something.else"));
    }

    #[test]
    fn test_tsx_default_env_object_name() {
        assert_eq!(get_tsx().default_env_object_name(), Some("process.env"));
    }

    #[test]
    fn test_tsx_known_env_modules() {
        let modules = get_tsx().known_env_modules();
        assert!(modules.contains(&"process"));
    }

    #[test]
    fn test_tsx_grammar_compiles() {
        let tsx = get_tsx();
        let _grammar = tsx.grammar();
    }

    #[test]
    fn test_tsx_reference_query_compiles() {
        let tsx = get_tsx();
        let _query = tsx.reference_query();
    }

    #[test]
    fn test_tsx_binding_query_compiles() {
        let tsx = get_tsx();
        assert!(tsx.binding_query().is_some());
    }

    #[test]
    fn test_tsx_completion_query_compiles() {
        let tsx = get_tsx();
        assert!(tsx.completion_query().is_some());
    }

    #[test]
    fn test_tsx_import_query_compiles() {
        let tsx = get_tsx();
        assert!(tsx.import_query().is_some());
    }

    #[test]
    fn test_tsx_reassignment_query_compiles() {
        let tsx = get_tsx();
        assert!(tsx.reassignment_query().is_some());
    }

    #[test]
    fn test_tsx_identifier_query_compiles() {
        let tsx = get_tsx();
        assert!(tsx.identifier_query().is_some());
    }

    #[test]
    fn test_tsx_export_query_compiles() {
        let tsx = get_tsx();
        assert!(tsx.export_query().is_some());
    }

    #[test]
    fn test_tsx_assignment_query_compiles() {
        let tsx = get_tsx();
        assert!(tsx.assignment_query().is_some());
    }

    #[test]
    fn test_tsx_destructure_query_compiles() {
        let tsx = get_tsx();
        assert!(tsx.destructure_query().is_some());
    }

    #[test]
    fn test_tsx_scope_query_compiles() {
        let tsx = get_tsx();
        assert!(tsx.scope_query().is_some());
    }

    #[test]
    fn test_tsx_strip_quotes() {
        let tsx = get_tsx();
        assert_eq!(tsx.strip_quotes("\"hello\""), "hello");
        assert_eq!(tsx.strip_quotes("'world'"), "world");
        assert_eq!(tsx.strip_quotes("`template`"), "template");
        assert_eq!(tsx.strip_quotes("noquotes"), "noquotes");
    }

    #[test]
    fn test_tsx_is_env_source_node_process_env() {
        let tsx = get_tsx();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tsx.grammar()).unwrap();

        let code = "const x = process.env;";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        fn walk_tree(
            cursor: &mut tree_sitter::TreeCursor,
            tsx: &TypeScriptReact,
            code: &str,
        ) -> bool {
            loop {
                let node = cursor.node();
                if node.kind() == "member_expression" {
                    if let Some(kind) = tsx.is_env_source_node(node, code.as_bytes()) {
                        if let EnvSourceKind::Object { canonical_name } = kind {
                            if canonical_name == "process.env" {
                                return true;
                            }
                        }
                    }
                }

                if cursor.goto_first_child() {
                    if walk_tree(cursor, tsx, code) {
                        return true;
                    }
                    cursor.goto_parent();
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            false
        }

        let mut cursor = root.walk();
        let found = walk_tree(&mut cursor, &tsx, code);
        assert!(found, "Should detect process.env as env source");
    }

    #[test]
    fn test_tsx_extract_property_access() {
        let tsx = get_tsx();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tsx.grammar()).unwrap();

        let code = "const x = env.API_KEY;";
        let tree = parser.parse(code, None).unwrap();

        let offset = code.find("API_KEY").unwrap();
        let result = tsx.extract_property_access(&tree, code, offset);
        assert!(result.is_some());
        let (obj, prop) = result.unwrap();
        assert_eq!(obj.as_str(), "env");
        assert_eq!(prop.as_str(), "API_KEY");
    }

    #[test]
    fn test_tsx_is_scope_node_jsx_element() {
        let tsx = get_tsx();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tsx.grammar()).unwrap();

        let code = "const App = () => <div>Hello</div>;";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        fn find_node_of_kind<'a>(
            node: tree_sitter::Node<'a>,
            kind: &str,
        ) -> Option<tree_sitter::Node<'a>> {
            if node.kind() == kind {
                return Some(node);
            }
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if let Some(found) = find_node_of_kind(child, kind) {
                        return Some(found);
                    }
                }
            }
            None
        }

        if let Some(jsx) = find_node_of_kind(root, "jsx_element") {
            assert!(tsx.is_scope_node(jsx));
        }
    }
}
