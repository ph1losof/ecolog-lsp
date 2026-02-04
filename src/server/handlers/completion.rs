use crate::analysis::{CrossModuleResolution, CrossModuleResolver};
use crate::server::handlers::util::format_source;
use crate::server::state::ServerState;
use std::time::Instant;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, Documentation, MarkupContent, MarkupKind,
    Position, Url,
};

pub async fn handle_completion(
    params: CompletionParams,
    state: &ServerState,
) -> Option<Vec<CompletionItem>> {
    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    tracing::debug!(
        "[HANDLE_COMPLETION_ENTER] uri={} pos={}:{}",
        uri,
        position.line,
        position.character
    );
    let start = Instant::now();

    if !state.config.is_completion_enabled() {
        tracing::debug!(
            "[HANDLE_COMPLETION_EXIT] disabled elapsed_ms={}",
            start.elapsed().as_millis()
        );
        return None;
    }

    let is_strict = {
        let config = state.config.get_config();
        let config = config.read().await;
        config.strict.completion
    };

    if is_strict {
        let position = params.text_document_position.position;
        if !state.document_manager.check_completion(uri, position).await && !check_imported_env_object_completion(uri, position, state).await {
            return None;
        }
    }

    let file_path = uri.to_file_path().ok()?;

    let workspace_root = crate::server::util::get_workspace_root(&state.core.workspace).await;

    let start = Instant::now();
    let all_vars = crate::server::util::safe_all_for_file(&state.core, &file_path).await;
    let elapsed = start.elapsed();
    if elapsed.as_millis() > 100 {
        tracing::warn!(
            "Slow completion resolution: {} took {:?}",
            file_path.display(),
            elapsed
        );
    }

    if !all_vars.is_empty() {
        let count = all_vars.len();
        let result = Some(
            all_vars
                .into_iter()
                .map(|var| {
                    let value = var.resolved_value.to_string();
                    let source_str = format_source(&var.source, &workspace_root);

                    let value_formatted = if value.is_empty() {
                        "*(empty)*".to_string()
                    } else if value.contains('\n') {
                        format!("`{}`", value.replace('\n', "`\n`"))
                    } else {
                        format!("`{}`", value)
                    };

                    let mut doc = format!(
                        "**Value**: {}\n\n**Source**: `{}`",
                        value_formatted, source_str
                    );
                    if let Some(desc) = &var.description {
                        if !desc.is_empty() {
                            doc.push_str(&format!("\n\n*{}*", desc));
                        }
                    }

                    CompletionItem {
                        label: var.key.to_string(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: None,
                        documentation: Some(Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: doc,
                        })),
                        ..Default::default()
                    }
                })
                .collect(),
        );
        tracing::debug!(
            "[HANDLE_COMPLETION_EXIT] count={} elapsed_ms={}",
            count,
            start.elapsed().as_millis()
        );
        result
    } else {
        tracing::debug!(
            "[HANDLE_COMPLETION_EXIT] none elapsed_ms={}",
            start.elapsed().as_millis()
        );
        None
    }
}

async fn check_imported_env_object_completion(
    uri: &Url,
    position: Position,
    state: &ServerState,
) -> bool {
    let obj_name = match state
        .document_manager
        .check_completion_context(uri, position)
        .await
    {
        Some(name) => name,
        None => return false,
    };

    let import_ctx = match state.document_manager.get(uri) {
        Some(doc) => doc.import_context.clone(),
        None => return false,
    };

    let (module_path, original_name) = match import_ctx.aliases.get(obj_name.as_str()) {
        Some(alias) => alias.clone(),
        None => return false,
    };

    if !module_path.starts_with("./") && !module_path.starts_with("../") {
        return false;
    }

    let cross_resolver = CrossModuleResolver::new(
        state.workspace_index.clone(),
        state.module_resolver.clone(),
        state.languages.clone(),
    );

    let is_default = original_name == module_path;

    matches!(
        cross_resolver.resolve_import(uri, &module_path, &original_name, is_default),
        CrossModuleResolution::EnvObject { .. }
    )
}
