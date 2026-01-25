use crate::server::handlers::references::{
    find_env_definition, get_env_var_at_position, get_env_var_usages_in_file,
};
use crate::server::handlers::util::{is_valid_env_var_name, korni_span_to_range, KorniEntryExt};
use crate::server::state::ServerState;
use korni::ParseOptions;
use std::collections::HashMap;
use std::time::Instant;
use tower_lsp::lsp_types::{
    Position, PrepareRenameResponse, Range, RenameParams, TextDocumentPositionParams, TextEdit,
    Url, WorkspaceEdit,
};

pub async fn handle_prepare_rename(
    params: TextDocumentPositionParams,
    state: &ServerState,
) -> Option<PrepareRenameResponse> {
    let uri = &params.text_document.uri;
    let position = params.position;

    if is_env_file_uri(state, uri).await {
        let (env_var_name, range) = get_env_var_in_env_file(state, uri, position).await?;
        if !is_valid_env_var_name(&env_var_name) {
            return None;
        }
        return Some(PrepareRenameResponse::Range(range));
    }

    let (env_var_name, range) = get_env_var_with_range(state, uri, position)?;

    if !is_valid_env_var_name(&env_var_name) {
        return None;
    }

    Some(PrepareRenameResponse::Range(range))
}

pub async fn handle_rename(params: RenameParams, state: &ServerState) -> Option<WorkspaceEdit> {
    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let new_name = &params.new_name;

    tracing::debug!("[HANDLE_RENAME_ENTER] uri={} new_name={}", uri, new_name);
    let start = Instant::now();

    if !is_valid_env_var_name(new_name) {
        tracing::debug!(
            "[HANDLE_RENAME_EXIT] invalid_name elapsed_ms={}",
            start.elapsed().as_millis()
        );
        return None;
    }

    let is_source_env_file = is_env_file_uri(state, uri).await;

    let (old_name, source_range) = if is_source_env_file {
        let (name, range) = get_env_var_in_env_file(state, uri, position).await?;
        (name, Some(range))
    } else {
        (get_env_var_at_position(state, uri, position).await?, None)
    };

    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();

    if let Some(range) = source_range {
        changes.entry(uri.clone()).or_default().push(TextEdit {
            range,
            new_text: new_name.to_string(),
        });
    }

    let files = state.workspace_index.files_for_env_var(&old_name);
    for file_uri in &files {
        let edits = collect_rename_edits(state, file_uri, &old_name, new_name).await;
        if !edits.is_empty() {
            changes.insert(file_uri.clone(), edits);
        }
    }

    if let Some(def_location) = find_env_definition(state, &old_name).await {
        if !changes.contains_key(&def_location.uri) {
            changes
                .entry(def_location.uri.clone())
                .or_default()
                .push(TextEdit {
                    range: def_location.range,
                    new_text: new_name.to_string(),
                });
        }
    }

    if changes.is_empty() {
        tracing::debug!(
            "[HANDLE_RENAME_EXIT] no_changes elapsed_ms={}",
            start.elapsed().as_millis()
        );
        None
    } else {
        tracing::debug!(
            "[HANDLE_RENAME_EXIT] files={} elapsed_ms={}",
            changes.len(),
            start.elapsed().as_millis()
        );
        Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        })
    }
}

async fn is_env_file_uri(state: &ServerState, uri: &Url) -> bool {
    let file_name = match uri
        .to_file_path()
        .ok()
        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
    {
        Some(name) => name,
        None => return false,
    };

    let config = state.config.get_config();
    let config = config.read().await;
    config.workspace.env_files.iter().any(|pattern| {
        glob::Pattern::new(pattern)
            .map(|p| p.matches(&file_name))
            .unwrap_or(false)
    })
}

async fn get_env_var_in_env_file(
    state: &ServerState,
    uri: &Url,
    position: Position,
) -> Option<(String, Range)> {
    let content = {
        let doc_content = state
            .document_manager
            .get(uri)
            .map(|doc| doc.content.clone());
        match doc_content {
            Some(c) => c,
            None => {
                if let Ok(path) = uri.to_file_path() {
                    std::sync::Arc::new(tokio::fs::read_to_string(&path).await.ok()?)
                } else {
                    return None;
                }
            }
        }
    };

    let entries = korni::parse_with_options(&content, ParseOptions::full());

    for kv in entries.into_iter().filter_map(|e| e.as_valid_pair()) {
        if let Some(key_span) = kv.key_span {
            let range = korni_span_to_range(&content, key_span);

            if position.line >= range.start.line
                && position.line <= range.end.line
                && position.character >= range.start.character
                && position.character <= range.end.character
            {
                return Some((kv.key.as_ref().to_string(), range));
            }
        }
    }

    None
}

fn get_env_var_with_range(
    state: &ServerState,
    uri: &Url,
    position: Position,
) -> Option<(String, Range)> {
    if let Some(reference) = state
        .document_manager
        .get_env_reference_cloned(uri, position)
    {
        return Some((reference.name.to_string(), reference.name_range));
    }

    if let Some(binding) = state.document_manager.get_env_binding_cloned(uri, position) {
        let range = binding
            .destructured_key_range
            .unwrap_or(binding.binding_range);
        return Some((binding.env_var_name.to_string(), range));
    }

    if let Some(usage) = state
        .document_manager
        .get_binding_usage_cloned(uri, position)
    {
        return Some((usage.env_var_name.to_string(), usage.range));
    }

    None
}

async fn collect_rename_edits(
    state: &ServerState,
    uri: &Url,
    old_name: &str,
    new_name: &str,
) -> Vec<TextEdit> {
    let mut edits = Vec::new();

    let usages = get_env_var_usages_in_file(state, uri, old_name).await;

    for usage in usages {
        let edit = match usage.kind {
            crate::analysis::resolver::UsageKind::DirectReference
            | crate::analysis::resolver::UsageKind::PropertyAccess
            | crate::analysis::resolver::UsageKind::BindingDeclaration => TextEdit {
                range: usage.range,
                new_text: new_name.to_string(),
            },
            crate::analysis::resolver::UsageKind::BindingUsage => {
                continue;
            }
        };

        edits.push(edit);
    }

    edits
}
