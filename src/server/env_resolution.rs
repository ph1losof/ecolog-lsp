//! Unified environment variable resolution at cursor position.
//!
//! This module provides a single entry point for resolving environment variables
//! at a given position, checking both local bindings and cross-module imports.

use crate::analysis::{CrossModuleResolution, CrossModuleResolver};
use crate::server::state::ServerState;
use compact_str::CompactString;
use tower_lsp::lsp_types::{Position, Range, Url};

/// How the env var was resolved
#[derive(Debug, Clone)]
pub enum EnvVarSource {
    /// Direct reference to env var (e.g., `process.env.DATABASE_URL`)
    DirectReference,
    /// Local binding (e.g., `const dbUrl = process.env.DATABASE_URL`)
    LocalBinding { binding_name: CompactString },
    /// Usage of a local binding
    LocalUsage { binding_name: CompactString },
    /// Imported from another module
    CrossModule { module_path: CompactString },
    /// Property access on an imported env object (e.g., `env.SECRET_KEY`)
    ImportedEnvObjectProperty { object_name: CompactString },
}

/// Result of resolving an env var at a position
#[derive(Debug, Clone)]
pub struct ResolvedEnvVarAtPosition {
    /// The environment variable name
    pub env_var_name: CompactString,
    /// The identifier name at the cursor (may differ from env_var_name for aliases)
    pub identifier_name: CompactString,
    /// Range of the identifier in the document
    pub range: Range,
    /// How the env var was resolved
    pub source: EnvVarSource,
}

/// Resolve the environment variable at a given position in a document.
///
/// This function checks in order:
/// 1. Direct env var references (e.g., `process.env.DATABASE_URL`)
/// 2. Env var bindings (e.g., `const dbUrl = process.env.DATABASE_URL`)
/// 3. Binding usages (e.g., using `dbUrl` after it was bound)
/// 4. Cross-module imports (if `include_cross_module` is true)
pub async fn resolve_env_var_at_position(
    uri: &Url,
    position: Position,
    state: &ServerState,
    include_cross_module: bool,
) -> Option<ResolvedEnvVarAtPosition> {
    // 1. Try direct reference
    if let Some(reference) = state
        .document_manager
        .get_env_reference_cloned(uri, position)
    {
        return Some(ResolvedEnvVarAtPosition {
            env_var_name: reference.name.clone(),
            identifier_name: reference.name,
            range: reference.full_range,
            source: EnvVarSource::DirectReference,
        });
    }

    // 2. Try binding
    if let Some(binding) = state.document_manager.get_env_binding_cloned(uri, position) {
        return Some(ResolvedEnvVarAtPosition {
            env_var_name: binding.env_var_name,
            identifier_name: binding.binding_name.clone(),
            range: binding.binding_range,
            source: EnvVarSource::LocalBinding {
                binding_name: binding.binding_name,
            },
        });
    }

    // 3. Try usage
    if let Some(usage) = state
        .document_manager
        .get_binding_usage_cloned(uri, position)
    {
        return Some(ResolvedEnvVarAtPosition {
            env_var_name: usage.env_var_name,
            identifier_name: usage.name.clone(),
            range: usage.range,
            source: EnvVarSource::LocalUsage {
                binding_name: usage.name,
            },
        });
    }

    // 4. Try cross-module resolution if enabled
    if include_cross_module {
        return resolve_cross_module_env_var(uri, position, state).await;
    }

    None
}

/// Resolve an env var through cross-module imports.
async fn resolve_cross_module_env_var(
    uri: &Url,
    position: Position,
    state: &ServerState,
) -> Option<ResolvedEnvVarAtPosition> {
    // Get document state for import context
    let doc = state.document_manager.get(uri)?;
    let import_ctx = doc.import_context.clone();
    let tree = doc.tree.clone();
    let content = doc.content.clone();
    drop(doc);

    // Get the identifier at position
    let (identifier_name, identifier_range) =
        get_identifier_at_position_internal(state, uri, position, &tree, &content).await?;

    // Check if this identifier is an import alias
    let (module_path, original_name) = match import_ctx.aliases.get(&identifier_name) {
        Some(alias) => alias.clone(),
        None => {
            // Not a direct import alias - check property access on imported env object
            return resolve_imported_env_object_property(
                uri,
                position,
                &import_ctx,
                &tree,
                &content,
                state,
            )
            .await;
        }
    };

    // Only resolve relative imports (workspace-internal)
    if !module_path.starts_with("./") && !module_path.starts_with("../") {
        return None;
    }

    // Create CrossModuleResolver and try to resolve the import
    let cross_resolver = CrossModuleResolver::new(
        state.workspace_index.clone(),
        state.module_resolver.clone(),
        state.languages.clone(),
    );

    let is_default = original_name == module_path;

    match cross_resolver.resolve_import(uri, &module_path, &original_name, is_default) {
        CrossModuleResolution::EnvVar {
            name: env_var_name, ..
        } => Some(ResolvedEnvVarAtPosition {
            env_var_name,
            identifier_name,
            range: identifier_range,
            source: EnvVarSource::CrossModule {
                module_path: module_path.into(),
            },
        }),
        _ => None,
    }
}

