use crate::analysis::{
    AnalysisPipeline, BindingGraph, BindingResolver, CrossModuleResolution, CrossModuleResolver,
};
use crate::server::state::ServerState;
use crate::types::ImportContext;
use abundantis::source::VariableSource;
use korni::{Error as KorniError, ParseOptions};
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, Diagnostic, DiagnosticSeverity,
    Documentation, ExecuteCommandParams, Hover, HoverContents, HoverParams, Location,
    MarkupContent, MarkupKind, NumberOrString, Position, PrepareRenameResponse, Range,
    ReferenceParams, RenameParams, SymbolInformation, SymbolKind as LspSymbolKind,
    TextDocumentPositionParams, TextEdit, Url, WorkspaceEdit, WorkspaceSymbolParams,
};

fn format_source(source: &VariableSource, root: &Path) -> String {
    match source {
        VariableSource::File { path, .. } => {
            let display_path = if let Ok(relative) = path.strip_prefix(root) {
                relative.display().to_string()
            } else {
                path.display().to_string()
            };
            display_path
        }
        VariableSource::Shell => "System Environment".to_string(),
        VariableSource::Memory => "In-Memory".to_string(),
        VariableSource::Remote { provider, path } => {
            if let Some(p) = path {
                format!("Remote ({}: {})", provider, p)
            } else {
                format!("Remote ({})", provider)
            }
        }
    }
}

struct ResolvedEnvVarValue {
    value: String,

    source: String,

    description: Option<compact_str::CompactString>,
}

async fn resolve_env_var_value(
    env_var_name: &str,
    file_path: &Path,
    state: &ServerState,
) -> Option<ResolvedEnvVarValue> {
    let start = Instant::now();
    let resolved =
        crate::server::util::safe_get_for_file(&state.core, env_var_name, file_path).await?;
    let elapsed = start.elapsed();
    if elapsed.as_millis() > 100 {
        tracing::warn!(
            "Slow env var resolution: {} took {:?} for '{}'",
            file_path.display(),
            elapsed,
            env_var_name
        );
    }

    let workspace_root = crate::server::util::get_workspace_root(&state.core.workspace).await;

    let source_str = format_source(&resolved.source, &workspace_root);

    Some(ResolvedEnvVarValue {
        value: resolved.resolved_value.to_string(),
        source: source_str,
        description: resolved.description.clone(),
    })
}

fn format_hover_markdown(
    env_var_name: &str,
    identifier_name: Option<&str>,
    resolved: &ResolvedEnvVarValue,
) -> String {
    let header = match identifier_name {
        Some(id) if id != env_var_name => format!("**`{}`** → **`{}`**", id, env_var_name),
        _ => format!("**`{}`**", env_var_name),
    };

    let value_formatted = if resolved.value.contains('\n') {
        format!("`{}`", resolved.value.replace('\n', "`\n`"))
    } else {
        format!("`{}`", resolved.value)
    };

    let mut markdown = format!(
        "{}\n\n**Value**: {}\n\n**Source**: `{}`",
        header, value_formatted, resolved.source
    );

    if let Some(desc) = &resolved.description {
        markdown.push_str(&format!("\n\n*{}*", desc));
    }

    markdown
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

    {
        let config = state.config.get_config();
        let config = config.read().await;
        if !config.features.hover {
            tracing::debug!(
                "[HANDLE_HOVER_EXIT] disabled elapsed_ms={}",
                start.elapsed().as_millis()
            );
            return None;
        }
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
            let value_formatted = if resolved.value.contains('\n') {
                format!("`{}`", resolved.value.replace('\n', "`\n`"))
            } else {
                format!("`{}`", resolved.value)
            };
            let mut md = format!(
                "**`{}`**\n\n**Value**: {}\n\n**Source**: `{}`",
                env_var_name, value_formatted, resolved.source
            );
            if let Some(desc) = &resolved.description {
                md.push_str(&format!("\n\n*{}*", desc));
            }
            md
        };

        tracing::debug!(
            "[HANDLE_HOVER_EXIT] found elapsed_ms={}",
            start.elapsed().as_millis()
        );
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown,
            }),
            range: Some(hover_range),
        });
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
        return None;
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
            return handle_hover_on_imported_env_object_property(
                uri,
                position,
                &identifier_name,
                &identifier_range,
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
    uri: &Url,
    position: Position,
    property_name: &compact_str::CompactString,
    property_range: &Range,
    import_ctx: &ImportContext,
    tree: &Option<tree_sitter::Tree>,
    content: &str,
    state: &ServerState,
) -> Option<Hover> {
    let tree = tree.as_ref()?;

    let language = state.languages.get_for_uri(uri)?;

    let rope = ropey::Rope::from_str(content);
    let line_start = rope.try_line_to_char(position.line as usize).ok()?;
    let char_offset = line_start + position.character as usize;
    let byte_offset = rope.try_char_to_byte(char_offset).ok()?;

    let (object_name, _extracted_property) =
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
            let env_var_name = property_name.as_str();
            let file_path = uri.to_file_path().ok()?;

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
                range: Some(*property_range),
            })
        }
        _ => None,
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

