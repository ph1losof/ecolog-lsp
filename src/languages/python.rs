use crate::languages::LanguageSupport;
use crate::types::EnvSourceKind;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

pub struct Python;

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

impl LanguageSupport for Python {
    fn id(&self) -> &'static str {
        "python"
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        name == "os.environ" || name == "os"
    }

    fn default_env_object_name(&self) -> Option<&'static str> {
        Some("os.environ")
    }

    fn is_scope_node(&self, node: Node) -> bool {
        matches!(
            node.kind(),
            "module"
                | "function_definition"
                | "class_definition"
                | "for_statement"
                | "if_statement"
                | "try_statement"
                | "with_statement"
                | "while_statement"
        )
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["py"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["python"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/python/references.scm"),
            )
            .expect("Failed to compile Python reference query")
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/python/bindings.scm"),
            )
            .expect("Failed to compile Python binding query")
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/python/imports.scm"),
            )
            .expect("Failed to compile Python import query")
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/python/completion.scm"),
            )
            .expect("Failed to compile Python completion query")
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/python/reassignments.scm"),
            )
            .expect("Failed to compile Python reassignment query")
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/python/identifiers.scm"),
            )
            .expect("Failed to compile Python identifier query")
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/python/exports.scm"),
            )
            .expect("Failed to compile Python export query")
        }))
    }

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/python/assignments.scm"),
            )
            .expect("Failed to compile Python assignment query")
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(DESTRUCTURE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/python/destructures.scm"),
            )
            .expect("Failed to compile Python destructure query")
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/python/scopes.scm"),
            )
            .expect("Failed to compile Python scope query")
        }))
    }

    fn is_env_source_node(&self, node: Node, source: &[u8]) -> Option<EnvSourceKind> {
        if node.kind() == "attribute" {
            let object = node.child_by_field_name("object")?;
            let attribute = node.child_by_field_name("attribute")?;

            let object_text = object.utf8_text(source).ok()?;
            let attribute_text = attribute.utf8_text(source).ok()?;

            if object_text == "os" && attribute_text == "environ" {
                return Some(EnvSourceKind::Object {
                    canonical_name: "os.environ".into(),
                });
            }
        }

        if node.kind() == "identifier" {
            let text = node.utf8_text(source).ok()?;
            if text == "environ" {
                return Some(EnvSourceKind::Object {
                    canonical_name: "os.environ".into(),
                });
            }
        }

        None
    }

    fn known_env_modules(&self) -> &'static [&'static str] {
        &["os", "dotenv", "decouple"]
    }

    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        &[".", "[\"", "['", "(\"", "('"]
    }

    fn strip_quotes<'a>(&self, text: &'a str) -> &'a str {
        text.trim_matches(|c| c == '"' || c == '\'')
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

        let attr_node = if node.kind() == "attribute" {
            node
        } else if let Some(parent) = node.parent() {
            if parent.kind() == "attribute" {
                parent
            } else {
                return None;
            }
        } else {
            return None;
        };

        let object_node = attr_node.child_by_field_name("object")?;
        let attribute_node = attr_node.child_by_field_name("attribute")?;

        if object_node.kind() != "identifier" {
            return None;
        }

        let object_name = object_node.utf8_text(content.as_bytes()).ok()?;
        let property_name = attribute_node.utf8_text(content.as_bytes()).ok()?;

        Some((object_name.into(), property_name.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_python() -> Python {
        Python
    }

    #[test]
    fn test_id() {
        assert_eq!(get_python().id(), "python");
    }

    #[test]
    fn test_extensions() {
        let exts = get_python().extensions();
        assert!(exts.contains(&"py"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_python().language_ids();
        assert!(ids.contains(&"python"));
    }

    #[test]
    fn test_is_standard_env_object() {
        let py = get_python();
        assert!(py.is_standard_env_object("os.environ"));
        assert!(py.is_standard_env_object("os")); // "os" is valid for function-call patterns like os.getenv()
        assert!(!py.is_standard_env_object("process"));
    }

    #[test]
    fn test_default_env_object_name() {
        assert_eq!(get_python().default_env_object_name(), Some("os.environ"));
    }

    #[test]
    fn test_known_env_modules() {
        let modules = get_python().known_env_modules();
        assert!(modules.contains(&"os"));
    }

    #[test]
    fn test_grammar_compiles() {
        let py = get_python();
        let _grammar = py.grammar();
    }

    #[test]
    fn test_reference_query_compiles() {
        let py = get_python();
        let _query = py.reference_query();
    }

    #[test]
    fn test_binding_query_compiles() {
        let py = get_python();
        assert!(py.binding_query().is_some());
    }

    #[test]
    fn test_import_query_compiles() {
        let py = get_python();
        assert!(py.import_query().is_some());
    }

    #[test]
    fn test_completion_query_compiles() {
        let py = get_python();
        assert!(py.completion_query().is_some());
    }

    #[test]
    fn test_reassignment_query_compiles() {
        let py = get_python();
        assert!(py.reassignment_query().is_some());
    }

    #[test]
    fn test_identifier_query_compiles() {
        let py = get_python();
        assert!(py.identifier_query().is_some());
    }

    #[test]
    fn test_export_query_compiles() {
        let py = get_python();
        assert!(py.export_query().is_some());
    }

    #[test]
    fn test_assignment_query_compiles() {
        let py = get_python();
        assert!(py.assignment_query().is_some());
    }

    #[test]
    fn test_scope_query_compiles() {
        let py = get_python();
        assert!(py.scope_query().is_some());
    }

    #[test]
    fn test_destructure_query_compiles() {
        let py = get_python();
        assert!(py.destructure_query().is_some());
    }

    #[test]
    fn test_strip_quotes() {
        let py = get_python();
        assert_eq!(py.strip_quotes("\"hello\""), "hello");
        assert_eq!(py.strip_quotes("'world'"), "world");
        assert_eq!(py.strip_quotes("noquotes"), "noquotes");
    }

    #[test]
    fn test_is_env_source_node_os_environ() {
        let py = get_python();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&py.grammar()).unwrap();

        let code = "import os\nx = os.environ";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        fn walk_tree(cursor: &mut tree_sitter::TreeCursor, py: &Python, code: &str) -> bool {
            loop {
                let node = cursor.node();
                if node.kind() == "attribute" {
                    if let Some(kind) = py.is_env_source_node(node, code.as_bytes()) {
                        if let EnvSourceKind::Object { canonical_name } = kind {
                            if canonical_name == "os.environ" {
                                return true;
                            }
                        }
                    }
                }

                if cursor.goto_first_child() {
                    if walk_tree(cursor, py, code) {
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
        let found = walk_tree(&mut cursor, &py, code);
        assert!(found, "Should detect os.environ as env source");
    }

    #[test]
    fn test_extract_property_access() {
        let py = get_python();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&py.grammar()).unwrap();

        let code = "x = env.DATABASE_URL";
        let tree = parser.parse(code, None).unwrap();

        let offset = code.find("DATABASE_URL").unwrap();
        let result = py.extract_property_access(&tree, code, offset);
        assert!(result.is_some());
        let (obj, prop) = result.unwrap();
        assert_eq!(obj.as_str(), "env");
        assert_eq!(prop.as_str(), "DATABASE_URL");
    }

    #[test]
    fn test_is_scope_node() {
        let py = get_python();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&py.grammar()).unwrap();

        let code = "def test():\n    pass";
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

        if let Some(func) = find_node_of_kind(root, "function_definition") {
            assert!(py.is_scope_node(func));
        }
    }
}
