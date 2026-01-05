use crate::server::semantic_tokens::SemanticTokenProvider;
use crate::server::state::ServerState;
use abundantis::source::VariableSource;
use korni::{Error as KorniError, ParseOptions};
use serde_json::json;
use std::path::Path;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, Diagnostic, DiagnosticSeverity,
    Documentation, ExecuteCommandParams, Hover, HoverContents, HoverParams, MarkupContent,
    MarkupKind, NumberOrString, SemanticTokens, SemanticTokensParams, SemanticTokensResult,
};

fn format_source(source: &VariableSource, root: &Path) -> String {
    match source {
        VariableSource::File { path, .. } => {
            // Try to make relative to workspace root
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

pub async fn handle_hover(params: HoverParams, state: &ServerState) -> Option<Hover> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    // 0. Check if hover is enabled
    {
        let config = state.config.get_config();
        let config = config.read().await;
        if !config.features.hover {
            return None;
        }
    }

    // Determine which environment variable to resolve:
    // 1. First try the new BindingGraph-based resolution (handles chains)
    // 2. Fall back to legacy methods if needed

    // For bindings, we also want to know the kind (Value vs Object)
    let (env_var_name, binding_name, hover_range, is_binding, binding_kind) =
        if let Some(reference) = state
            .document_manager
            .get_env_reference_cloned(uri, position)
        {
            (reference.name, None, reference.full_range, false, None)
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
            return None;
        };

    // Resolve value using Abundantis
    let file_path = uri.to_file_path().ok()?;

    let context = {
        let workspace = state.core.workspace.read();
        workspace.context_for_file(&file_path)?
    };

    let registry = &state.core.registry;
    let resolved_result = state
        .core
        .resolution
        .resolve(&env_var_name, &context, registry)
        .await;

    if let Ok(Some(variable)) = resolved_result {
        let should_mask = {
            let config_manager = state.config.get_config();
            let config = config_manager.read().await;
            config.masking.should_mask_hover()
        };

        let value = if should_mask {
            let mut masker = state.masker.lock().await;
            let source = variable
                .source
                .file_path()
                .and_then(|p| p.strip_prefix(&context.workspace_root).ok())
                .and_then(|p| p.to_str());
            let key = Some(variable.key.as_str());
            masker.mask(&variable.resolved_value, source, key)
        } else {
            variable.resolved_value.to_string()
        };

        let source_str = format_source(&variable.source, &context.workspace_root);

        // Build markdown with binding indicator if applicable
        let mut markdown = if is_binding {
            let b_name = binding_name.as_deref().unwrap_or(env_var_name.as_str());
            if b_name == env_var_name {
                format!(
                    "**`{}`**\n\n**Value**: `{}`\n**Source**: `{}`",
                    env_var_name, value, source_str
                )
            } else {
                format!(
                    "**`{}`** → **`{}`**\n\n**Value**: `{}`\n**Source**: `{}`",
                    b_name, env_var_name, value, source_str
                )
            }
        } else {
            format!("**Value**: `{}`\n**Source**: `{}`", value, source_str)
        };

        if let Some(desc) = &variable.description {
            markdown.push_str(&format!("\n\n*{}*", desc));
        }

        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown,
            }),
            range: Some(hover_range),
        });
    } else {
        // Not found or error

        // Check for Object Alias kind
        if let Some(crate::types::BindingKind::Object) = binding_kind {
            let b_name = binding_name.as_deref().unwrap_or(env_var_name.as_str());
            // For object alias binding (e.g. const test = process.env), we want to show it aliases the env object.
            let msg = format!(
                "**`{}`** → **`{}`**\n\n*Environment Object*",
                b_name, env_var_name
            );

            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: msg,
                }),
                range: Some(hover_range),
            });
        }

        // Don't show hover for undefined vars - the diagnostic warning is sufficient
        return None;
    }
}