async fn get_identifier_at_position(
    state: &ServerState,
    uri: &Url,
    position: Position,
) -> Option<(compact_str::CompactString, Range)> {
    let doc = state.document_manager.get(uri)?;
    let tree = doc.tree.as_ref()?;
    let content = &doc.content;

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
        return Some((compact_str::CompactString::from(name), range));
    }

    None
}

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

    let is_strict = {
        let config_manager = state.config.get_config();
        let config = config_manager.read().await;
        if !config.features.completion {
            tracing::debug!(
                "[HANDLE_COMPLETION_EXIT] disabled elapsed_ms={}",
                start.elapsed().as_millis()
            );
            return None;
        }
        config.strict.completion
    };

    if is_strict {
        let position = params.text_document_position.position;
        if !state.document_manager.check_completion(uri, position).await {
            if !check_imported_env_object_completion(uri, position, state).await {
                return None;
            }
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

                    let value_formatted = if value.contains('\n') {
                        format!("`{}`", value.replace('\n', "`\n`"))
                    } else {
                        format!("`{}`", value)
                    };

                    let mut doc = format!(
                        "**Value**: {}\n\n**Source**: `{}`",
                        value_formatted, source_str
                    );
                    if let Some(desc) = &var.description {
                        doc.push_str(&format!("\n\n*{}*", desc));
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

pub async fn handle_definition(
    params: tower_lsp::lsp_types::GotoDefinitionParams,
    state: &ServerState,
) -> Option<tower_lsp::lsp_types::GotoDefinitionResponse> {
    use tower_lsp::lsp_types::{GotoDefinitionResponse, Location, Position, Range, Url};

    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;
    tracing::debug!(
        "[HANDLE_DEFINITION_ENTER] uri={} pos={}:{}",
        uri,
        position.line,
        position.character
    );
    let start = Instant::now();

    {
        let config = state.config.get_config();
        let config = config.read().await;
        if !config.features.definition {
            tracing::debug!(
                "[HANDLE_DEFINITION_EXIT] disabled elapsed_ms={}",
                start.elapsed().as_millis()
            );
            return None;
        }
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
    params: &tower_lsp::lsp_types::GotoDefinitionParams,
    state: &ServerState,
) -> Option<tower_lsp::lsp_types::GotoDefinitionResponse> {
    use tower_lsp::lsp_types::{GotoDefinitionResponse, Location};

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

pub async fn compute_diagnostics(
    uri: &tower_lsp::lsp_types::Url,
    state: &ServerState,
) -> Vec<Diagnostic> {
    use tower_lsp::lsp_types::{Position, Range};

    tracing::debug!("[COMPUTE_DIAGNOSTICS_ENTER] uri={}", uri);
    let start = Instant::now();

    {
        let config = state.config.get_config();
        let config = config.read().await;
        if !config.features.diagnostics {
            tracing::debug!(
                "[COMPUTE_DIAGNOSTICS_EXIT] disabled elapsed_ms={}",
                start.elapsed().as_millis()
            );
            return vec![];
        }
    }

    let mut diagnostics = Vec::new();

    let content = {
        let doc_ref = state.document_manager.get(uri);
        let Some(doc) = doc_ref else {
            tracing::debug!("Document not found for diagnostics: {}", uri);
            return vec![];
        };
        doc.content.clone()
    };

    let (references, env_var_symbols, property_accesses): (
        Vec<crate::types::EnvReference>,
        Vec<(compact_str::CompactString, tower_lsp::lsp_types::Range)>,
        Vec<(compact_str::CompactString, tower_lsp::lsp_types::Range)>,
    ) = {
        if let Some(graph) = state.document_manager.get_binding_graph(uri) {
            let refs = graph.direct_references().to_vec();

            let symbols: Vec<_> = graph
                .symbols()
                .iter()
                .filter_map(|s| {
                    if let crate::types::SymbolOrigin::EnvVar { name } = &s.origin {
                        Some((name.clone(), s.name_range))
                    } else {
                        None
                    }
                })
                .collect();

            let prop_accesses: Vec<_> = graph
                .usages()
                .iter()
                .filter_map(|usage| {
                    let prop_name = usage.property_access.as_ref()?;

                    let symbol = graph.get_symbol(usage.symbol_id)?;
                    if matches!(
                        graph.resolve_to_env(symbol.id),
                        Some(crate::types::ResolvedEnv::Object(_))
                    ) {
                        let range = usage.property_access_range.unwrap_or(usage.range);
                        Some((prop_name.clone(), range))
                    } else {
                        None
                    }
                })
                .collect();
            (refs, symbols, prop_accesses)
        } else {
            (vec![], vec![], vec![])
        }
    };

    let file_path = if let Ok(p) = uri.to_file_path() {
        p
    } else {
        return vec![];
    };
    let file_name = file_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let is_env_file = {
        let config = state.config.get_config();
        let config = config.read().await;
        config.workspace.env_files.iter().any(|pattern| {
            glob::Pattern::new(pattern)
                .map(|p| p.matches(&file_name))
                .unwrap_or(false)
        })
    };

    if is_env_file {
        let entries = korni::parse_with_options(&content, ParseOptions::full());
        for entry in entries {
            if let korni::Entry::Error(err) = entry {
                let (msg, code, severity) = match &err {
                    KorniError::ForbiddenWhitespace { .. } => {
                        ("Forbidden whitespace", "EDF001", DiagnosticSeverity::ERROR)
                    }
                    KorniError::DoubleEquals { .. } => (
                        "Double equals sign detected",
                        "EDF002",
                        DiagnosticSeverity::ERROR,
                    ),
                    KorniError::Generic { message, .. } if message == "Empty key" => {
                        ("Empty key", "EDF003", DiagnosticSeverity::ERROR)
                    }
                    KorniError::InvalidKey { .. } => (
                        "Invalid character in key",
                        "EDF004",
                        DiagnosticSeverity::ERROR,
                    ),
                    KorniError::UnclosedQuote { .. } => {
                        ("Unclosed quote", "EDF005", DiagnosticSeverity::ERROR)
                    }
                    KorniError::InvalidUtf8 { .. } => (
                        "Invalid UTF-8 sequence",
                        "EDF006",
                        DiagnosticSeverity::WARNING,
                    ),
                    KorniError::Expected { .. } => {
                        ("Syntax error", "EDF999", DiagnosticSeverity::ERROR)
                    }
                    _ => ("Syntax Error", "EDF999", DiagnosticSeverity::ERROR),
                };

                let offset = err.offset();

                let (line, col) = get_line_col(&content, offset);

                let range = Range {
                    start: Position::new(line, col),
                    end: Position::new(line, col + 1),
                };

                diagnostics.push(Diagnostic {
                    range,
                    severity: Some(severity),
                    code: Some(NumberOrString::String(code.to_string())),
                    source: Some("ecolog-linter".to_string()),
                    message: format!("{}: {}", msg, err),
                    ..Default::default()
                });
            }
        }
    }

    if !is_env_file {
        for reference in references {
            let resolved =
                crate::server::util::safe_get_for_file(&state.core, &reference.name, &file_path)
                    .await;

            if resolved.is_none() {
                diagnostics.push(Diagnostic {
                    range: reference.name_range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String("undefined-env-var".to_string())),
                    source: Some("ecolog".to_string()),
                    message: format!("Environment variable '{}' is not defined.", reference.name),
                    ..Default::default()
                });
            }
        }

        for (env_name, range) in env_var_symbols {
            let resolved =
                crate::server::util::safe_get_for_file(&state.core, &env_name, &file_path).await;

            if resolved.is_none() {
                diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String("undefined-env-var".to_string())),
                    source: Some("ecolog".to_string()),
                    message: format!("Environment variable '{}' is not defined.", env_name),
                    ..Default::default()
                });
            }
        }

        for (env_name, range) in property_accesses {
            let resolved =
                crate::server::util::safe_get_for_file(&state.core, &env_name, &file_path).await;

            if resolved.is_none() {
                diagnostics.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String("undefined-env-var".to_string())),
                    source: Some("ecolog".to_string()),
                    message: format!("Environment variable '{}' is not defined.", env_name),
                    ..Default::default()
                });
            }
        }
    }

    tracing::debug!(
        "[COMPUTE_DIAGNOSTICS_EXIT] count={} elapsed_ms={}",
        diagnostics.len(),
        start.elapsed().as_millis()
    );
    diagnostics
}

