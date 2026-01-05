use crate::languages::LanguageSupport;
use crate::types::EnvSourceKind;
use compact_str::CompactString;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

pub struct JavaScript;

static REFERENCE_QUERY: OnceLock<Query> = OnceLock::new();
static BINDING_QUERY: OnceLock<Query> = OnceLock::new();
static COMPLETION_QUERY: OnceLock<Query> = OnceLock::new();
static IMPORT_QUERY: OnceLock<Query> = OnceLock::new();
static REASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static IDENTIFIER_QUERY: OnceLock<Query> = OnceLock::new();
static ASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static DESTRUCTURE_QUERY: OnceLock<Query> = OnceLock::new();
static SCOPE_QUERY: OnceLock<Query> = OnceLock::new();

impl LanguageSupport for JavaScript {
    fn id(&self) -> &'static str {
        "javascript"
    }

    fn is_standard_env_object(&self, name: &str) -> bool {
        name == "process.env" || name == "process" || name == "import.meta"
    }

    fn default_env_object_name(&self) -> Option<&'static str> {
        Some("process.env")
    }

    fn known_env_modules(&self) -> &'static [&'static str] {
        &["process"]
    }

    fn is_scope_node(&self, node: Node) -> bool {
        match node.kind() {
            "program"
            | "function_declaration"
            | "arrow_function"
            | "function"
            | "method_definition"
            | "class_body"
            | "statement_block"
            | "for_statement"
            | "if_statement"
            | "else_clause"
            | "try_statement"
            | "catch_clause" => true,
            _ => false,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["js", "jsx", "mjs", "cjs"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["javascript", "javascriptreact"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_javascript::LANGUAGE.into()
    }

    fn reference_query(&self) -> &Query {
        REFERENCE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/references.scm"),
            )
            .expect("Failed to compile JavaScript reference query")
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(BINDING_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/bindings.scm"),
            )
            .expect("Failed to compile JavaScript binding query")
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(COMPLETION_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/completion.scm"),
            )
            .expect("Failed to compile JavaScript completion query")
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(IMPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/imports.scm"),
            )
            .expect("Failed to compile JavaScript import query")
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(REASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/reassignments.scm"),
            )
            .expect("Failed to compile JavaScript reassignment query")
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(IDENTIFIER_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/identifiers.scm"),
            )
            .expect("Failed to compile JavaScript identifier query")
        }))
    }

    // ─────────────────────────────────────────────────────────────
    // NEW: Enhanced Binding Resolution Queries
    // ─────────────────────────────────────────────────────────────

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/assignments.scm"),
            )
            .expect("Failed to compile JavaScript assignment query")
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(DESTRUCTURE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/destructures.scm"),
            )
            .expect("Failed to compile JavaScript destructure query")
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/javascript/scopes.scm"),
            )
            .expect("Failed to compile JavaScript scope query")
        }))
    }

    fn is_env_source_node(&self, node: Node, source: &[u8]) -> Option<EnvSourceKind> {
        // Check for member_expression like process.env
        if node.kind() == "member_expression" {
            let object = node.child_by_field_name("object")?;
            let property = node.child_by_field_name("property")?;

            let object_text = object.utf8_text(source).ok()?;
            let property_text = property.utf8_text(source).ok()?;

            // process.env
            if object_text == "process" && property_text == "env" {
                return Some(EnvSourceKind::Object {
                    canonical_name: "process.env".into(),
                });
            }

            // import.meta.env (for Vite, etc.)
            if object.kind() == "member_expression" {
                let inner_object = object.child_by_field_name("object")?;
                let inner_property = object.child_by_field_name("property")?;
                let inner_object_text = inner_object.utf8_text(source).ok()?;
                let inner_property_text = inner_property.utf8_text(source).ok()?;

                if inner_object_text == "import"
                    && inner_property_text == "meta"
                    && property_text == "env"
                {
                    return Some(EnvSourceKind::Object {
                        canonical_name: "import.meta.env".into(),
                    });
                }
            }
        }

        None
    }

    fn extract_destructure_key(&self, node: Node, source: &[u8]) -> Option<CompactString> {
        // For pair_pattern like { KEY: alias }, the key is a property_identifier
        if node.kind() == "pair_pattern" {
            if let Some(key_node) = node.child_by_field_name("key") {
                return key_node.utf8_text(source).ok().map(|s| s.into());
            }
        }
        // For shorthand like { KEY }, the node itself is the key
        node.utf8_text(source).ok().map(|s| s.into())
    }

    fn strip_quotes<'a>(&self, text: &'a str) -> &'a str {
        // JavaScript/TypeScript supports double quotes, single quotes, and backticks (template literals)
        text.trim_matches(|c| c == '"' || c == '\'' || c == '`')
    }
}
