use crate::languages::LanguageSupport;
use tree_sitter::Language;

pub struct Java;

define_language_queries!("java", "java");

impl LanguageSupport for Java {
    fn id(&self) -> &'static str {
        "java"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["java"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["java"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_java::LANGUAGE.into()
    }

    impl_language_queries!("java");

    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        &["(\"", "('"]
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        name == "System"
    }

    fn comment_node_kinds(&self) -> &'static [&'static str] {
        &["line_comment", "block_comment"]
    }

    fn is_scope_node(&self, node: tree_sitter::Node) -> bool {
        matches!(
            node.kind(),
            "method_declaration"
                | "constructor_declaration"
                | "block"
                | "for_statement"
                | "enhanced_for_statement"
                | "if_statement"
                | "while_statement"
                | "do_statement"
                | "switch_expression"
                | "try_statement"
                | "catch_clause"
                | "class_declaration"
                | "interface_declaration"
                | "lambda_expression"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_java() -> Java {
        Java
    }

    #[test]
    fn test_id() {
        assert_eq!(get_java().id(), "java");
    }

    #[test]
    fn test_extensions() {
        let exts = get_java().extensions();
        assert!(exts.contains(&"java"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_java().language_ids();
        assert!(ids.contains(&"java"));
    }

    #[test]
    fn test_grammar_compiles() {
        let java = get_java();
        let _grammar = java.grammar();
    }

    #[test]
    fn test_reference_query_compiles() {
        let java = get_java();
        let _query = java.reference_query();
    }

    #[test]
    fn test_binding_query_compiles() {
        let java = get_java();
        assert!(java.binding_query().is_some());
    }

    #[test]
    fn test_import_query_compiles() {
        let java = get_java();
        assert!(java.import_query().is_some());
    }

    #[test]
    fn test_completion_query_compiles() {
        let java = get_java();
        assert!(java.completion_query().is_some());
    }

    #[test]
    fn test_reassignment_query_compiles() {
        let java = get_java();
        assert!(java.reassignment_query().is_some());
    }

    #[test]
    fn test_identifier_query_compiles() {
        let java = get_java();
        assert!(java.identifier_query().is_some());
    }

    #[test]
    fn test_export_query_compiles() {
        let java = get_java();
        assert!(java.export_query().is_some());
    }

    #[test]
    fn test_assignment_query_compiles() {
        let java = get_java();
        assert!(java.assignment_query().is_some());
    }

    #[test]
    fn test_scope_query_compiles() {
        let java = get_java();
        assert!(java.scope_query().is_some());
    }

    #[test]
    fn test_destructure_query_compiles() {
        let java = get_java();
        assert!(java.destructure_query().is_some());
    }
}
