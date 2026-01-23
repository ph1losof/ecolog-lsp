use crate::analysis::{
    AnalysisPipeline, BindingGraph, BindingResolver, CrossModuleResolution, CrossModuleResolver,
};
use crate::server::handlers::util::{get_identifier_at_position, korni_span_to_range};
use crate::server::state::ServerState;
use crate::types::ImportContext;
use korni::ParseOptions;
use std::time::Instant;
use tower_lsp::lsp_types::{
    Location, Position, Range, ReferenceParams, SymbolInformation, SymbolKind as LspSymbolKind,
    Url, WorkspaceSymbolParams,
};

pub async fn handle_references(
    params: ReferenceParams,
    state: &ServerState,
) -> Option<Vec<Location>> {
    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let include_declaration = params.context.include_declaration;

    tracing::debug!(
        "[HANDLE_REFERENCES_ENTER] uri={} pos={}:{}",
        uri,
        position.line,
        position.character
    );
    let start = Instant::now();

    let env_var_name = match get_env_var_at_position(state, uri, position).await {
        Some(name) => name,
        None => {
            tracing::debug!(
                "[HANDLE_REFERENCES_EXIT] no_env_var elapsed_ms={}",
                start.elapsed().as_millis()
            );
            return None;
        }
    };

    let files = state.workspace_index.files_for_env_var(&env_var_name);

    let mut locations = Vec::new();

    for file_uri in &files {
        let usages = get_env_var_usages_in_file(state, file_uri, &env_var_name).await;
        for usage in usages {
            if matches!(
                usage.kind,
                crate::analysis::resolver::UsageKind::BindingUsage
            ) {
                continue;
            }
            locations.push(Location {
                uri: file_uri.clone(),
                range: usage.range,
            });
        }
    }

    if include_declaration {
        if let Some(def_location) = find_env_definition(state, &env_var_name).await {
            if !locations.iter().any(|loc| loc == &def_location) {
                locations.push(def_location);
            }
        }
    }

    if locations.is_empty() {
        tracing::debug!(
            "[HANDLE_REFERENCES_EXIT] none elapsed_ms={}",
            start.elapsed().as_millis()
        );
        None
    } else {
        tracing::debug!(
            "[HANDLE_REFERENCES_EXIT] count={} elapsed_ms={}",
            locations.len(),
            start.elapsed().as_millis()
        );
        Some(locations)
    }
}

pub(crate) async fn get_env_var_at_position(
    state: &ServerState,
    uri: &Url,
    position: Position,
) -> Option<String> {
    if let Some(reference) = state
        .document_manager
        .get_env_reference_cloned(uri, position)
    {
        return Some(reference.name.to_string());
    }

    if let Some(binding) = state.document_manager.get_env_binding_cloned(uri, position) {
        return Some(binding.env_var_name.to_string());
    }

    if let Some(usage) = state
        .document_manager
        .get_binding_usage_cloned(uri, position)
    {
        return Some(usage.env_var_name.to_string());
    }

    if let Some(env_var_name) = get_env_var_from_cross_module(state, uri, position).await {
        return Some(env_var_name);
    }

    None
}

async fn get_env_var_from_cross_module(
    state: &ServerState,
    uri: &Url,
    position: Position,
) -> Option<String> {
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
        CrossModuleResolution::EnvVar { name, .. } => Some(name.to_string()),
        _ => None,
    }
}

pub(crate) async fn get_env_var_usages_in_file(
    state: &ServerState,
    uri: &Url,
    env_var_name: &str,
) -> Vec<crate::analysis::resolver::EnvVarUsageLocation> {
    if let Some(graph_ref) = state.document_manager.get_binding_graph(uri) {
        let resolver = BindingResolver::new(&*graph_ref);
        return resolver.find_env_var_usages(env_var_name);
    }

    if let Some(graph) = parse_file_for_binding_graph(state, uri).await {
        let resolver = BindingResolver::new(&graph);
        return resolver.find_env_var_usages(env_var_name);
    }

    Vec::new()
}

