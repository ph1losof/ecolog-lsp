use crate::languages::LanguageSupport;
use std::sync::OnceLock;
use tree_sitter::{Language, Query};
use tracing::error;

pub struct Cpp;

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

fn compile_query(grammar: &Language, source: &str, query_name: &str) -> Query {
    match Query::new(grammar, source) {
        Ok(query) => query,
        Err(e) => {
            error!(
                language = "cpp",
                query = query_name,
                error = %e,
                "Failed to compile query, using empty fallback"
            );
            Query::new(grammar, "").unwrap_or_else(|_| {
                panic!(
                    "Failed to create empty fallback query for C++ {}",
                    query_name
                )
            })
        }
    }
}

impl LanguageSupport for Cpp {
    fn id(&self) -> &'static str {
        "cpp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cpp", "cc", "cxx", "hpp", "hh", "hxx"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["cpp", "c++"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_cpp::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/cpp/references.scm"),
                "references",
            )
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/cpp/bindings.scm"),
                "bindings",
            )
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/cpp/imports.scm"),
                "imports",
            )
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/cpp/completion.scm"),
                "completion",
            )
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/cpp/reassignments.scm"),
                "reassignments",
            )
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/cpp/identifiers.scm"),
                "identifiers",
            )
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/cpp/exports.scm"),
                "exports",
            )
        }))
    }

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/cpp/assignments.scm"),
                "assignments",
            )
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(DESTRUCTURE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/cpp/destructures.scm"),
                "destructures",
            )
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/cpp/scopes.scm"),
                "scopes",
            )
        }))
    }

    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        &["(\"", "('"]
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        matches!(name, "getenv" | "secure_getenv" | "std")
    }

    fn comment_node_kinds(&self) -> &'static [&'static str] {
        &["comment"]
    }

    fn is_scope_node(&self, node: tree_sitter::Node) -> bool {
        matches!(
            node.kind(),
            "function_definition"
                | "compound_statement"
                | "for_statement"
                | "for_range_loop"
                | "if_statement"
                | "while_statement"
                | "do_statement"
                | "switch_statement"
                | "class_specifier"
                | "namespace_definition"
                | "lambda_expression"
                | "try_statement"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_cpp() -> Cpp {
        Cpp
    }

    #[test]
    fn test_id() {
        assert_eq!(get_cpp().id(), "cpp");
    }

    #[test]
    fn test_extensions() {
        let exts = get_cpp().extensions();
        assert!(exts.contains(&"cpp"));
        assert!(exts.contains(&"hpp"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_cpp().language_ids();
        assert!(ids.contains(&"cpp"));
    }

    #[test]
    fn test_grammar_compiles() {
        let cpp = get_cpp();
        let _grammar = cpp.grammar();
    }

    #[test]
    fn test_reference_query_compiles() {
        let cpp = get_cpp();
        let _query = cpp.reference_query();
    }

    #[test]
    fn test_binding_query_compiles() {
        let cpp = get_cpp();
        assert!(cpp.binding_query().is_some());
    }

    #[test]
    fn test_import_query_compiles() {
        let cpp = get_cpp();
        assert!(cpp.import_query().is_some());
    }

    #[test]
    fn test_completion_query_compiles() {
        let cpp = get_cpp();
        assert!(cpp.completion_query().is_some());
    }

    #[test]
    fn test_reassignment_query_compiles() {
        let cpp = get_cpp();
        assert!(cpp.reassignment_query().is_some());
    }

    #[test]
    fn test_identifier_query_compiles() {
        let cpp = get_cpp();
        assert!(cpp.identifier_query().is_some());
    }

    #[test]
    fn test_export_query_compiles() {
        let cpp = get_cpp();
        assert!(cpp.export_query().is_some());
    }

    #[test]
    fn test_assignment_query_compiles() {
        let cpp = get_cpp();
        assert!(cpp.assignment_query().is_some());
    }

    #[test]
    fn test_scope_query_compiles() {
        let cpp = get_cpp();
        assert!(cpp.scope_query().is_some());
    }

    #[test]
    fn test_destructure_query_compiles() {
        let cpp = get_cpp();
        assert!(cpp.destructure_query().is_some());
    }
}