pub async fn handle_completion(
    params: CompletionParams,
    state: &ServerState,
) -> Option<Vec<CompletionItem>> {
    let (is_strict, should_mask) = {
        let config_manager = state.config.get_config();
        let config = config_manager.read().await;
        if !config.features.completion {
            return None;
        }
        (
            config.strict.completion,
            config.masking.should_mask_completion(),
        )
    };

    let uri = &params.text_document_position.text_document.uri;

    // Strict Mode Check
    if is_strict {
        let position = params.text_document_position.position;
        if !state.document_manager.check_completion(uri, position).await {
            // Not in valid context (e.g. process.env.|)
            return None;
        }
    }

    let file_path = uri.to_file_path().ok()?;

    let context = {
        let workspace = state.core.workspace.read();
        workspace.context_for_file(&file_path)?
    };

    let registry = &state.core.registry;

    if let Ok(all_vars) = state
        .core
        .resolution
        .all_variables(&context, registry)
        .await
    {
        // Using pre-fetched config value

        let mut masker = state.masker.lock().await;

        Some(
            all_vars
                .into_iter()
                .map(|var| {
                    let value = if should_mask {
                        let source = var
                            .source
                            .file_path()
                            .and_then(|p| p.strip_prefix(&context.workspace_root).ok())
                            .and_then(|p| p.to_str());
                        let key = Some(var.key.as_str());
                        masker.mask(&var.resolved_value, source, key)
                    } else {
                        var.resolved_value.to_string()
                    };

                    let source_str = format_source(&var.source, &context.workspace_root);

                    let mut doc = format!("**Value**: `{}`\n**Source**: `{}`", value, source_str);
                    if let Some(desc) = &var.description {
                        doc.push_str(&format!("\n\n*{}*", desc));
                    }

                    CompletionItem {
                        label: var.key.to_string(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: None, // Removed "Sensitive Variable" as requested
                        documentation: Some(Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: doc,
                        })),
                        ..Default::default()
                    }
                })
                .collect(),
        )
    } else {
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

    // 0. Check if definition is enabled
    {
        let config = state.config.get_config();
        let config = config.read().await;
        if !config.features.definition {
            return None;
        }
    }

    // Determine which environment variable to look up using BindingGraph
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
        return None;
    };

    // Resolve variable
    let file_path = uri.to_file_path().ok()?;
    let context = {
        let workspace = state.core.workspace.read();
        workspace.context_for_file(&file_path)?
    };

    let registry = &state.core.registry;
    if let Ok(Some(variable)) = state
        .core
        .resolution
        .resolve(&env_var_name, &context, registry)
        .await
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

                Some(GotoDefinitionResponse::Scalar(Location {
                    uri: target_uri,
                    range,
                }))
            }
            _ => None,
        }
    } else {
        None
    }
}

pub async fn handle_semantic_tokens_full(
    params: SemanticTokensParams,
    state: &ServerState,
) -> Option<SemanticTokensResult> {
    let uri = &params.text_document.uri;

    let file_name = uri
        .to_file_path()
        .ok()?
        .file_name()?
        .to_string_lossy()
        .to_string();

    // Fast check: must be an env file pattern
    let is_env_file = {
        let config = state.config.get_config();
        let config = config.read().await;
        config.workspace.env_files.iter().any(|pattern| {
            glob::Pattern::new(pattern)
                .map(|p| p.matches(&file_name))
                .unwrap_or(false)
        })
    };

    if !is_env_file {
        return None;
    }

    let doc_ref = state.document_manager.get(uri)?;
    let content = doc_ref.content.clone();
    drop(doc_ref);

    let entries = korni::parse_with_options(&content, ParseOptions::full());

    let rope = ropey::Rope::from_str(&content);
    let tokens = SemanticTokenProvider::extract_tokens(&rope, &content, &entries);

    Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data: tokens,
    }))
}

