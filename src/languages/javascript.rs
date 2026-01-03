use crate::languages::LanguageSupport;
use crate::types::EnvSourceKind;
use compact_str::CompactString;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

pub struct JavaScript;

static REFERENCE_QUERY: OnceLock<Query> = OnceLock::new();
static BINDING_QUERY: OnceLock<Query> = OnceLock::new();
static COMPLETION_QUERY: OnceLock<Query> = OnceLock::new();
static IMPORT_QUERY: OnceLock<Query> = OnceLock::new();
static REASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static IDENTIFIER_QUERY: OnceLock<Query> = OnceLock::new();
static ASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static DESTRUCTURE_QUERY: OnceLock<Query> = OnceLock::new();
static SCOPE_QUERY: OnceLock<Query> = OnceLock::new();
static EXPORT_QUERY: OnceLock<Query> = OnceLock::new();

impl LanguageSupport for JavaScript {
    fn id(&self) -> &'static str {
        "javascript"
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
        // Trigger on:
        // - `.` for process.env. and import.meta.env.
        // - `"` and `'` for process.env[" and process.env['
        // Server-side context validation ensures completions only appear in valid patterns
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
            | "catch_clause" => true,
            _ => false,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["js", "jsx", "mjs", "cjs"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["javascript", "javascriptreact"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_javascript::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/references.scm"),
            )
            .expect("Failed to compile JavaScript reference query")
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/bindings.scm"),
            )
            .expect("Failed to compile JavaScript binding query")
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/completion.scm"),
            )
            .expect("Failed to compile JavaScript completion query")
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/imports.scm"),
            )
            .expect("Failed to compile JavaScript import query")
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/reassignments.scm"),
            )
            .expect("Failed to compile JavaScript reassignment query")
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/identifiers.scm"),
            )
            .expect("Failed to compile JavaScript identifier query")
        }))
    }

    // ─────────────────────────────────────────────────────────────
    // NEW: Enhanced Binding Resolution Queries
    // ─────────────────────────────────────────────────────────────

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/assignments.scm"),
            )
            .expect("Failed to compile JavaScript assignment query")
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(DESTRUCTURE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/destructures.scm"),
            )
            .expect("Failed to compile JavaScript destructure query")
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/scopes.scm"),
            )
            .expect("Failed to compile JavaScript scope query")
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/exports.scm"),
            )
            .expect("Failed to compile JavaScript export query")
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
        // JavaScript/TypeScript supports double quotes, single quotes, and backticks (template literals)
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

    fn get_js() -> JavaScript {
        JavaScript
    }

    #[test]
    fn test_id() {
        assert_eq!(get_js().id(), "javascript");
    }

    #[test]
    fn test_extensions() {
        let exts = get_js().extensions();
        assert!(exts.contains(&"js"));
        assert!(exts.contains(&"jsx"));
        assert!(exts.contains(&"mjs"));
        assert!(exts.contains(&"cjs"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_js().language_ids();
        assert!(ids.contains(&"javascript"));
        assert!(ids.contains(&"javascriptreact"));
    }

    #[test]
    fn test_is_standard_env_object() {
        let js = get_js();
        assert!(js.is_standard_env_object("process.env"));
        assert!(js.is_standard_env_object("import.meta.env"));
        assert!(!js.is_standard_env_object("process"));
        assert!(!js.is_standard_env_object("import.meta"));
        assert!(!js.is_standard_env_object("something.else"));
    }

    #[test]
    fn test_default_env_object_name() {
        assert_eq!(get_js().default_env_object_name(), Some("process.env"));
    }

    #[test]
    fn test_known_env_modules() {
        let modules = get_js().known_env_modules();
        assert!(modules.contains(&"process"));
    }

    #[test]
    fn test_grammar_compiles() {
        let js = get_js();
        let _grammar = js.grammar();
        // If we get here without panic, grammar is valid
    }

    #[test]
    fn test_reference_query_compiles() {
        let js = get_js();
        let _query = js.reference_query();
    }

    #[test]
    fn test_binding_query_compiles() {
        let js = get_js();
        assert!(js.binding_query().is_some());
    }

    #[test]
    fn test_completion_query_compiles() {
        let js = get_js();
        assert!(js.completion_query().is_some());
    }

    #[test]
    fn test_import_query_compiles() {
        let js = get_js();
        assert!(js.import_query().is_some());
    }

    #[test]
    fn test_reassignment_query_compiles() {
        let js = get_js();
        assert!(js.reassignment_query().is_some());
    }

    #[test]
    fn test_identifier_query_compiles() {
        let js = get_js();
        assert!(js.identifier_query().is_some());
    }

    #[test]
    fn test_assignment_query_compiles() {
        let js = get_js();
        assert!(js.assignment_query().is_some());
    }

    #[test]
    fn test_destructure_query_compiles() {
        let js = get_js();
        assert!(js.destructure_query().is_some());
    }

    #[test]
    fn test_scope_query_compiles() {
        let js = get_js();
        assert!(js.scope_query().is_some());
    }

    #[test]
    fn test_export_query_compiles() {
        let js = get_js();
        assert!(js.export_query().is_some());
    }

    #[test]
    fn test_strip_quotes() {
        let js = get_js();
        assert_eq!(js.strip_quotes("\"hello\""), "hello");
        assert_eq!(js.strip_quotes("'world'"), "world");
        assert_eq!(js.strip_quotes("`template`"), "template");
        assert_eq!(js.strip_quotes("noquotes"), "noquotes");
    }

    #[test]
    fn test_is_env_source_node_process_env() {
        let js = get_js();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&js.grammar()).unwrap();

        let code = "const x = process.env;";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        // Find member_expression node
        let mut cursor = root.walk();

        fn walk_tree(cursor: &mut tree_sitter::TreeCursor, js: &JavaScript, code: &str) -> bool {
            loop {
                let node = cursor.node();
                if node.kind() == "member_expression" {
                    if let Some(kind) = js.is_env_source_node(node, code.as_bytes()) {
                        if let EnvSourceKind::Object { canonical_name } = kind {
                            if canonical_name == "process.env" {
                                return true;
                            }
                        }
                    }
                }

                if cursor.goto_first_child() {
                    if walk_tree(cursor, js, code) {
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

        let found_env_source = walk_tree(&mut cursor, &js, code);
        assert!(found_env_source, "Should detect process.env as env source");
    }

    #[test]
    fn test_extract_property_access() {
        let js = get_js();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&js.grammar()).unwrap();

        let code = "const x = env.DATABASE_URL;";
        let tree = parser.parse(code, None).unwrap();

        // Offset for 'D' in DATABASE_URL
        let offset = code.find("DATABASE_URL").unwrap();

        let result = js.extract_property_access(&tree, code, offset);
        assert!(result.is_some());
        let (obj, prop) = result.unwrap();
        assert_eq!(obj.as_str(), "env");
        assert_eq!(prop.as_str(), "DATABASE_URL");
    }

    #[test]
    fn test_extract_destructure_key_shorthand() {
        let js = get_js();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&js.grammar()).unwrap();

        let code = "const { VAR } = process.env;";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        // Find shorthand_property_identifier_pattern node
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

        let key = js.extract_destructure_key(shorthand.unwrap(), code.as_bytes());
        assert!(key.is_some());
        assert_eq!(key.unwrap().as_str(), "VAR");
    }
}