async fn parse_file_for_binding_graph(state: &ServerState, uri: &Url) -> Option<BindingGraph> {
    let path = uri.to_file_path().ok()?;
    let content = tokio::fs::read_to_string(&path).await.ok()?;
    let lang = state.languages.get_for_uri(uri)?;

    let query_engine = state.document_manager.query_engine();
    let tree = query_engine.parse(lang.as_ref(), &content, None).await?;

    let graph = AnalysisPipeline::analyze(
        query_engine,
        lang.as_ref(),
        &tree,
        content.as_bytes(),
        &ImportContext::default(),
    )
    .await;

    Some(graph)
}

pub(crate) async fn find_env_definition(state: &ServerState, env_var_name: &str) -> Option<Location> {
    let workspace_root = crate::server::util::get_workspace_root(&state.core.workspace).await;

    let env_patterns: Vec<String> = {
        let config = state.config.get_config();
        let config = config.read().await;
        config
            .workspace
            .env_files
            .iter()
            .map(|s| s.to_string())
            .collect()
    };

    for pattern in env_patterns {
        let env_path = workspace_root.join(&pattern);
        if env_path.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&env_path).await {
                let entries = korni::parse_with_options(&content, ParseOptions::full());

                for entry in entries {
                    if let korni::Entry::Pair(kv) = entry {
                        if kv.key.as_ref() == env_var_name {
                            if let Some(key_span) = kv.key_span {
                                let range = korni_span_to_range(&content, key_span);
                                let uri = Url::from_file_path(&env_path).ok()?;
                                return Some(Location { uri, range });
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

#[allow(deprecated)]
pub async fn handle_workspace_symbol(
    params: WorkspaceSymbolParams,
    state: &ServerState,
) -> Option<Vec<SymbolInformation>> {
    let query = params.query.to_lowercase();
    tracing::debug!("[HANDLE_WORKSPACE_SYMBOL_ENTER] query={}", query);
    let start = Instant::now();

    let all_vars = state.workspace_index.all_env_vars();

    if all_vars.is_empty() {
        tracing::debug!(
            "[HANDLE_WORKSPACE_SYMBOL_EXIT] empty elapsed_ms={}",
            start.elapsed().as_millis()
        );
        return None;
    }

    let mut symbols = Vec::new();

    for var_name in all_vars {
        if !query.is_empty() && !var_name.to_lowercase().contains(&query) {
            continue;
        }

        let location = if let Some(def_location) = find_env_definition(state, &var_name).await {
            def_location
        } else {
            let files = state.workspace_index.files_for_env_var(&var_name);
            if let Some(first_file) = files.first() {
                let usages = get_env_var_usages_in_file(state, first_file, &var_name).await;
                if let Some(first_usage) = usages.first() {
                    Location {
                        uri: first_file.clone(),
                        range: first_usage.range,
                    }
                } else {
                    Location {
                        uri: first_file.clone(),
                        range: Range::default(),
                    }
                }
            } else {
                continue;
            }
        };

        symbols.push(SymbolInformation {
            name: var_name.to_string(),
            kind: LspSymbolKind::CONSTANT,
            location,
            tags: None,
            deprecated: None,
            container_name: Some("Environment Variables".to_string()),
        });
    }

    if symbols.is_empty() {
        tracing::debug!(
            "[HANDLE_WORKSPACE_SYMBOL_EXIT] none elapsed_ms={}",
            start.elapsed().as_millis()
        );
        None
    } else {
        symbols.sort_by(|a, b| a.name.cmp(&b.name));
        tracing::debug!(
            "[HANDLE_WORKSPACE_SYMBOL_EXIT] count={} elapsed_ms={}",
            symbols.len(),
            start.elapsed().as_millis()
        );
        Some(symbols)
    }
}
