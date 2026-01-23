use crate::analysis::{CrossModuleResolution, CrossModuleResolver};
use crate::server::state::ServerState;
use compact_str::CompactString;
use tower_lsp::lsp_types::{Position, Range, Url};

#[derive(Debug, Clone)]
pub enum EnvVarSource {
    DirectReference,

    LocalBinding { binding_name: CompactString },

    LocalUsage { binding_name: CompactString },

    CrossModule { module_path: CompactString },

    ImportedEnvObjectProperty { object_name: CompactString },
}

#[derive(Debug, Clone)]
pub struct ResolvedEnvVarAtPosition {
    pub env_var_name: CompactString,

    pub identifier_name: CompactString,

    pub range: Range,

    pub source: EnvVarSource,
}

pub async fn resolve_env_var_at_position(
    uri: &Url,
    position: Position,
    state: &ServerState,
    include_cross_module: bool,
) -> Option<ResolvedEnvVarAtPosition> {
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

    if include_cross_module {
        return resolve_cross_module_env_var(uri, position, state).await;
    }

    None
}

async fn resolve_cross_module_env_var(
    uri: &Url,
    position: Position,
    state: &ServerState,
) -> Option<ResolvedEnvVarAtPosition> {
    let doc = state.document_manager.get(uri)?;
    let import_ctx = doc.import_context.clone();
    let tree = doc.tree.clone();
    let content = doc.content.clone();
    drop(doc);

    let (identifier_name, identifier_range) =
        get_identifier_at_position_internal(state, uri, position, &tree, &content).await?;

    let (module_path, original_name) = match import_ctx.aliases.get(&identifier_name) {
        Some(alias) => alias.clone(),
        None => {
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

    if !module_path.starts_with("./") && !module_path.starts_with("../") {
        return None;
    }

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
                module_path,
            },
        }),
        _ => None,
    }
}

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

    let rope = ropey::Rope::from_str(content);
    let line_start = rope.try_line_to_char(position.line as usize).ok()?;
    let char_offset = line_start + position.character as usize;
    let byte_offset = rope.try_char_to_byte(char_offset).ok()?;

    let (object_name, property_name) =
        language.extract_property_access(tree, content, byte_offset)?;

    let (module_path, original_name) = import_ctx.aliases.get(object_name.as_str())?;

    if !module_path.starts_with("./") && !module_path.starts_with("../") {
        return None;
    }

    let cross_resolver = CrossModuleResolver::new(
        state.workspace_index.clone(),
        state.module_resolver.clone(),
        state.languages.clone(),
    );

    let is_default = original_name == module_path;

    match cross_resolver.resolve_import(uri, module_path, original_name, is_default) {
        CrossModuleResolution::EnvObject { .. } => {
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

async fn get_identifier_at_position_internal(
    _state: &ServerState,
    _uri: &Url,
    position: Position,
    tree: &Option<tree_sitter::Tree>,
    content: &str,
) -> Option<(CompactString, Range)> {
    let tree = tree.as_ref()?;

    let rope = ropey::Rope::from_str(content);
    let line_start = rope.try_line_to_char(position.line as usize).ok()?;
    let char_offset = line_start + position.character as usize;
    let byte_offset = rope.try_char_to_byte(char_offset).ok()?;

    let node = tree
        .root_node()
        .descendant_for_byte_range(byte_offset, byte_offset)?;

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
