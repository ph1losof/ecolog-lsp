use crate::analysis::{CrossModuleResolution, CrossModuleResolver};
use crate::server::handlers::util::{
    format_hover_markdown, get_identifier_at_position, resolve_env_var_value,
};
use crate::server::state::ServerState;
use crate::types::ImportContext;
use std::time::Instant;
use tower_lsp::lsp_types::{
    Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, Position, Range, Url,
};

/// Context for hover operations on imported env object properties
struct ImportedEnvPropertyHoverContext<'a> {
    uri: &'a Url,
    position: Position,
    property_name: &'a compact_str::CompactString,
    property_range: &'a Range,
    import_ctx: &'a ImportContext,
    tree: &'a Option<tree_sitter::Tree>,
    content: &'a str,
}

pub async fn handle_hover(params: HoverParams, state: &ServerState) -> Option<Hover> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    tracing::debug!(
        "[HANDLE_HOVER_ENTER] uri={} pos={}:{}",
        uri,
        position.line,
        position.character
    );
    let start = Instant::now();

    if !state.config.is_hover_enabled() {
        tracing::debug!(
            "[HANDLE_HOVER_EXIT] disabled elapsed_ms={}",
            start.elapsed().as_millis()
        );
        return None;
    }

    let (env_var_name, binding_name, hover_range, is_binding, binding_kind) =
        if let Some(reference) = state
            .document_manager
            .get_env_reference_cloned(uri, position)
        {
            (reference.name, None, reference.name_range, false, None)
        } else if let Some(binding) = state.document_manager.get_env_binding_cloned(uri, position) {
            (
                binding.env_var_name,
                Some(binding.binding_name),
                binding.binding_range,
                true,
                Some(binding.kind),
            )
        } else if let Some(usage) = state
            .document_manager
            .get_binding_usage_cloned(uri, position)
        {
            let kind = state
                .document_manager
                .get_binding_kind_for_usage(uri, &usage.name);
            (
                usage.env_var_name,
                Some(usage.name),
                usage.range,
                true,
                kind,
            )
        } else {
            return handle_hover_cross_module(params, state).await;
        };

    let file_path = uri.to_file_path().ok()?;

    if let Some(resolved) = resolve_env_var_value(&env_var_name, &file_path, state).await {
        let markdown = if is_binding {
            let b_name = binding_name.as_deref().unwrap_or(env_var_name.as_str());
            format_hover_markdown(&env_var_name, Some(b_name), &resolved)
        } else {
            format_hover_markdown(&env_var_name, None, &resolved)
        };

        tracing::debug!(
            "[HANDLE_HOVER_EXIT] found elapsed_ms={}",
            start.elapsed().as_millis()
        );
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown,
            }),
            range: Some(hover_range),
        })
    } else {
        if let Some(crate::types::BindingKind::Object) = binding_kind {
            let b_name = binding_name.as_deref().unwrap_or(env_var_name.as_str());

            let msg = format!(
                "**`{}`** → **`{}`**\n\n*Environment Object*",
                b_name, env_var_name
            );

            tracing::debug!(
                "[HANDLE_HOVER_EXIT] object_alias elapsed_ms={}",
                start.elapsed().as_millis()
            );
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: msg,
                }),
                range: Some(hover_range),
            });
        }

        tracing::debug!(
            "[HANDLE_HOVER_EXIT] not_found elapsed_ms={}",
            start.elapsed().as_millis()
        );
        None
    }
}

async fn handle_hover_cross_module(params: HoverParams, state: &ServerState) -> Option<Hover> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    tracing::debug!("[HANDLE_HOVER_CROSS_MODULE_ENTER] uri={}", uri);

    let (import_ctx, tree, content) = {
        let doc = state.document_manager.get(uri)?;
        (
            doc.import_context.clone(),
            doc.tree.clone(),
            doc.content.clone(),
        )
    };

    let (identifier_name, identifier_range) =
        get_identifier_at_position(state, uri, position).await?;

    let (module_path, original_name) = match import_ctx.aliases.get(&identifier_name) {
        Some(alias) => alias.clone(),
        None => {
            let ctx = ImportedEnvPropertyHoverContext {
                uri,
                position,
                property_name: &identifier_name,
                property_range: &identifier_range,
                import_ctx: &import_ctx,
                tree: &tree,
                content: &content,
            };
            return handle_hover_on_imported_env_object_property(&ctx, state).await;
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
        } => {
            let file_path = uri.to_file_path().ok()?;
            let resolved = resolve_env_var_value(&env_var_name, &file_path, state).await?;
            let markdown =
                format_hover_markdown(&env_var_name, Some(identifier_name.as_str()), &resolved);

            Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: markdown,
                }),
                range: Some(identifier_range),
            })
        }
        CrossModuleResolution::EnvObject { canonical_name, .. } => {
            let header = if identifier_name.as_str() != canonical_name.as_str() {
                format!("**`{}`** → **`{}`**", identifier_name, canonical_name)
            } else {
                format!("**`{}`**", canonical_name)
            };
            let markdown = format!("{}\n\n*Environment Object*", header);

            Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: markdown,
                }),
                range: Some(identifier_range),
            })
        }
        CrossModuleResolution::Unresolved => None,
    }
}

async fn handle_hover_on_imported_env_object_property(
    ctx: &ImportedEnvPropertyHoverContext<'_>,
    state: &ServerState,
) -> Option<Hover> {
    let tree = ctx.tree.as_ref()?;

    let language = state.languages.get_for_uri(ctx.uri)?;

    let rope = ropey::Rope::from_str(ctx.content);
    let line_start = rope.try_line_to_char(ctx.position.line as usize).ok()?;
    let char_offset = line_start + ctx.position.character as usize;
    let byte_offset = rope.try_char_to_byte(char_offset).ok()?;

    let (object_name, _extracted_property) =
        language.extract_property_access(tree, ctx.content, byte_offset)?;

    let (module_path, original_name) = ctx.import_ctx.aliases.get(object_name.as_str())?;

    if !module_path.starts_with("./") && !module_path.starts_with("../") {
        return None;
    }

    let cross_resolver = CrossModuleResolver::new(
        state.workspace_index.clone(),
        state.module_resolver.clone(),
        state.languages.clone(),
    );

    let is_default = original_name == module_path;

    match cross_resolver.resolve_import(ctx.uri, module_path, original_name, is_default) {
        CrossModuleResolution::EnvObject { .. } => {
            let env_var_name = ctx.property_name.as_str();
            let file_path = ctx.uri.to_file_path().ok()?;

            let markdown = if let Some(resolved) =
                resolve_env_var_value(env_var_name, &file_path, state).await
            {
                format_hover_markdown(env_var_name, None, &resolved)
            } else {
                format!(
                    "**`{}`**\n\n*Environment variable not found in sources*",
                    env_var_name
                )
            };

            Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: markdown,
                }),
                range: Some(*ctx.property_range),
            })
        }
        _ => None,
    }
}
