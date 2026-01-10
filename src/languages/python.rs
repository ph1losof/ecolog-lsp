use crate::languages::LanguageSupport;
use crate::types::EnvSourceKind;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

pub struct Python;

static REFERENCE_QUERY: OnceLock<Query> = OnceLock::new();
static BINDING_QUERY: OnceLock<Query> = OnceLock::new();
static IMPORT_QUERY: OnceLock<Query> = OnceLock::new();
static COMPLETION_QUERY: OnceLock<Query> = OnceLock::new();
static REASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static IDENTIFIER_QUERY: OnceLock<Query> = OnceLock::new();
static EXPORT_QUERY: OnceLock<Query> = OnceLock::new();
// Enhanced binding resolution queries
static ASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static SCOPE_QUERY: OnceLock<Query> = OnceLock::new();

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

    fn export_query(&self) -> Option<&Query> {
        Some(EXPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/python/exports.scm"),
            )
            .expect("Failed to compile Python export query")
        }))
    }

    // ─────────────────────────────────────────────────────────────
    // Enhanced Binding Resolution Queries
    // ─────────────────────────────────────────────────────────────

    fn assignment_query(&self) -> Option<&Query> {
        Some(ASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/python/assignments.scm"),
            )
            .expect("Failed to compile Python assignment query")
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(SCOPE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/python/scopes.scm"),
            )
            .expect("Failed to compile Python scope query")
        }))
    }

    fn is_env_source_node(&self, node: Node, source: &[u8]) -> Option<EnvSourceKind> {
        // Check for attribute like os.environ
        if node.kind() == "attribute" {
            let object = node.child_by_field_name("object")?;
            let attribute = node.child_by_field_name("attribute")?;

            let object_text = object.utf8_text(source).ok()?;
            let attribute_text = attribute.utf8_text(source).ok()?;

            // os.environ
            if object_text == "os" && attribute_text == "environ" {
                return Some(EnvSourceKind::Object {
                    canonical_name: "os.environ".into(),
                });
            }
        }

        // Check for just `environ` (from `from os import environ`)
        if node.kind() == "identifier" {
            let text = node.utf8_text(source).ok()?;
            if text == "environ" {
                return Some(EnvSourceKind::Object {
                    canonical_name: "os.environ".into(),
                });
            }
        }

        None
    }

    fn known_env_modules(&self) -> &'static [&'static str] {
        &["os"]
    }

    fn strip_quotes<'a>(&self, text: &'a str) -> &'a str {
        // Python supports double quotes and single quotes
        // Note: triple-quoted strings (''' or """") would require more complex handling
        text.trim_matches(|c| c == '"' || c == '\'')
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

        // In Python, we might be on the attribute identifier inside an `attribute` node
        // Check if current node or parent is an `attribute` node
        let attr_node = if node.kind() == "attribute" {
            node
        } else if let Some(parent) = node.parent() {
            if parent.kind() == "attribute" {
                parent
            } else {
                return None;
            }
        } else {
            return None;
        };

        // Get the object and attribute from the attribute node
        let object_node = attr_node.child_by_field_name("object")?;
        let attribute_node = attr_node.child_by_field_name("attribute")?;

        // We want the object to be a simple identifier
        if object_node.kind() != "identifier" {
            return None;
        }

        let object_name = object_node.utf8_text(content.as_bytes()).ok()?;
        let property_name = attribute_node.utf8_text(content.as_bytes()).ok()?;

        Some((object_name.into(), property_name.into()))
    }
}
