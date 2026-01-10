use crate::languages::LanguageSupport;
use crate::types::EnvSourceKind;
use compact_str::CompactString;
use std::sync::OnceLock;
use tree_sitter::{Language, Node, Query};

pub struct TypeScript;
pub struct TypeScriptReact;

static TS_REFERENCE_QUERY: OnceLock<Query> = OnceLock::new();
static TS_BINDING_QUERY: OnceLock<Query> = OnceLock::new();
static TS_COMPLETION_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_REFERENCE_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_BINDING_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_COMPLETION_QUERY: OnceLock<Query> = OnceLock::new();
static TS_IMPORT_QUERY: OnceLock<Query> = OnceLock::new();
static TS_REASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_IMPORT_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_REASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static TS_IDENTIFIER_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_IDENTIFIER_QUERY: OnceLock<Query> = OnceLock::new();
static TS_EXPORT_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_EXPORT_QUERY: OnceLock<Query> = OnceLock::new();
// New statics for enhanced binding resolution
static TS_ASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static TS_DESTRUCTURE_QUERY: OnceLock<Query> = OnceLock::new();
static TS_SCOPE_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_ASSIGNMENT_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_DESTRUCTURE_QUERY: OnceLock<Query> = OnceLock::new();
static TSX_SCOPE_QUERY: OnceLock<Query> = OnceLock::new();

impl LanguageSupport for TypeScript {
    fn id(&self) -> &'static str {
        "typescript"
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
            | "catch_clause"
            | "interface_declaration"
            | "module" => true,
            _ => false,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ts", "mts", "cts"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["typescript"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    }

    fn reference_query(&self) -> &Query {
        TS_REFERENCE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/references.scm"),
            )
            .expect("Failed to compile TypeScript reference query")
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(TS_BINDING_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/bindings.scm"),
            )
            .expect("Failed to compile TypeScript binding query")
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(TS_COMPLETION_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/completion.scm"),
            )
            .expect("Failed to compile TypeScript completion query")
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(TS_IMPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/imports.scm"),
            )
            .expect("Failed to compile TypeScript import query")
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(TS_REASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/reassignments.scm"),
            )
            .expect("Failed to compile TypeScript reassignment query")
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(TS_IDENTIFIER_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/identifiers.scm"),
            )
            .expect("Failed to compile TypeScript identifier query")
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(TS_EXPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/exports.scm"),
            )
            .expect("Failed to compile TypeScript export query")
        }))
    }

    // ─────────────────────────────────────────────────────────────
    // Enhanced Binding Resolution Queries
    // ─────────────────────────────────────────────────────────────

    fn assignment_query(&self) -> Option<&Query> {
        Some(TS_ASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/assignments.scm"),
            )
            .expect("Failed to compile TypeScript assignment query")
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(TS_DESTRUCTURE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/destructures.scm"),
            )
            .expect("Failed to compile TypeScript destructure query")
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(TS_SCOPE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/scopes.scm"),
            )
            .expect("Failed to compile TypeScript scope query")
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
        // TypeScript supports double quotes, single quotes, and backticks (template literals)
        text.trim_matches(|c| c == '"' || c == '\'' || c == '`')
    }

    fn extract_property_access(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        byte_offset: usize,
    ) -> Option<(CompactString, CompactString)> {
        let node = tree
            .root_node()
            .descendant_for_byte_range(byte_offset, byte_offset)?;

        // Check if we're on a property_identifier
        if node.kind() != "property_identifier" {
            return None;
        }

        let parent = node.parent()?;
        if parent.kind() != "member_expression" {
            return None;
        }

        // Get the object of the member expression
        let object_node = parent.child_by_field_name("object")?;
        if object_node.kind() != "identifier" {
            return None;
        }

        let object_name = object_node.utf8_text(content.as_bytes()).ok()?;
        let property_name = node.utf8_text(content.as_bytes()).ok()?;

        Some((object_name.into(), property_name.into()))
    }
}

