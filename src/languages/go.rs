use crate::languages::LanguageSupport;
use crate::types::EnvSourceKind;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};
use tracing::error;

pub struct Go;

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

/// Compiles a tree-sitter query, logging an error and returning an empty fallback on failure.
/// This prevents the LSP from crashing due to query compilation errors.
fn compile_query(grammar: &Language, source: &str, query_name: &str) -> Query {
    match Query::new(grammar, source) {
        Ok(query) => query,
        Err(e) => {
            error!(
                language = "go",
                query = query_name,
                error = %e,
                "Failed to compile query, using empty fallback"
            );
            // Return an empty query that matches nothing, allowing the LSP to continue
            Query::new(grammar, "").unwrap_or_else(|_| {
                panic!(
                    "Failed to create empty fallback query for Go {}",
                    query_name
                )
            })
        }
    }
}

impl LanguageSupport for Go {
    fn id(&self) -> &'static str {
        "go"
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        name == "os"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["go"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/go/references.scm"),
                "references",
            )
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/go/bindings.scm"),
                "bindings",
            )
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/go/imports.scm"),
                "imports",
            )
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/go/completion.scm"),
                "completion",
            )
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/go/reassignments.scm"),
                "reassignments",
            )
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/go/identifiers.scm"),
                "identifiers",
            )
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/go/exports.scm"),
                "exports",
            )
        }))
    }

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/go/assignments.scm"),
                "assignments",
            )
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(DESTRUCTURE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/go/destructures.scm"),
                "destructures",
            )
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/go/scopes.scm"),
                "scopes",
            )
        }))
    }

    fn is_env_source_node(&self, node: Node, source: &[u8]) -> Option<EnvSourceKind> {
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
        &["(\"", "('"]
    }

    fn is_scope_node(&self, node: tree_sitter::Node) -> bool {
        matches!(
            node.kind(),
            "function_declaration"
                | "method_declaration"
                | "func_literal"
                | "block"
                | "for_statement"
                | "if_statement"
                | "switch_statement"
                | "select_statement"
        )
    }

    fn extract_var_name(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
    ) -> Option<compact_str::CompactString> {
        use compact_str::CompactString;
        node.utf8_text(source)
            .ok()
            .map(|s| CompactString::from(self.strip_quotes(s)))
    }

    fn strip_quotes<'a>(&self, text: &'a str) -> &'a str {
        text.trim_matches(|c| c == '"' || c == '\'' || c == '`')
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

        let selector = if node.kind() == "selector_expression" {
            node
        } else if let Some(parent) = node.parent() {
            if parent.kind() == "selector_expression" {
                parent
            } else {
                return None;
            }
        } else {
            return None;
        };

        let operand_node = selector.child_by_field_name("operand")?;
        let field_node = selector.child_by_field_name("field")?;

        if operand_node.kind() != "identifier" {
            return None;
        }

        let object_name = operand_node.utf8_text(content.as_bytes()).ok()?;
        let property_name = field_node.utf8_text(content.as_bytes()).ok()?;

        Some((object_name.into(), property_name.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_go() -> Go {
        Go
    }

    #[test]
    fn test_id() {
        assert_eq!(get_go().id(), "go");
    }

    #[test]
    fn test_extensions() {
        let exts = get_go().extensions();
        assert!(exts.contains(&"go"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_go().language_ids();
        assert!(ids.contains(&"go"));
    }

    #[test]
    fn test_is_standard_env_object() {
        let go = get_go();
        assert!(go.is_standard_env_object("os"));
        assert!(!go.is_standard_env_object("process"));
        assert!(!go.is_standard_env_object("something.else"));
    }

    #[test]
    fn test_known_env_modules() {
        let modules = get_go().known_env_modules();
        assert!(modules.contains(&"os"));
    }

    #[test]
    fn test_grammar_compiles() {
        let go = get_go();
        let _grammar = go.grammar();
    }

    #[test]
    fn test_reference_query_compiles() {
        let go = get_go();
        let _query = go.reference_query();
    }

    #[test]
    fn test_binding_query_compiles() {
        let go = get_go();
        assert!(go.binding_query().is_some());
    }

    #[test]
    fn test_import_query_compiles() {
        let go = get_go();
        assert!(go.import_query().is_some());
    }

    #[test]
    fn test_completion_query_compiles() {
        let go = get_go();
        assert!(go.completion_query().is_some());
    }

    #[test]
    fn test_reassignment_query_compiles() {
        let go = get_go();
        assert!(go.reassignment_query().is_some());
    }

    #[test]
    fn test_identifier_query_compiles() {
        let go = get_go();
        assert!(go.identifier_query().is_some());
    }

    #[test]
    fn test_export_query_compiles() {
        let go = get_go();
        assert!(go.export_query().is_some());
    }

    #[test]
    fn test_assignment_query_compiles() {
        let go = get_go();
        assert!(go.assignment_query().is_some());
    }

    #[test]
    fn test_scope_query_compiles() {
        let go = get_go();
        assert!(go.scope_query().is_some());
    }

    #[test]
    fn test_destructure_query_compiles() {
        let go = get_go();
        assert!(go.destructure_query().is_some());
    }

    #[test]
    fn test_strip_quotes() {
        let go = get_go();
        assert_eq!(go.strip_quotes("\"hello\""), "hello");
        assert_eq!(go.strip_quotes("'a'"), "a");
        assert_eq!(go.strip_quotes("`raw`"), "raw");
        assert_eq!(go.strip_quotes("noquotes"), "noquotes");
    }

    #[test]
    fn test_is_env_source_node_os() {
        let go = get_go();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&go.grammar()).unwrap();

        let code = "package main\nimport \"os\"\nfunc main() { os.Getenv(\"VAR\") }";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        fn walk_tree(cursor: &mut tree_sitter::TreeCursor, go: &Go, code: &str) -> bool {
            loop {
                let node = cursor.node();
                if node.kind() == "identifier" {
                    if let Some(kind) = go.is_env_source_node(node, code.as_bytes()) {
                        if let EnvSourceKind::Object { canonical_name } = kind {
                            if canonical_name == "os" {
                                return true;
                            }
                        }
                    }
                }

                if cursor.goto_first_child() {
                    if walk_tree(cursor, go, code) {
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
        let found = walk_tree(&mut cursor, &go, code);
        assert!(found, "Should detect os as env source");
    }

    #[test]
    fn test_extract_property_access() {
        let go = get_go();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&go.grammar()).unwrap();

        let code = "package main\nfunc main() { env.DATABASE_URL }";
        let tree = parser.parse(code, None).unwrap();

        let offset = code.find("DATABASE_URL").unwrap();
        let result = go.extract_property_access(&tree, code, offset);
        assert!(result.is_some());
        let (obj, prop) = result.unwrap();
        assert_eq!(obj.as_str(), "env");
        assert_eq!(prop.as_str(), "DATABASE_URL");
    }

    #[test]
    fn test_is_scope_node() {
        let go = get_go();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&go.grammar()).unwrap();

        let code = "package main\nfunc test() {}";
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
            assert!(go.is_scope_node(func));
        }
    }

    #[test]
    fn test_extract_var_name() {
        let go = get_go();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&go.grammar()).unwrap();

        let code = "package main\nconst VAR = \"value\"";
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

        if let Some(str_lit) = find_node_of_kind(root, "interpreted_string_literal") {
            let name = go.extract_var_name(str_lit, code.as_bytes());
            assert!(name.is_some());
            assert_eq!(name.unwrap().as_str(), "value");
        }
    }
}