pub async fn compute_diagnostics(
    uri: &tower_lsp::lsp_types::Url,
    state: &ServerState,
) -> Vec<Diagnostic> {
    use tower_lsp::lsp_types::{Position, Range};

    // 0. Check if diagnostics are enabled
    {
        let config = state.config.get_config();
        let config = config.read().await;
        if !config.features.diagnostics {
            return vec![];
        }
    }

    let mut diagnostics = Vec::new();

    // 1. Get document content
    let content = {
        let doc_ref = state.document_manager.get(uri);
        let Some(doc) = doc_ref else {
            return vec![];
        };
        doc.content.clone()
    };

    // 2. Get references from BindingGraph (direct refs + symbols resolving to env vars)
    let (references, env_var_symbols): (
        Vec<crate::types::EnvReference>,
        Vec<(compact_str::CompactString, tower_lsp::lsp_types::Range)>,
    ) = {
        if let Some(graph) = state.document_manager.get_binding_graph(uri) {
            let refs = graph.direct_references().to_vec();
            // Also collect symbols that resolve to specific env vars (e.g., destructured patterns)
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
            (refs, symbols)
        } else {
            (vec![], vec![])
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

    // Part A: Linter Diagnostics (Only for .env files)
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
                // Convert offset to Range. Simple line calculation or use korni Span if available on Error?
                // Error has offset. We need line/col.
                // korni::Position::from_offset is (0,0,offset). We need to calculate line/col from content + offset.
                // Simple helper:
                let (line, col) = get_line_col(&content, offset);

                let range = Range {
                    start: Position::new(line, col),
                    end: Position::new(line, col + 1), // 1 char width for now
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

    // Part B: Undefined Variable Diagnostics (For code files)
    // Only if NOT .env file (references usually are in code)
    if !is_env_file {
        let context_opt = {
            let workspace = state.core.workspace.read();
            workspace.context_for_file(&file_path)
        };

        if let Some(context) = context_opt {
            let registry = &state.core.registry;

            // Check direct references
            for reference in references {
                let resolved_result = state
                    .core
                    .resolution
                    .resolve(&reference.name, &context, registry)
                    .await;

                if let Ok(None) = resolved_result {
                    diagnostics.push(Diagnostic {
                        range: reference.name_range,
                        severity: Some(DiagnosticSeverity::WARNING),
                        code: Some(NumberOrString::String("undefined-env-var".to_string())),
                        source: Some("ecolog".to_string()),
                        message: format!(
                            "Environment variable '{}' is not defined.",
                            reference.name
                        ),
                        ..Default::default()
                    });
                }
            }

            // Check symbols that resolve to env vars (e.g., destructured patterns)
            for (env_name, range) in env_var_symbols {
                let resolved_result = state
                    .core
                    .resolution
                    .resolve(&env_name, &context, registry)
                    .await;

                if let Ok(None) = resolved_result {
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
    }

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

/// Handle workspace/executeCommand requests
pub async fn handle_execute_command(
    params: ExecuteCommandParams,
    state: &ServerState,
) -> Option<serde_json::Value> {
    match params.command.as_str() {
        "ecolog.file.setActive" => {
            // Check if file source is enabled
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

            // Arguments: file patterns as strings
            let patterns: Vec<String> = params
                .arguments
                .into_iter()
                .filter_map(|arg| arg.as_str().map(|s| s.to_string()))
                .collect();

            if patterns.is_empty() {
                // Clear active files filter
                state.core.clear_active_files();
                Some(json!({ "success": true, "message": "Cleared active file filter" }))
            } else {
                state.core.set_active_files(&patterns);
                Some(json!({ "success": true, "patterns": patterns }))
            }
        }
        "ecolog.listEnvVariables" => {
            // Get all variables for the current workspace
            let root = {
                let workspace = state.core.workspace.read();
                workspace.root().to_path_buf()
            };

            match state.core.all_for_file(&root).await {
                Ok(vars) => {
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
                Err(e) => Some(json!({ "error": format!("Failed to list variables: {}", e) })),
            }
        }
        "ecolog.file.list" => {
            // Check if file source is enabled
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

            // List available .env files in the workspace
            let root = {
                let workspace = state.core.workspace.read();
                workspace.root().to_path_buf()
            };

            // Use configured env file patterns to discover files
            let env_files_patterns = {
                let config = state.config.get_config();
                let config = config.read().await;
                config.workspace.env_files.clone()
            };

            let mut env_files: Vec<String> = Vec::new();

            // Walk directory looking for matching files (limited depth?)
            // Abundantis has discover_file_sources logic, but that is private/internal to build?
            // We can replicate simple discovery here or just walkdir.
            // Using walkdir to match abundantis behavior roughly. Or just list root files if simple.
            // User request implies full discovery?
            // Let's stick to root level for now to avoid deep scans in command handler, unless needed.
            // Previous impl just read_dir. Let's keep reading dir but use patterns.

            if let Ok(entries) = std::fs::read_dir(&root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            let matches = env_files_patterns.iter().any(|pattern| {
                                glob::Pattern::new(pattern)
                                    .map(|p| p.matches(name))
                                    .unwrap_or(false)
                            });

                            if matches {
                                env_files.push(name.to_string());
                            }
                        }
                    }
                }
            }

            Some(json!({ "files": env_files, "count": env_files.len() }))
        }
        _ => None,
    }
}