/// Resolve property access on an imported env object.
async fn resolve_imported_env_object_property(
    uri: &Url,
    position: Position,
    import_ctx: &crate::types::ImportContext,
    tree: &Option<tree_sitter::Tree>,
    content: &str,
    state: &ServerState,
) -> Option<ResolvedEnvVarAtPosition> {
    let tree = tree.as_ref()?;
    let language = state.languages.get_for_uri(uri)?;

    // Convert LSP position to byte offset
    let rope = ropey::Rope::from_str(content);
    let line_start = rope.try_line_to_char(position.line as usize).ok()?;
    let char_offset = line_start + position.character as usize;
    let byte_offset = rope.try_char_to_byte(char_offset).ok()?;

    // Use language-agnostic property access extraction
    let (object_name, property_name) = language.extract_property_access(tree, content, byte_offset)?;

    // Check if the object is an imported env object
    let (module_path, original_name) = import_ctx.aliases.get(object_name.as_str())?;

    // Only resolve relative imports
    if !module_path.starts_with("./") && !module_path.starts_with("../") {
        return None;
    }

    // Create CrossModuleResolver and check if the import resolves to an env object
    let cross_resolver = CrossModuleResolver::new(
        state.workspace_index.clone(),
        state.module_resolver.clone(),
        state.languages.clone(),
    );

    let is_default = original_name == module_path;

    match cross_resolver.resolve_import(uri, module_path, original_name, is_default) {
        CrossModuleResolution::EnvObject { .. } => {
            // Find the range for the property
            let node = tree
                .root_node()
                .descendant_for_byte_range(byte_offset, byte_offset)?;
            let range = Range::new(
                Position::new(
                    node.start_position().row as u32,
                    node.start_position().column as u32,
                ),
                Position::new(
                    node.end_position().row as u32,
                    node.end_position().column as u32,
                ),
            );

            Some(ResolvedEnvVarAtPosition {
                env_var_name: property_name.clone(),
                identifier_name: property_name,
                range,
                source: EnvVarSource::ImportedEnvObjectProperty { object_name },
            })
        }
        _ => None,
    }
}

/// Get the identifier at a position (internal helper).
async fn get_identifier_at_position_internal(
    _state: &ServerState,
    _uri: &Url,
    position: Position,
    tree: &Option<tree_sitter::Tree>,
    content: &str,
) -> Option<(CompactString, Range)> {
    let tree = tree.as_ref()?;

    // Convert LSP position to byte offset
    let rope = ropey::Rope::from_str(content);
    let line_start = rope.try_line_to_char(position.line as usize).ok()?;
    let char_offset = line_start + position.character as usize;
    let byte_offset = rope.try_char_to_byte(char_offset).ok()?;

    // Find the node at position
    let node = tree
        .root_node()
        .descendant_for_byte_range(byte_offset, byte_offset)?;

    // Check if it's an identifier (language-agnostic common cases)
    if node.kind() == "identifier"
        || node.kind() == "property_identifier"
        || node.kind() == "shorthand_property_identifier"
        || node.kind() == "field_identifier"
    {
        let name = node.utf8_text(content.as_bytes()).ok()?;
        let range = Range::new(
            Position::new(
                node.start_position().row as u32,
                node.start_position().column as u32,
            ),
            Position::new(
                node.end_position().row as u32,
                node.end_position().column as u32,
            ),
        );
        return Some((CompactString::from(name), range));
    }

    None
}
