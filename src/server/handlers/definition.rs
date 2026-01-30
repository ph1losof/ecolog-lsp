use crate::analysis::{CrossModuleResolution, CrossModuleResolver};
use crate::server::handlers::util::get_identifier_at_position;
use crate::server::state::ServerState;
use abundantis::source::VariableSource;
use std::time::Instant;
use tower_lsp::lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Location, Position, Range, Url};

pub async fn handle_definition(
    params: GotoDefinitionParams,
    state: &ServerState,
) -> Option<GotoDefinitionResponse> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    tracing::debug!(
        "[HANDLE_DEFINITION_ENTER] uri={} pos={}:{}",
        uri,
        position.line,
        position.character
    );
    let start = Instant::now();

    if !state.config.is_definition_enabled() {
        tracing::debug!(
            "[HANDLE_DEFINITION_EXIT] disabled elapsed_ms={}",
            start.elapsed().as_millis()
        );
        return None;
    }

    let env_var_name = if let Some(reference) = state
        .document_manager
        .get_env_reference_cloned(uri, position)
    {
        reference.name
    } else if let Some(binding) = state.document_manager.get_env_binding_cloned(uri, position) {
        binding.env_var_name
    } else if let Some(usage) = state
        .document_manager
        .get_binding_usage_cloned(uri, position)
    {
        usage.env_var_name
    } else {
        return handle_definition_cross_module(&params, state).await;
    };

    let file_path = uri.to_file_path().ok()?;

    if let Some(variable) =
        crate::server::util::safe_get_for_file(&state.core, &env_var_name, &file_path).await
    {
        match &variable.source {
            VariableSource::File { path, offset } => {
                let target_uri = Url::from_file_path(path).ok()?;

                let content = std::fs::read_to_string(path).ok()?;
                let (line, char) = crate::server::util::offset_to_linecol(&content, *offset);

                let range = Range::new(
                    Position::new(line, char),
                    Position::new(line, char + variable.key.len() as u32),
                );

                tracing::debug!(
                    "[HANDLE_DEFINITION_EXIT] found elapsed_ms={}",
                    start.elapsed().as_millis()
                );
                Some(GotoDefinitionResponse::Scalar(Location {
                    uri: target_uri,
                    range,
                }))
            }
            _ => {
                tracing::debug!(
                    "[HANDLE_DEFINITION_EXIT] non_file_source elapsed_ms={}",
                    start.elapsed().as_millis()
                );
                None
            }
        }
    } else {
        tracing::debug!(
            "[HANDLE_DEFINITION_EXIT] not_found elapsed_ms={}",
            start.elapsed().as_millis()
        );
        None
    }
}

async fn handle_definition_cross_module(
    params: &GotoDefinitionParams,
    state: &ServerState,
) -> Option<GotoDefinitionResponse> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    let import_ctx = {
        let doc = state.document_manager.get(uri)?;
        doc.import_context.clone()
    };

    let (identifier_name, _) = get_identifier_at_position(state, uri, position).await?;

    let (module_path, original_name) = import_ctx.aliases.get(&identifier_name)?.clone();

    if !module_path.starts_with("./") && !module_path.starts_with("../") {
        return None;
    }

    let cross_resolver = CrossModuleResolver::new(
        state.workspace_index.clone(),
        state.module_resolver.clone(),
        state.languages.clone(),
    );

    let is_default = original_name == identifier_name;

    match cross_resolver.resolve_import(uri, &module_path, &original_name, is_default) {
        CrossModuleResolution::EnvVar {
            name: env_var_name,
            defining_file,
            declaration_range,
        } => {
            let file_path = uri.to_file_path().ok()?;

            let workspace = std::sync::Arc::clone(&state.core.workspace);
            let fp = file_path.clone();
            let context =
                tokio::task::spawn_blocking(move || workspace.read().context_for_file(&fp))
                    .await
                    .ok()??;

            let registry = &state.core.registry;
            if let Ok(Some(variable)) = state
                .core
                .resolution
                .resolve(&env_var_name, &context, registry)
                .await
            {
                if let VariableSource::File { path, offset } = &variable.source {
                    let target_uri = Url::from_file_path(path).ok()?;
                    let content = std::fs::read_to_string(path).ok()?;
                    let (line, char) = crate::server::util::offset_to_linecol(&content, *offset);

                    let range = Range::new(
                        Position::new(line, char),
                        Position::new(line, char + variable.key.len() as u32),
                    );

                    return Some(GotoDefinitionResponse::Scalar(Location {
                        uri: target_uri,
                        range,
                    }));
                }
            }

            Some(GotoDefinitionResponse::Scalar(Location {
                uri: defining_file,
                range: declaration_range,
            }))
        }
        CrossModuleResolution::EnvObject { defining_file, .. } => {
            Some(GotoDefinitionResponse::Scalar(Location {
                uri: defining_file,
                range: Range::default(),
            }))
        }
        CrossModuleResolution::Unresolved => None,
    }
}
