use crate::languages::LanguageSupport;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

pub struct Python;

static REFERENCE_QUERY: OnceLock<Query> = OnceLock::new();
static BINDING_QUERY: OnceLock<Query> = OnceLock::new();
static IMPORT_QUERY: OnceLock<Query> = OnceLock::new();
static COMPLETION_QUERY: OnceLock<Query> = OnceLock::new();
static REASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static IDENTIFIER_QUERY: OnceLock<Query> = OnceLock::new();

impl LanguageSupport for Python {
    fn id(&self) -> &'static str {
        "python"
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        name == "os" || name == "os.environ"
    }

    fn default_env_object_name(&self) -> Option<&'static str> {
        Some("os.environ")
    }

    fn is_scope_node(&self, node: Node) -> bool {
        match node.kind() {
            "module"
            | "function_definition"
            | "class_definition"
            | "for_statement"
            | "if_statement"
            | "try_statement"
            | "with_statement"
            | "while_statement" => true,
            _ => false,
        }
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

    fn known_env_modules(&self) -> &'static [&'static str] {
        &["os"]
    }

    fn strip_quotes<'a>(&self, text: &'a str) -> &'a str {
        // Python supports double quotes and single quotes
        // Note: triple-quoted strings (''' or """") would require more complex handling
        text.trim_matches(|c| c == '"' || c == '\'')
    }
}
