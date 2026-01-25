use crate::languages::LanguageSupport;
use crate::types::EnvSourceKind;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

pub struct Lua;

static REFERENCE_QUERY: OnceLock<Query> = OnceLock::new();
static BINDING_QUERY: OnceLock<Query> = OnceLock::new();
static IMPORT_QUERY: OnceLock<Query> = OnceLock::new();
static COMPLETION_QUERY: OnceLock<Query> = OnceLock::new();
static REASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static IDENTIFIER_QUERY: OnceLock<Query> = OnceLock::new();
static EXPORT_QUERY: OnceLock<Query> = OnceLock::new();

static ASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static DESTRUCTURE_QUERY: OnceLock<Query> = OnceLock::new();
static SCOPE_QUERY: OnceLock<Query> = OnceLock::new();

impl LanguageSupport for Lua {
    fn id(&self) -> &'static str {
        "lua"
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        name == "os" || name == "os.getenv"
    }

    fn default_env_object_name(&self) -> Option<&'static str> {
        Some("os.getenv")
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["lua"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["lua"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_lua::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/lua/references.scm"),
            )
            .expect("Failed to compile Lua reference query")
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/lua/bindings.scm"),
            )
            .expect("Failed to compile Lua binding query")
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/lua/imports.scm"),
            )
            .expect("Failed to compile Lua import query")
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/lua/completion.scm"),
            )
            .expect("Failed to compile Lua completion query")
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/lua/reassignments.scm"),
            )
            .expect("Failed to compile Lua reassignment query")
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/lua/identifiers.scm"),
            )
            .expect("Failed to compile Lua identifier query")
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/lua/exports.scm"),
            )
            .expect("Failed to compile Lua export query")
        }))
    }

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/lua/assignments.scm"),
            )
            .expect("Failed to compile Lua assignment query")
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(DESTRUCTURE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/lua/destructures.scm"),
            )
            .expect("Failed to compile Lua destructure query")
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/lua/scopes.scm"),
            )
            .expect("Failed to compile Lua scope query")
        }))
    }

    fn is_env_source_node(&self, node: Node, source: &[u8]) -> Option<EnvSourceKind> {
        // Detect os.getenv pattern
        // In Lua, this is typically a function call: os.getenv("VAR")
        // We want to detect when we're looking at the "os" identifier that's part of os.getenv
        if node.kind() == "identifier" {
            let text = node.utf8_text(source).ok()?;
            if text == "os" {
                return Some(EnvSourceKind::Object {
                    canonical_name: "os".into(),
                });
            }
        }

        None
    }

    fn known_env_modules(&self) -> &'static [&'static str] {
        &["os"]
    }

    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        // Support both parenthesized and parenthesis-less function calls:
        // os.getenv("  os.getenv('  os.getenv "  os.getenv '
        &["(\"", "('", " \"", " '"]
    }

    fn is_scope_node(&self, node: Node) -> bool {
        matches!(
            node.kind(),
            "function_declaration"
                | "function_definition"
                | "do_statement"
                | "while_statement"
                | "repeat_statement"
                | "for_statement"
                | "if_statement"
        )
    }

    fn strip_quotes<'a>(&self, text: &'a str) -> &'a str {
        text.trim_matches(|c| c == '"' || c == '\'')
    }

    fn extract_var_name(&self, node: Node, source: &[u8]) -> Option<compact_str::CompactString> {
        node.utf8_text(source)
            .ok()
            .map(|s| compact_str::CompactString::from(self.strip_quotes(s)))
    }

    fn extract_property_access(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        byte_offset: usize,
    ) -> Option<(compact_str::CompactString, compact_str::CompactString)> {
        let node = tree
            .root_node()
            .descendant_for_byte_range(byte_offset, byte_offset)?;

        // In Lua, property access is through dot_index_expression
        let dot_index = if node.kind() == "dot_index_expression" {
            node
        } else if let Some(parent) = node.parent() {
            if parent.kind() == "dot_index_expression" {
                parent
            } else {
                return None;
            }
        } else {
            return None;
        };

        let table_node = dot_index.child_by_field_name("table")?;
        let field_node = dot_index.child_by_field_name("field")?;

        if table_node.kind() != "identifier" {
            return None;
        }

        let table_name = table_node.utf8_text(content.as_bytes()).ok()?;
        let field_name = field_node.utf8_text(content.as_bytes()).ok()?;

        Some((table_name.into(), field_name.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_lua() -> Lua {
        Lua
    }

    #[test]
    fn test_id() {
        assert_eq!(get_lua().id(), "lua");
    }

    #[test]
    fn test_extensions() {
        let exts = get_lua().extensions();
        assert!(exts.contains(&"lua"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_lua().language_ids();
        assert!(ids.contains(&"lua"));
    }

    #[test]
    fn test_is_standard_env_object() {
        let lua = get_lua();
        assert!(lua.is_standard_env_object("os"));
        assert!(lua.is_standard_env_object("os.getenv"));
        assert!(!lua.is_standard_env_object("process"));
    }

    #[test]
    fn test_default_env_object_name() {
        assert_eq!(get_lua().default_env_object_name(), Some("os.getenv"));
    }

    #[test]
    fn test_known_env_modules() {
        let modules = get_lua().known_env_modules();
        assert!(modules.contains(&"os"));
    }

    #[test]
    fn test_grammar_compiles() {
        let lua = get_lua();
        let _grammar = lua.grammar();
    }

    #[test]
    fn test_reference_query_compiles() {
        let lua = get_lua();
        let _query = lua.reference_query();
    }

    #[test]
    fn test_binding_query_compiles() {
        let lua = get_lua();
        assert!(lua.binding_query().is_some());
    }

    #[test]
    fn test_import_query_compiles() {
        let lua = get_lua();
        assert!(lua.import_query().is_some());
    }

    #[test]
    fn test_completion_query_compiles() {
        let lua = get_lua();
        assert!(lua.completion_query().is_some());
    }

    #[test]
    fn test_reassignment_query_compiles() {
        let lua = get_lua();
        assert!(lua.reassignment_query().is_some());
    }

    #[test]
    fn test_identifier_query_compiles() {
        let lua = get_lua();
        assert!(lua.identifier_query().is_some());
    }

    #[test]
    fn test_export_query_compiles() {
        let lua = get_lua();
        assert!(lua.export_query().is_some());
    }

    #[test]
    fn test_assignment_query_compiles() {
        let lua = get_lua();
        assert!(lua.assignment_query().is_some());
    }

    #[test]
    fn test_scope_query_compiles() {
        let lua = get_lua();
        assert!(lua.scope_query().is_some());
    }

    #[test]
    fn test_destructure_query_compiles() {
        let lua = get_lua();
        assert!(lua.destructure_query().is_some());
    }

    #[test]
    fn test_strip_quotes() {
        let lua = get_lua();
        assert_eq!(lua.strip_quotes("\"hello\""), "hello");
        assert_eq!(lua.strip_quotes("'world'"), "world");
        assert_eq!(lua.strip_quotes("noquotes"), "noquotes");
    }

    #[test]
    fn test_is_env_source_node_os() {
        let lua = get_lua();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lua.grammar()).unwrap();

        let code = "local x = os.getenv(\"VAR\")";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        fn walk_tree(cursor: &mut tree_sitter::TreeCursor, lua: &Lua, code: &str) -> bool {
            loop {
                let node = cursor.node();
                if node.kind() == "identifier" {
                    if let Some(kind) = lua.is_env_source_node(node, code.as_bytes()) {
                        if let EnvSourceKind::Object { canonical_name } = kind {
                            if canonical_name == "os" {
                                return true;
                            }
                        }
                    }
                }

                if cursor.goto_first_child() {
                    if walk_tree(cursor, lua, code) {
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
        let found = walk_tree(&mut cursor, &lua, code);
        assert!(found, "Should detect os as env source");
    }

    #[test]
    fn test_extract_property_access() {
        let lua = get_lua();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lua.grammar()).unwrap();

        let code = "local x = env.DATABASE_URL";
        let tree = parser.parse(code, None).unwrap();

        let offset = code.find("DATABASE_URL").unwrap();
        let result = lua.extract_property_access(&tree, code, offset);
        assert!(result.is_some());
        let (table, field) = result.unwrap();
        assert_eq!(table.as_str(), "env");
        assert_eq!(field.as_str(), "DATABASE_URL");
    }

    #[test]
    fn test_is_scope_node() {
        let lua = get_lua();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lua.grammar()).unwrap();

        let code = "function test()\nend";
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
            assert!(lua.is_scope_node(func));
        }
    }

    #[test]
    fn test_extract_var_name() {
        let lua = get_lua();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lua.grammar()).unwrap();

        let code = "local VAR = \"value\"";
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

        if let Some(str_lit) = find_node_of_kind(root, "string") {
            let name = lua.extract_var_name(str_lit, code.as_bytes());
            assert!(name.is_some());
            assert_eq!(name.unwrap().as_str(), "value");
        }
    }
}
