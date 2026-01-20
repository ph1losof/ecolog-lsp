use crate::languages::LanguageSupport;
use crate::types::EnvSourceKind;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

pub struct Rust;

static REFERENCE_QUERY: OnceLock<Query> = OnceLock::new();
static BINDING_QUERY: OnceLock<Query> = OnceLock::new();
static IMPORT_QUERY: OnceLock<Query> = OnceLock::new();
static COMPLETION_QUERY: OnceLock<Query> = OnceLock::new();
static REASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static IDENTIFIER_QUERY: OnceLock<Query> = OnceLock::new();
static EXPORT_QUERY: OnceLock<Query> = OnceLock::new();

static ASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static SCOPE_QUERY: OnceLock<Query> = OnceLock::new();

impl LanguageSupport for Rust {
    fn id(&self) -> &'static str {
        "rust"
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        name == "std::env" || name == "std" || name == "env"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["rust"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/references.scm"),
            )
            .expect("Failed to compile Rust reference query")
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/bindings.scm"),
            )
            .expect("Failed to compile Rust binding query")
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/imports.scm"),
            )
            .expect("Failed to compile Rust import query")
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/completion.scm"),
            )
            .expect("Failed to compile Rust completion query")
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/reassignments.scm"),
            )
            .expect("Failed to compile Rust reassignment query")
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/identifiers.scm"),
            )
            .expect("Failed to compile Rust identifier query")
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/exports.scm"),
            )
            .expect("Failed to compile Rust export query")
        }))
    }

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/assignments.scm"),
            )
            .expect("Failed to compile Rust assignment query")
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/rust/scopes.scm"),
            )
            .expect("Failed to compile Rust scope query")
        }))
    }

    fn is_env_source_node(&self, node: Node, source: &[u8]) -> Option<EnvSourceKind> {
        if node.kind() == "scoped_identifier" {
            let text = node.utf8_text(source).ok()?;
            if text == "std::env" || text == "env" {
                return Some(EnvSourceKind::Object {
                    canonical_name: "std::env".into(),
                });
            }
        }

        if node.kind() == "identifier" {
            let text = node.utf8_text(source).ok()?;

            if text == "env" {
                return Some(EnvSourceKind::Object {
                    canonical_name: "std::env".into(),
                });
            }
        }

        None
    }

    fn known_env_modules(&self) -> &'static [&'static str] {
        &["std::env", "env"]
    }

    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        &["\""]
    }

    fn is_scope_node(&self, node: tree_sitter::Node) -> bool {
        match node.kind() {
            "function_item" | "closure_expression" | "block" | "for_expression"
            | "if_expression" | "loop_expression" | "while_expression" | "match_expression"
            | "impl_item" | "trait_item" | "mod_item" => true,
            _ => false,
        }
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

        let field_expr = if node.kind() == "field_expression" {
            node
        } else if let Some(parent) = node.parent() {
            if parent.kind() == "field_expression" {
                parent
            } else {
                return None;
            }
        } else {
            return None;
        };

        let value_node = field_expr.child_by_field_name("value")?;
        let field_node = field_expr.child_by_field_name("field")?;

        if value_node.kind() != "identifier" {
            return None;
        }

        let object_name = value_node.utf8_text(content.as_bytes()).ok()?;
        let property_name = field_node.utf8_text(content.as_bytes()).ok()?;

        Some((object_name.into(), property_name.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_rust() -> Rust {
        Rust
    }

    #[test]
    fn test_id() {
        assert_eq!(get_rust().id(), "rust");
    }

    #[test]
    fn test_extensions() {
        let exts = get_rust().extensions();
        assert!(exts.contains(&"rs"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_rust().language_ids();
        assert!(ids.contains(&"rust"));
    }

    #[test]
    fn test_is_standard_env_object() {
        let rs = get_rust();
        assert!(rs.is_standard_env_object("std::env"));
        assert!(rs.is_standard_env_object("std"));
        assert!(rs.is_standard_env_object("env"));
        assert!(!rs.is_standard_env_object("process"));
    }

    #[test]
    fn test_known_env_modules() {
        let modules = get_rust().known_env_modules();
        assert!(modules.contains(&"std::env"));
        assert!(modules.contains(&"env"));
    }

    #[test]
    fn test_grammar_compiles() {
        let rs = get_rust();
        let _grammar = rs.grammar();
    }

    #[test]
    fn test_reference_query_compiles() {
        let rs = get_rust();
        let _query = rs.reference_query();
    }

    #[test]
    fn test_binding_query_compiles() {
        let rs = get_rust();
        assert!(rs.binding_query().is_some());
    }

    #[test]
    fn test_import_query_compiles() {
        let rs = get_rust();
        assert!(rs.import_query().is_some());
    }

    #[test]
    fn test_completion_query_compiles() {
        let rs = get_rust();
        assert!(rs.completion_query().is_some());
    }

    #[test]
    fn test_reassignment_query_compiles() {
        let rs = get_rust();
        assert!(rs.reassignment_query().is_some());
    }

    #[test]
    fn test_identifier_query_compiles() {
        let rs = get_rust();
        assert!(rs.identifier_query().is_some());
    }

    #[test]
    fn test_export_query_compiles() {
        let rs = get_rust();
        assert!(rs.export_query().is_some());
    }

    #[test]
    fn test_assignment_query_compiles() {
        let rs = get_rust();
        assert!(rs.assignment_query().is_some());
    }

    #[test]
    fn test_scope_query_compiles() {
        let rs = get_rust();
        assert!(rs.scope_query().is_some());
    }

    #[test]
    fn test_is_env_source_node_env() {
        let rs = get_rust();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&rs.grammar()).unwrap();

        let code = "use std::env; fn main() { env::var(\"VAR\"); }";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        fn walk_tree(cursor: &mut tree_sitter::TreeCursor, rs: &Rust, code: &str) -> bool {
            loop {
                let node = cursor.node();
                if node.kind() == "identifier" || node.kind() == "scoped_identifier" {
                    if let Some(kind) = rs.is_env_source_node(node, code.as_bytes()) {
                        if let EnvSourceKind::Object { canonical_name } = kind {
                            if canonical_name == "std::env" {
                                return true;
                            }
                        }
                    }
                }

                if cursor.goto_first_child() {
                    if walk_tree(cursor, rs, code) {
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
        let found = walk_tree(&mut cursor, &rs, code);
        assert!(found, "Should detect env as env source");
    }

    #[test]
    fn test_extract_property_access() {
        let rs = get_rust();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&rs.grammar()).unwrap();

        let code = "fn main() { config.database_url }";
        let tree = parser.parse(code, None).unwrap();

        let offset = code.find("database_url").unwrap();
        let result = rs.extract_property_access(&tree, code, offset);
        assert!(result.is_some());
        let (obj, prop) = result.unwrap();
        assert_eq!(obj.as_str(), "config");
        assert_eq!(prop.as_str(), "database_url");
    }

    #[test]
    fn test_is_scope_node() {
        let rs = get_rust();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&rs.grammar()).unwrap();

        let code = "fn test() {}";
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

        if let Some(func) = find_node_of_kind(root, "function_item") {
            assert!(rs.is_scope_node(func));
        }
    }
}
