use crate::languages::LanguageSupport;
use tree_sitter::Language;

pub struct Bash;

define_language_queries!("bash", "bash");

impl LanguageSupport for Bash {
    fn id(&self) -> &'static str {
        "bash"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["sh", "bash", "zsh", "zshrc", "bashrc", "bash_profile", "zprofile"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["shellscript", "bash", "sh", "zsh"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_bash::LANGUAGE.into()
    }

    impl_language_queries!("bash");

    fn completion_trigger_characters(&self) -> &'static [&'static str] {
        &["$", "${"]
    }

    fn is_standard_env_object(&self, _name: &str) -> bool {
        // In bash, all variable expansions ($VAR, ${VAR}) are env var access
        true
    }

    fn comment_node_kinds(&self) -> &'static [&'static str] {
        &["comment"]
    }

    fn is_scope_node(&self, node: tree_sitter::Node) -> bool {
        matches!(
            node.kind(),
            "function_definition"
                | "compound_statement"
                | "subshell"
                | "for_statement"
                | "while_statement"
                | "if_statement"
                | "case_statement"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_bash() -> Bash {
        Bash
    }

    #[test]
    fn test_id() {
        assert_eq!(get_bash().id(), "bash");
    }

    #[test]
    fn test_extensions() {
        let exts = get_bash().extensions();
        assert!(exts.contains(&"sh"));
        assert!(exts.contains(&"bash"));
        assert!(exts.contains(&"zsh"));
    }

    #[test]
    fn test_language_ids() {
        let ids = get_bash().language_ids();
        assert!(ids.contains(&"bash"));
        assert!(ids.contains(&"shellscript"));
    }

    #[test]
    fn test_grammar_compiles() {
        let bash = get_bash();
        let _grammar = bash.grammar();
    }

    #[test]
    fn test_reference_query_compiles() {
        let bash = get_bash();
        let _query = bash.reference_query();
    }

    #[test]
    fn test_binding_query_compiles() {
        let bash = get_bash();
        assert!(bash.binding_query().is_some());
    }

    #[test]
    fn test_import_query_compiles() {
        let bash = get_bash();
        assert!(bash.import_query().is_some());
    }

    #[test]
    fn test_completion_query_compiles() {
        let bash = get_bash();
        assert!(bash.completion_query().is_some());
    }

    #[test]
    fn test_reassignment_query_compiles() {
        let bash = get_bash();
        assert!(bash.reassignment_query().is_some());
    }

    #[test]
    fn test_identifier_query_compiles() {
        let bash = get_bash();
        assert!(bash.identifier_query().is_some());
    }

    #[test]
    fn test_export_query_compiles() {
        let bash = get_bash();
        assert!(bash.export_query().is_some());
    }

    #[test]
    fn test_assignment_query_compiles() {
        let bash = get_bash();
        assert!(bash.assignment_query().is_some());
    }

    #[test]
    fn test_scope_query_compiles() {
        let bash = get_bash();
        assert!(bash.scope_query().is_some());
    }

    #[test]
    fn test_destructure_query_compiles() {
        let bash = get_bash();
        assert!(bash.destructure_query().is_some());
    }
}
