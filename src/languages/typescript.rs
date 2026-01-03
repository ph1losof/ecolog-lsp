use crate::languages::LanguageSupport;
use crate::types::EnvSourceKind;
use compact_str::CompactString;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

pub struct TypeScript;
pub struct TypeScriptReact;

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
// New statics for enhanced binding resolution
static TS_ASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static TS_DESTRUCTURE_QUERY: OnceLock<Query> = OnceLock::new();
static TS_SCOPE_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_ASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_DESTRUCTURE_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_SCOPE_QUERY: OnceLock<Query> = OnceLock::new();

impl LanguageSupport for TypeScript {
    fn id(&self) -> &'static str {
        "typescript"
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
            | "module" => true,
            _ => false,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "mts", "cts"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["typescript"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    }

    fn reference_query(&self) -> &Query {
        TS_REFERENCE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/references.scm"),
            )
            .expect("Failed to compile TypeScript reference query")
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(TS_BINDING_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/bindings.scm"),
            )
            .expect("Failed to compile TypeScript binding query")
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(TS_COMPLETION_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/completion.scm"),
            )
            .expect("Failed to compile TypeScript completion query")
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(TS_IMPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/imports.scm"),
            )
            .expect("Failed to compile TypeScript import query")
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(TS_REASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/reassignments.scm"),
            )
            .expect("Failed to compile TypeScript reassignment query")
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(TS_IDENTIFIER_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/identifiers.scm"),
            )
            .expect("Failed to compile TypeScript identifier query")
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(TS_EXPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/exports.scm"),
            )
            .expect("Failed to compile TypeScript export query")
        }))
    }

    // ─────────────────────────────────────────────────────────────
    // Enhanced Binding Resolution Queries
    // ─────────────────────────────────────────────────────────────

    fn assignment_query(&self) -> Option<&Query> {
        Some(TS_ASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/assignments.scm"),
            )
            .expect("Failed to compile TypeScript assignment query")
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(TS_DESTRUCTURE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/destructures.scm"),
            )
            .expect("Failed to compile TypeScript destructure query")
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(TS_SCOPE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/scopes.scm"),
            )
            .expect("Failed to compile TypeScript scope query")
        }))
    }

    fn is_env_source_node(&self, node: Node, source: &[u8]) -> Option<EnvSourceKind> {
        // Check for member_expression like process.env
        if node.kind() == "member_expression" {
            let object = node.child_by_field_name("object")?;
            let property = node.child_by_field_name("property")?;

            let object_text = object.utf8_text(source).ok()?;
            let property_text = property.utf8_text(source).ok()?;

            // process.env
            if object_text == "process" && property_text == "env" {
                return Some(EnvSourceKind::Object {
                    canonical_name: "process.env".into(),
                });
            }

            // import.meta.env (for Vite, etc.)
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

    fn extract_destructure_key(&self, node: Node, source: &[u8]) -> Option<CompactString> {
        // For pair_pattern like { KEY: alias }, the key is a property_identifier
        if node.kind() == "pair_pattern" {
            if let Some(key_node) = node.child_by_field_name("key") {
                return key_node.utf8_text(source).ok().map(|s| s.into());
            }
        }
        // For shorthand like { KEY }, the node itself is the key
        node.utf8_text(source).ok().map(|s| s.into())
    }

    fn strip_quotes<'a>(&self, text: &'a str) -> &'a str {
        // TypeScript supports double quotes, single quotes, and backticks (template literals)
        text.trim_matches(|c| c == '"' || c == '\'' || c == '`')
    }

    fn extract_property_access(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        byte_offset: usize,
    ) -> Option<(CompactString, CompactString)> {
        let node = tree
            .root_node()
            .descendant_for_byte_range(byte_offset, byte_offset)?;

        // Check if we're on a property_identifier
        if node.kind() != "property_identifier" {
            return None;
        }

        let parent = node.parent()?;
        if parent.kind() != "member_expression" {
            return None;
        }

        // Get the object of the member expression
        let object_node = parent.child_by_field_name("object")?;
        if object_node.kind() != "identifier" {
            return None;
        }

        let object_name = object_node.utf8_text(content.as_bytes()).ok()?;
        let property_name = node.utf8_text(content.as_bytes()).ok()?;

        Some((object_name.into(), property_name.into()))
    }
}

