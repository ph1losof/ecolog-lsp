use crate::languages::LanguageSupport;
use std::sync::OnceLock;
use tree_sitter::{Language, Query};
use tracing::error;

pub struct CSharp;

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
                language = "csharp",
                query = query_name,
                error = %e,
                "Failed to compile query, using empty fallback"
            );
            Query::new(grammar, "").unwrap_or_else(|_| {
                panic!(
                    "Failed to create empty fallback query for C# {}",
                    query_name
                )
            })
        }
    }
}

impl LanguageSupport for CSharp {
    fn id(&self) -> &'static str {
        "csharp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["cs", "csx"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["csharp", "c#"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_c_sharp::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/csharp/references.scm"),
                "references",
            )
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/csharp/bindings.scm"),
                "bindings",
            )
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/csharp/imports.scm"),
                "imports",
            )
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/csharp/completion.scm"),
                "completion",
            )
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/csharp/reassignments.scm"),
                "reassignments",
            )
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/csharp/identifiers.scm"),
                "identifiers",
            )
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/csharp/exports.scm"),
                "exports",
            )
        }))
    }

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/csharp/assignments.scm"),
                "assignments",
            )
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(DESTRUCTURE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/csharp/destructures.scm"),
                "destructures",
            )
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            compile_query(
                &self.grammar(),
                include_str!("../../queries/csharp/scopes.scm"),
                "scopes",
            )
        }))
    }

    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        &["(\"", "('"]
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        name == "Environment"
    }

    fn comment_node_kinds(&self) -> &'static [&'static str] {
        &["comment"]
    }

    fn is_scope_node(&self, node: tree_sitter::Node) -> bool {
        matches!(
            node.kind(),
            "method_declaration"
                | "constructor_declaration"
                | "block"
                | "for_statement"
                | "foreach_statement"
                | "if_statement"
                | "while_statement"
                | "do_statement"
                | "switch_statement"
                | "try_statement"
                | "catch_clause"
                | "class_declaration"
                | "struct_declaration"
                | "interface_declaration"
                | "namespace_declaration"
                | "lambda_expression"
                | "local_function_statement"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_csharp() -> CSharp {
        CSharp
    }

    #[test]
    fn test_id() {
        assert_eq!(get_csharp().id(), "csharp");
    }

    #[test]
    fn test_extensions() {
        let exts = get_csharp().extensions();
        assert!(exts.contains(&"cs"));
        assert!(exts.contains(&"csx"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_csharp().language_ids();
        assert!(ids.contains(&"csharp"));
    }

    #[test]
    fn test_grammar_compiles() {
        let csharp = get_csharp();
        let _grammar = csharp.grammar();
    }

    #[test]
    fn test_reference_query_compiles() {
        let csharp = get_csharp();
        let _query = csharp.reference_query();
    }

    #[test]
    fn test_binding_query_compiles() {
        let csharp = get_csharp();
        assert!(csharp.binding_query().is_some());
    }

    #[test]
    fn test_import_query_compiles() {
        let csharp = get_csharp();
        assert!(csharp.import_query().is_some());
    }

    #[test]
    fn test_completion_query_compiles() {
        let csharp = get_csharp();
        assert!(csharp.completion_query().is_some());
    }

    #[test]
    fn test_reassignment_query_compiles() {
        let csharp = get_csharp();
        assert!(csharp.reassignment_query().is_some());
    }

    #[test]
    fn test_identifier_query_compiles() {
        let csharp = get_csharp();
        assert!(csharp.identifier_query().is_some());
    }

    #[test]
    fn test_export_query_compiles() {
        let csharp = get_csharp();
        assert!(csharp.export_query().is_some());
    }

    #[test]
    fn test_assignment_query_compiles() {
        let csharp = get_csharp();
        assert!(csharp.assignment_query().is_some());
    }

    #[test]
    fn test_scope_query_compiles() {
        let csharp = get_csharp();
        assert!(csharp.scope_query().is_some());
    }

    #[test]
    fn test_destructure_query_compiles() {
        let csharp = get_csharp();
        assert!(csharp.destructure_query().is_some());
    }
}