fn get_line_col(content: &str, offset: usize) -> (u32, u32) {
    if offset >= content.len() {
        return (0, 0);
    }

    let rope = ropey::Rope::from_str(content);
    let line_idx = rope.byte_to_line(offset);
    let line_start_byte = rope.line_to_byte(line_idx);
    let col_char = rope.byte_slice(line_start_byte..offset).len_chars();

    (line_idx as u32, col_char as u32)
}

pub async fn handle_execute_command(
    params: ExecuteCommandParams,
    state: &ServerState,
) -> Option<serde_json::Value> {
    tracing::debug!("[HANDLE_EXECUTE_COMMAND_ENTER] cmd={}", params.command);
    let start = Instant::now();

    let result = handle_execute_command_inner(&params, state).await;

    tracing::debug!(
        "[HANDLE_EXECUTE_COMMAND_EXIT] cmd={} result={} elapsed_ms={}",
        params.command,
        if result.is_some() { "some" } else { "none" },
        start.elapsed().as_millis()
    );
    result
}

async fn handle_execute_command_inner(
    params: &ExecuteCommandParams,
    state: &ServerState,
) -> Option<serde_json::Value> {
    match params.command.as_str() {
        "ecolog.file.setActive" => {
            {
                let config = state.config.get_config();
                let config = config.read().await;
                if !config
                    .resolution
                    .precedence
                    .contains(&abundantis::config::SourcePrecedence::File)
                {
                    return Some(json!({ "error": "File source is not enabled in configuration" }));
                }
            }

            let patterns: Vec<String> = params
                .arguments
                .iter()
                .filter_map(|arg| arg.as_str().map(|s| s.to_string()))
                .collect();

            if patterns.is_empty() {
                state.core.clear_active_files();
                Some(json!({ "success": true, "message": "Cleared active file filter" }))
            } else {
                state.core.set_active_files(&patterns);
                Some(json!({ "success": true, "patterns": patterns }))
            }
        }
        "ecolog.listEnvVariables" => {
            let file_path = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str())
                .map(std::path::PathBuf::from);

            let root = crate::server::util::get_workspace_root(&state.core.workspace).await;

            let resolve_path = file_path.as_ref().unwrap_or(&root);
            let vars = crate::server::util::safe_all_for_file(&state.core, resolve_path).await;

            let var_list: Vec<serde_json::Value> = vars
                .iter()
                .map(|v| {
                    json!({
                        "name": v.key,
                        "value": v.resolved_value,
                        "source": format_source(&v.source, &root)
                    })
                })
                .collect();

            Some(json!({ "variables": var_list, "count": var_list.len() }))
        }
        "ecolog.generateEnvExample" => {
            let mut env_vars: Vec<String> = state
                .workspace_index
                .all_env_vars()
                .into_iter()
                .map(|s| s.to_string())
                .collect();
            env_vars.sort();

            if env_vars.is_empty() {
                return Some(json!({
                    "content": "# No environment variables found in workspace\n",
                    "count": 0
                }));
            }

            let content = env_vars
                .iter()
                .map(|var| format!("{}=", var))
                .collect::<Vec<_>>()
                .join("\n");

            Some(json!({
                "content": format!("{}\n", content),
                "count": env_vars.len()
            }))
        }
        "ecolog.file.list" => {
            {
                let config = state.config.get_config();
                let config = config.read().await;
                if !config
                    .resolution
                    .precedence
                    .contains(&abundantis::config::SourcePrecedence::File)
                {
                    return Some(json!({ "error": "File source is not enabled in configuration" }));
                }
            }

            let file_path = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str())
                .map(|s| s.to_string());

            let return_all = params
                .arguments
                .get(1)
                .and_then(|arg| arg.as_bool())
                .unwrap_or(false);

            let root = crate::server::util::get_workspace_root(&state.core.workspace).await;

            let env_file_paths: Vec<std::path::PathBuf> = if return_all {
                let all_files = state.core.registry.registered_file_paths();

                if let Some(ref fp) = file_path {
                    let workspace = std::sync::Arc::clone(&state.core.workspace);
                    let fp_path = std::path::PathBuf::from(fp.as_str());
                    let context_opt = tokio::task::spawn_blocking(move || {
                        workspace.read().context_for_file(&fp_path)
                    })
                    .await
                    .ok()
                    .flatten();

                    if let Some(context) = context_opt {
                        let package_root = context.package_root;
                        let workspace_root = context.workspace_root;

                        all_files
                            .into_iter()
                            .filter(|path| {
                                path.starts_with(&package_root)
                                    || (path.parent() == Some(workspace_root.as_path()))
                            })
                            .collect()
                    } else {
                        all_files
                    }
                } else {
                    all_files
                }
            } else if let Some(ref fp) = file_path {
                state.core.active_env_files(fp)
            } else {
                state.core.active_env_files(&root)
            };

            let env_files: Vec<String> = env_file_paths
                .iter()
                .filter_map(|path| {
                    if let Ok(relative) = path.strip_prefix(&root) {
                        Some(relative.to_string_lossy().to_string())
                    } else {
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.to_string())
                    }
                })
                .collect();

            Some(json!({ "files": env_files, "count": env_files.len() }))
        }
        "ecolog.variable.get" => {
            let var_name = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str())
                .map(|s| s.to_string());

            let Some(name) = var_name else {
                return Some(json!({ "error": "Variable name required" }));
            };

            let root = crate::server::util::get_workspace_root(&state.core.workspace).await;

            if let Some(resolved) = resolve_env_var_value(&name, &root, state).await {
                Some(json!({
                    "name": name,
                    "value": resolved.value,
                    "source": resolved.source,
                    "description": resolved.description
                }))
            } else {
                Some(json!({ "error": format!("Variable '{}' not found", name) }))
            }
        }
        "ecolog.workspace.list" => {
            let workspace = std::sync::Arc::clone(&state.core.workspace);
            let workspace_info = tokio::task::spawn_blocking(move || {
                let workspace = workspace.read();
                json!({
                    "name": workspace.root().file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("workspace"),
                    "path": workspace.root().display().to_string(),
                    "isActive": true
                })
            })
            .await
            .unwrap_or_else(|_| json!({"error": "Failed to get workspace info"}));

            Some(json!({
                "workspaces": [workspace_info],
                "count": 1
            }))
        }
        "ecolog.source.list" => {
            use abundantis::config::SourcePrecedence;
            let precedence = state.config.get_precedence().await;

            let all_sources = [
                ("Shell", SourcePrecedence::Shell, 100),
                ("File", SourcePrecedence::File, 50),
                ("Remote", SourcePrecedence::Remote, 25),
            ];

            let sources: Vec<serde_json::Value> = all_sources
                .iter()
                .map(|(name, sp, priority)| {
                    json!({
                        "name": name,
                        "enabled": precedence.contains(sp),
                        "priority": priority
                    })
                })
                .collect();

            Some(json!({
                "sources": sources,
                "count": sources.len()
            }))
        }
        "ecolog.source.setPrecedence" => {
            use abundantis::config::SourcePrecedence;

            let source_names: Vec<String> = params
                .arguments
                .iter()
                .filter_map(|arg| arg.as_str().map(|s| s.to_string()))
                .collect();

            let mut new_precedence: Vec<SourcePrecedence> = Vec::new();

            for name in &source_names {
                match name.to_lowercase().as_str() {
                    "shell" => new_precedence.push(SourcePrecedence::Shell),
                    "file" => new_precedence.push(SourcePrecedence::File),
                    "remote" => new_precedence.push(SourcePrecedence::Remote),
                    _ => {
                        return Some(json!({
                            "error": format!("Unknown source: {}. Valid sources: Shell, File, Remote", name)
                        }));
                    }
                }
            }

            state.config.set_precedence(new_precedence.clone()).await;

            let mut new_resolution_config = abundantis::config::ResolutionConfig::default();
            new_resolution_config.precedence = new_precedence.clone();
            state
                .core
                .resolution
                .update_resolution_config(new_resolution_config);

            crate::server::util::safe_refresh(
                &state.core,
                abundantis::RefreshOptions::preserve_all(),
            )
            .await;

            let enabled_names: Vec<&str> = new_precedence
                .iter()
                .map(|s| match s {
                    SourcePrecedence::Shell => "Shell",
                    SourcePrecedence::File => "File",
                    SourcePrecedence::Remote => "Remote",
                })
                .collect();

            Some(json!({
                "success": true,
                "precedence": enabled_names
            }))
        }
        "ecolog.workspace.setRoot" => {
            let path = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str())
                .map(|s| s.to_string());

            let Some(path_str) = path else {
                return Some(json!({ "error": "Path argument required" }));
            };

            let new_root = std::path::PathBuf::from(&path_str);

            if !new_root.exists() {
                return Some(json!({ "error": format!("Path does not exist: {}", path_str) }));
            }

            if !new_root.is_dir() {
                return Some(json!({ "error": format!("Path is not a directory: {}", path_str) }));
            }

            match state.core.set_root(&new_root).await {
                Ok(()) => {
                    let canonical = new_root.canonicalize().unwrap_or(new_root);
                    tracing::info!("Workspace root changed to: {:?}", canonical);
                    Some(json!({
                        "success": true,
                        "root": canonical.display().to_string()
                    }))
                }
                Err(e) => {
                    tracing::error!("Failed to set workspace root: {}", e);
                    Some(json!({ "error": format!("Failed to set root: {}", e) }))
                }
            }
        }
        "ecolog.interpolation.set" => {
            let enabled = params
                .arguments
                .first()
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            state.config.set_interpolation_enabled(enabled).await;

            let mut new_interpolation_config = abundantis::config::InterpolationConfig::default();
            new_interpolation_config.enabled = enabled;
            state
                .core
                .resolution
                .update_interpolation_config(new_interpolation_config);

            crate::server::util::safe_refresh(
                &state.core,
                abundantis::RefreshOptions::preserve_all(),
            )
            .await;

            tracing::info!("Interpolation set to: {}", enabled);

            Some(json!({
                "success": true,
                "enabled": enabled
            }))
        }
        "ecolog.interpolation.get" => {
            let enabled = state.core.resolution.interpolation_enabled();
            Some(json!({
                "enabled": enabled
            }))
        }
        _ => None,
    }
}

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

