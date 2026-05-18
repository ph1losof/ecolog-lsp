use crate::languages::LanguageSupport;
use tree_sitter::Language;

pub struct Cpp;

define_language_queries!("cpp", "cpp");

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

    impl_language_queries!("cpp");

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