impl LanguageSupport for TypeScriptReact {
    fn id(&self) -> &'static str {
        "typescriptreact"
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
            | "jsx_element" => true,
            _ => false,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["tsx"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["typescriptreact"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    }

    fn reference_query(&self) -> &Query {
        TSX_REFERENCE_QUERY.get_or_init(|| {
            // Using same queries for now, assuming they are compatible or main query works for both
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/references.scm"),
            )
            .expect("Failed to compile TypeScriptReact reference query")
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(TSX_BINDING_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/bindings.scm"),
            )
            .expect("Failed to compile TypeScriptReact binding query")
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(TSX_COMPLETION_QUERY.get_or_init(|| {
            // Reusing TS query for TSX
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/completion.scm"),
            )
            .expect("Failed to compile TypeScriptReact completion query")
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(TSX_IMPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/imports.scm"),
            )
            .expect("Failed to compile TypeScriptReact import query")
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(TSX_REASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/reassignments.scm"),
            )
            .expect("Failed to compile TypeScriptReact reassignment query")
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(TSX_IDENTIFIER_QUERY.get_or_init(|| {
            // Using TS query for now
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/identifiers.scm"),
            )
            .expect("Failed to compile TypeScriptReact identifier query")
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(TSX_EXPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/exports.scm"),
            )
            .expect("Failed to compile TypeScriptReact export query")
        }))
    }

    // ─────────────────────────────────────────────────────────────
    // Enhanced Binding Resolution Queries
    // ─────────────────────────────────────────────────────────────

    fn assignment_query(&self) -> Option<&Query> {
        Some(TSX_ASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/assignments.scm"),
            )
            .expect("Failed to compile TypeScriptReact assignment query")
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(TSX_DESTRUCTURE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/destructures.scm"),
            )
            .expect("Failed to compile TypeScriptReact destructure query")
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(TSX_SCOPE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/scopes.scm"),
            )
            .expect("Failed to compile TypeScriptReact scope query")
        }))
    }

    fn is_env_source_node(&self, node: Node, source: &[u8]) -> Option<EnvSourceKind> {
        // Check for member_expression like process.env
        if node.kind() == "member_expression" {
            let object = node.child_by_field_name("object")?;
            let property = node.child_by_field_name("property")?;

            let object_text = object.utf8_text(source).ok()?;
            let property_text = property.utf8_text(source).ok()?;

            // process.env
            if object_text == "process" && property_text == "env" {
                return Some(EnvSourceKind::Object {
                    canonical_name: "process.env".into(),
                });
            }

            // import.meta.env (for Vite, etc.)
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

    fn extract_destructure_key(&self, node: Node, source: &[u8]) -> Option<CompactString> {
        // For pair_pattern like { KEY: alias }, the key is a property_identifier
        if node.kind() == "pair_pattern" {
            if let Some(key_node) = node.child_by_field_name("key") {
                return key_node.utf8_text(source).ok().map(|s| s.into());
            }
        }
        // For shorthand like { KEY }, the node itself is the key
        node.utf8_text(source).ok().map(|s| s.into())
    }

    fn strip_quotes<'a>(&self, text: &'a str) -> &'a str {
        // TypeScript supports double quotes, single quotes, and backticks (template literals)
        text.trim_matches(|c| c == '"' || c == '\'' || c == '`')
    }

    fn extract_property_access(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        byte_offset: usize,
    ) -> Option<(CompactString, CompactString)> {
        let node = tree
            .root_node()
            .descendant_for_byte_range(byte_offset, byte_offset)?;

        // Check if we're on a property_identifier
        if node.kind() != "property_identifier" {
            return None;
        }

        let parent = node.parent()?;
        if parent.kind() != "member_expression" {
            return None;
        }

        // Get the object of the member expression
        let object_node = parent.child_by_field_name("object")?;
        if object_node.kind() != "identifier" {
            return None;
        }

        let object_name = object_node.utf8_text(content.as_bytes()).ok()?;
        let property_name = node.utf8_text(content.as_bytes()).ok()?;

        Some((object_name.into(), property_name.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════════════════════════
    // TypeScript Tests
    // ═══════════════════════════════════════════════════════════════

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

        fn find_node<'a>(
            node: tree_sitter::Node<'a>,
            kind: &str,
        ) -> Option<tree_sitter::Node<'a>> {
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

    // ═══════════════════════════════════════════════════════════════
    // TypeScriptReact Tests
    // ═══════════════════════════════════════════════════════════════

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

        // jsx_element is a scope node in TSX
        if let Some(jsx) = find_node_of_kind(root, "jsx_element") {
            assert!(tsx.is_scope_node(jsx));
        }
    }
}