async fn get_env_var_at_position(
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

async fn get_env_var_usages_in_file(
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

async fn find_env_definition(state: &ServerState, env_var_name: &str) -> Option<Location> {
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

fn korni_span_to_range(content: &str, span: korni::Span) -> Range {
    let (start_line, start_col) = offset_to_line_col(content, span.start.offset);
    let (end_line, end_col) = offset_to_line_col(content, span.end.offset);

    Range {
        start: Position {
            line: start_line,
            character: start_col,
        },
        end: Position {
            line: end_line,
            character: end_col,
        },
    }
}

fn offset_to_line_col(content: &str, offset: usize) -> (u32, u32) {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in content.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
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
                    tokio::fs::read_to_string(&path).await.ok()?
                } else {
                    return None;
                }
            }
        }
    };

    let entries = korni::parse_with_options(&content, ParseOptions::full());

    for entry in entries {
        if let korni::Entry::Pair(kv) = entry {
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
    }

    None
}

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

fn is_valid_env_var_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let mut chars = name.chars();
    if let Some(first) = chars.next() {
        if !first.is_ascii_alphabetic() && first != '_' {
            return false;
        }
    }

    for ch in chars {
        if !ch.is_ascii_alphanumeric() && ch != '_' {
            return false;
        }
    }

    true
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