impl LanguageSupport for TypeScriptReact {
    fn id(&self) -> &'static str {
        "typescriptreact"
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
            | "catch_clause"
            | "interface_declaration"
            | "module"
            | "jsx_element" => true,
            _ => false,
        }
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["tsx"]
    }

    fn language_ids(&self) -> &'static [&'static str] {
        &["typescriptreact"]
    }

    fn grammar(&self) -> Language {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    }

    fn reference_query(&self) -> &Query {
        TSX_REFERENCE_QUERY.get_or_init(|| {
            // Using same queries for now, assuming they are compatible or main query works for both
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/references.scm"),
            )
            .expect("Failed to compile TypeScriptReact reference query")
        })
    }

    fn binding_query(&self) -> Option<&Query> {
        Some(TSX_BINDING_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/bindings.scm"),
            )
            .expect("Failed to compile TypeScriptReact binding query")
        }))
    }

    fn completion_query(&self) -> Option<&Query> {
        Some(TSX_COMPLETION_QUERY.get_or_init(|| {
            // Reusing TS query for TSX
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/completion.scm"),
            )
            .expect("Failed to compile TypeScriptReact completion query")
        }))
    }

    fn import_query(&self) -> Option<&Query> {
        Some(TSX_IMPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/imports.scm"),
            )
            .expect("Failed to compile TypeScriptReact import query")
        }))
    }

    fn reassignment_query(&self) -> Option<&Query> {
        Some(TSX_REASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/reassignments.scm"),
            )
            .expect("Failed to compile TypeScriptReact reassignment query")
        }))
    }

    fn identifier_query(&self) -> Option<&Query> {
        Some(TSX_IDENTIFIER_QUERY.get_or_init(|| {
            // Using TS query for now
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/identifiers.scm"),
            )
            .expect("Failed to compile TypeScriptReact identifier query")
        }))
    }

    fn export_query(&self) -> Option<&Query> {
        Some(TSX_EXPORT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/exports.scm"),
            )
            .expect("Failed to compile TypeScriptReact export query")
        }))
    }

    // ─────────────────────────────────────────────────────────────
    // Enhanced Binding Resolution Queries
    // ─────────────────────────────────────────────────────────────

    fn assignment_query(&self) -> Option<&Query> {
        Some(TSX_ASSIGNMENT_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/assignments.scm"),
            )
            .expect("Failed to compile TypeScriptReact assignment query")
        }))
    }

    fn destructure_query(&self) -> Option<&Query> {
        Some(TSX_DESTRUCTURE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/destructures.scm"),
            )
            .expect("Failed to compile TypeScriptReact destructure query")
        }))
    }

    fn scope_query(&self) -> Option<&Query> {
        Some(TSX_SCOPE_QUERY.get_or_init(|| {
            Query::new(
                &self.grammar(),
                include_str!("../../queries/typescript/scopes.scm"),
            )
            .expect("Failed to compile TypeScriptReact scope query")
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
        // TypeScript supports double quotes, single quotes, and backticks (template literals)
        text.trim_matches(|c| c == '"' || c == '\'' || c == '`')
    }

    fn extract_property_access(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        byte_offset: usize,
    ) -> Option<(CompactString, CompactString)> {
        let node = tree
            .root_node()
            .descendant_for_byte_range(byte_offset, byte_offset)?;

        // Check if we're on a property_identifier
        if node.kind() != "property_identifier" {
            return None;
        }

        let parent = node.parent()?;
        if parent.kind() != "member_expression" {
            return None;
        }

        // Get the object of the member expression
        let object_node = parent.child_by_field_name("object")?;
        if object_node.kind() != "identifier" {
            return None;
        }

        let object_name = object_node.utf8_text(content.as_bytes()).ok()?;
        let property_name = node.utf8_text(content.as_bytes()).ok()?;

        Some((object_name.into(), property_name.into()))
    }
}
