use crate::languages::LanguageSupport;
use tree_sitter::Language;

pub struct Kotlin;

define_language_queries!("kotlin", "kotlin");

impl LanguageSupport for Kotlin {
    fn id(&self) -> &'static str {
        "kotlin"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["kt", "kts"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["kotlin"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_kotlin_ng::LANGUAGE.into()
    }

    impl_language_queries!("kotlin");

    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        &["(\"", "('"]
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        name == "System"
    }

    fn comment_node_kinds(&self) -> &'static [&'static str] {
        &["line_comment", "multiline_comment"]
    }

    fn is_scope_node(&self, node: tree_sitter::Node) -> bool {
        matches!(
            node.kind(),
            "function_declaration"
                | "anonymous_function"
                | "lambda_literal"
                | "class_declaration"
                | "object_declaration"
                | "for_statement"
                | "while_statement"
                | "do_while_statement"
                | "when_expression"
                | "if_expression"
                | "try_expression"
                | "catch_block"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_kotlin() -> Kotlin {
        Kotlin
    }

    #[test]
    fn test_id() {
        assert_eq!(get_kotlin().id(), "kotlin");
    }

    #[test]
    fn test_extensions() {
        let exts = get_kotlin().extensions();
        assert!(exts.contains(&"kt"));
        assert!(exts.contains(&"kts"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_kotlin().language_ids();
        assert!(ids.contains(&"kotlin"));
    }

    #[test]
    fn test_grammar_compiles() {
        let kotlin = get_kotlin();
        let _grammar = kotlin.grammar();
    }

    #[test]
    fn test_reference_query_compiles() {
        let kotlin = get_kotlin();
        let _query = kotlin.reference_query();
    }

    #[test]
    fn test_binding_query_compiles() {
        let kotlin = get_kotlin();
        assert!(kotlin.binding_query().is_some());
    }

    #[test]
    fn test_import_query_compiles() {
        let kotlin = get_kotlin();
        assert!(kotlin.import_query().is_some());
    }

    #[test]
    fn test_completion_query_compiles() {
        let kotlin = get_kotlin();
        assert!(kotlin.completion_query().is_some());
    }

    #[test]
    fn test_reassignment_query_compiles() {
        let kotlin = get_kotlin();
        assert!(kotlin.reassignment_query().is_some());
    }

    #[test]
    fn test_identifier_query_compiles() {
        let kotlin = get_kotlin();
        assert!(kotlin.identifier_query().is_some());
    }

    #[test]
    fn test_export_query_compiles() {
        let kotlin = get_kotlin();
        assert!(kotlin.export_query().is_some());
    }

    #[test]
    fn test_assignment_query_compiles() {
        let kotlin = get_kotlin();
        assert!(kotlin.assignment_query().is_some());
    }

    #[test]
    fn test_scope_query_compiles() {
        let kotlin = get_kotlin();
        assert!(kotlin.scope_query().is_some());
    }

    #[test]
    fn test_destructure_query_compiles() {
        let kotlin = get_kotlin();
        assert!(kotlin.destructure_query().is_some());
    }
}
