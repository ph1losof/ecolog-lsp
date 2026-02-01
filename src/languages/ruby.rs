use crate::languages::LanguageSupport;
use crate::types::EnvSourceKind;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};
use tracing::error;

pub struct Ruby;

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
                language = "ruby",
                query = query_name,
                error = %e,
                "Failed to compile query, using empty fallback"
            );
            // Return an empty query that matches nothing, allowing the LSP to continue
            Query::new(grammar, "").unwrap_or_else(|_| {
                panic!(
                    "Failed to create empty fallback query for Ruby {}",
                    query_name
                )
            })
        }
    }
}

impl LanguageSupport for Ruby {
    fn id(&self) -> &'static str {
        "ruby"
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        // Ruby uses the ENV constant
        name == "ENV"
    }

    fn default_env_object_name(&self) -> Option<&'static str> {
        Some("ENV")
    }

    fn is_scope_node(&self, node: Node) -> bool {
        matches!(
            node.kind(),
            "program"
                | "method"
                | "singleton_method"
                | "class"
                | "module"
                | "block"
                | "do_block"
                | "lambda"
                | "for"
                | "if"
                | "unless"
                | "case"
                | "while"
                | "until"
                | "begin"
        )
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rb", "rake", "gemspec", "ru"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["ruby"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_ruby::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/ruby/references.scm"),
                "references",
            )
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/ruby/bindings.scm"),
                "bindings",
            )
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/ruby/imports.scm"),
                "imports",
            )
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/ruby/completion.scm"),
                "completion",
            )
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/ruby/reassignments.scm"),
                "reassignments",
            )
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/ruby/identifiers.scm"),
                "identifiers",
            )
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/ruby/exports.scm"),
                "exports",
            )
        }))
    }

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/ruby/assignments.scm"),
                "assignments",
            )
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(DESTRUCTURE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/ruby/destructures.scm"),
                "destructures",
            )
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/ruby/scopes.scm"),
                "scopes",
            )
        }))
    }

    fn is_env_source_node(&self, node: Node, source: &[u8]) -> Option<EnvSourceKind> {
        // Detect ENV constant
        if node.kind() == "constant" {
            let text = node.utf8_text(source).ok()?;
            if text == "ENV" {
                return Some(EnvSourceKind::Object {
                    canonical_name: "ENV".into(),
                });
            }
        }

        None
    }

    fn known_env_modules(&self) -> &'static [&'static str] {
        &["dotenv"]
    }

    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        // Trigger on opening quote after array subscript or method call
        &["[\"", "['", "(\"", "('"]
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

        // In Ruby, method calls are through "call" nodes
        let call_node = if node.kind() == "call" {
            node
        } else if let Some(parent) = node.parent() {
            if parent.kind() == "call" {
                parent
            } else {
                return None;
            }
        } else {
            return None;
        };

        let receiver_node = call_node.child_by_field_name("receiver")?;
        let method_node = call_node.child_by_field_name("method")?;

        let receiver_name = receiver_node.utf8_text(content.as_bytes()).ok()?;
        let method_name = method_node.utf8_text(content.as_bytes()).ok()?;

        Some((receiver_name.into(), method_name.into()))
    }

    fn comment_node_kinds(&self) -> &'static [&'static str] {
        &["comment"]
    }

    fn is_root_node(&self, node: Node) -> bool {
        node.kind() == "program"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_ruby() -> Ruby {
        Ruby
    }

    #[test]
    fn test_id() {
        assert_eq!(get_ruby().id(), "ruby");
    }

    #[test]
    fn test_extensions() {
        let exts = get_ruby().extensions();
        assert!(exts.contains(&"rb"));
        assert!(exts.contains(&"rake"));
        assert!(exts.contains(&"gemspec"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_ruby().language_ids();
        assert!(ids.contains(&"ruby"));
    }

    #[test]
    fn test_is_standard_env_object() {
        let ruby = get_ruby();
        assert!(ruby.is_standard_env_object("ENV"));
        assert!(!ruby.is_standard_env_object("process"));
        assert!(!ruby.is_standard_env_object("os"));
    }

    #[test]
    fn test_default_env_object_name() {
        assert_eq!(get_ruby().default_env_object_name(), Some("ENV"));
    }

    #[test]
    fn test_known_env_modules() {
        let modules = get_ruby().known_env_modules();
        assert!(modules.contains(&"dotenv"));
    }

    #[test]
    fn test_grammar_compiles() {
        let ruby = get_ruby();
        let _grammar = ruby.grammar();
    }

    #[test]
    fn test_strip_quotes() {
        let ruby = get_ruby();
        assert_eq!(ruby.strip_quotes("\"hello\""), "hello");
        assert_eq!(ruby.strip_quotes("'world'"), "world");
        assert_eq!(ruby.strip_quotes("noquotes"), "noquotes");
    }

    #[test]
    fn test_is_env_source_node_env() {
        let ruby = get_ruby();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&ruby.grammar()).unwrap();

        let code = "x = ENV['VAR']";
        let tree = parser.parse(code, None).unwrap();
        let root = tree.root_node();

        fn walk_tree(cursor: &mut tree_sitter::TreeCursor, ruby: &Ruby, code: &str) -> bool {
            loop {
                let node = cursor.node();
                if node.kind() == "constant" {
                    if let Some(kind) = ruby.is_env_source_node(node, code.as_bytes()) {
                        if let EnvSourceKind::Object { canonical_name } = kind {
                            if canonical_name == "ENV" {
                                return true;
                            }
                        }
                    }
                }

                if cursor.goto_first_child() {
                    if walk_tree(cursor, ruby, code) {
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
        let found = walk_tree(&mut cursor, &ruby, code);
        assert!(found, "Should detect ENV as env source");
    }

    #[test]
    fn test_is_scope_node() {
        let ruby = get_ruby();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&ruby.grammar()).unwrap();

        let code = "def test\nend";
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

        if let Some(method) = find_node_of_kind(root, "method") {
            assert!(ruby.is_scope_node(method));
        }
    }
}
