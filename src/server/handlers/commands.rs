use crate::server::handlers::util::{format_source, resolve_env_var_value};
use crate::server::state::ServerState;
use abundantis::source::AsyncEnvSource;
use serde_json::json;
use std::time::Instant;
use tower_lsp::lsp_types::ExecuteCommandParams;

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
            let root = crate::server::util::get_workspace_root(&state.core.workspace).await;

            // Collect from both sources
            let mut env_vars: std::collections::HashSet<String> = std::collections::HashSet::new();

            // Source 1: Variables defined in .env files
            let defined_vars = crate::server::util::safe_all_for_file(&state.core, &root).await;
            for var in defined_vars {
                env_vars.insert(var.key.to_string());
            }

            // Source 2: Variables referenced in code files
            for var in state.workspace_index.all_env_vars() {
                env_vars.insert(var.to_string());
            }

            // Sort for consistent output
            let mut env_vars: Vec<String> = env_vars.into_iter().collect();
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
            use abundantis::source::VariableSource;
            let precedence = state.config.get_precedence().await;
            let root = crate::server::util::get_workspace_root(&state.core.workspace).await;

            // Get all resolved variables to count by source type
            let all_vars = crate::server::util::safe_all_for_file(&state.core, &root).await;

            // Count variables by source type
            let mut shell_count = 0usize;
            let mut file_count = 0usize;
            let mut remote_count = 0usize;

            for var in &all_vars {
                match &var.source {
                    VariableSource::Shell => shell_count += 1,
                    VariableSource::File { .. } => file_count += 1,
                    VariableSource::Remote { .. } => remote_count += 1,
                    VariableSource::Memory => {}
                }
            }

            // Get authenticated remote provider names
            let external_providers = state.core.registry.external_providers();
            let mut providers: Vec<String> = Vec::new();
            for adapter in &external_providers {
                let status = adapter.auth_status();
                if status.is_authenticated() {
                    if let Some(info) = adapter.provider_info() {
                        providers.push(info.id.to_string());
                    }
                }
            }

            let all_sources = [
                ("Shell", SourcePrecedence::Shell, 100, shell_count),
                ("File", SourcePrecedence::File, 50, file_count),
                ("Remote", SourcePrecedence::Remote, 25, remote_count),
            ];

            let sources: Vec<serde_json::Value> = all_sources
                .iter()
                .enumerate()
                .map(|(i, (name, sp, priority, count))| {
                    let mut obj = json!({
                        "name": name,
                        "enabled": precedence.contains(sp),
                        "priority": priority,
                        "count": count
                    });
                    // Add providers only for Remote source
                    if i == 2 && !providers.is_empty() {
                        obj["providers"] = json!(providers);
                    }
                    obj
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

            // Empty precedence = no sources enabled (all disabled)
            // This is a valid state - user explicitly disabled all sources

            state.config.set_precedence(new_precedence.clone()).await;

            let new_resolution_config = abundantis::config::ResolutionConfig {
                precedence: new_precedence.clone(),
                ..Default::default()
            };
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

            let new_interpolation_config = abundantis::config::InterpolationConfig {
                enabled,
                ..Default::default()
            };
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
        // Remote source commands
        "ecolog.source.remote.list" => handle_remote_list(state).await,
        "ecolog.source.remote.authFields" => {
            let provider = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str());
            handle_remote_auth_fields(state, provider).await
        }
        "ecolog.source.remote.authenticate" => {
            let provider = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str());
            let credentials = params
                .arguments
                .get(1)
                .and_then(|arg| arg.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect::<std::collections::HashMap<_, _>>()
                });
            handle_remote_authenticate(state, provider, credentials).await
        }
        "ecolog.source.remote.navigate" => {
            let provider = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str());
            let level = params
                .arguments
                .get(1)
                .and_then(|arg| arg.as_str());
            let parent_scope = params
                .arguments
                .get(2)
                .and_then(|arg| serde_json::from_value::<abundantis::source::ScopeSelection>(arg.clone()).ok());
            handle_remote_navigate(state, provider, level, parent_scope).await
        }
        "ecolog.source.remote.select" => {
            let provider = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str());
            let scope = params
                .arguments
                .get(1)
                .and_then(|arg| serde_json::from_value::<abundantis::source::ScopeSelection>(arg.clone()).ok());
            handle_remote_select(state, provider, scope).await
        }
        "ecolog.source.remote.refresh" => {
            let provider = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str());
            handle_remote_refresh(state, provider).await
        }
        // External provider commands (out-of-process providers)
        "ecolog.provider.list" => handle_provider_list(state).await,
        "ecolog.provider.spawn" => {
            let provider = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str());
            handle_provider_spawn(state, provider).await
        }
        "ecolog.provider.authFields" => {
            let provider = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str());
            handle_provider_auth_fields(state, provider).await
        }
        "ecolog.provider.authenticate" => {
            let provider = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str());
            let credentials = params
                .arguments
                .get(1)
                .and_then(|arg| arg.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect::<std::collections::HashMap<_, _>>()
                });
            handle_provider_authenticate(state, provider, credentials).await
        }
        "ecolog.provider.scopeLevels" => {
            let provider = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str());
            handle_provider_scope_levels(state, provider).await
        }
        "ecolog.provider.navigate" => {
            let provider = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str());
            let level = params
                .arguments
                .get(1)
                .and_then(|arg| arg.as_str());
            let parent_scope = params
                .arguments
                .get(2)
                .and_then(|arg| serde_json::from_value::<abundantis::source::remote::ProtocolScopeSelection>(arg.clone()).ok());
            handle_provider_navigate(state, provider, level, parent_scope).await
        }
        "ecolog.provider.select" => {
            let provider = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str());
            let scope = params
                .arguments
                .get(1)
                .and_then(|arg| serde_json::from_value::<abundantis::source::remote::ProtocolScopeSelection>(arg.clone()).ok());
            handle_provider_select(state, provider, scope).await
        }
        "ecolog.provider.refresh" => {
            let provider = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str());
            handle_provider_refresh(state, provider).await
        }
        "ecolog.provider.shutdown" => {
            let provider = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str());
            handle_provider_shutdown(state, provider).await
        }
        _ => None,
    }
}

// Remote source command handlers

/// Gets an external provider adapter by provider ID.
fn get_external_provider(
    state: &ServerState,
    provider: &str,
) -> Result<std::sync::Arc<abundantis::source::remote::ExternalProviderAdapter>, String> {
    state.core.registry.get_external_provider(provider)
        .ok_or_else(|| format!("Unknown provider: {}", provider))
}

async fn handle_remote_list(state: &ServerState) -> Option<serde_json::Value> {
    let adapters = state.core.registry.external_providers();

    let mut sources = Vec::new();
    for adapter in adapters {
        let info = match adapter.provider_info() {
            Some(info) => info,
            None => continue,
        };
        let auth_status = adapter.auth_status();
        let scope = adapter.scope();
        let cached_count = match adapter.load().await {
            Ok(snapshot) => snapshot.variables.len(),
            Err(_) => 0,
        };

        let scope_levels = match adapter.scope_levels().await {
            Ok(levels) => levels.iter().map(|l| json!({
                "name": l.name.as_str(),
                "displayName": l.display_name.as_str(),
                "required": l.required,
            })).collect::<Vec<_>>(),
            Err(_) => Vec::new(),
        };

        sources.push(json!({
            "id": info.id.as_str(),
            "displayName": info.name.as_str(),
            "shortName": info.short_name.as_ref().map(|s| s.as_str()).unwrap_or(info.id.as_str()),
            "authStatus": format!("{:?}", auth_status),
            "isAuthenticated": auth_status.is_authenticated(),
            "scope": scope.selections,
            "secretCount": cached_count,
            "scopeLevels": scope_levels,
        }));
    }

    // List registered external provider IDs
    let provider_ids = state.core.registry.external_provider_ids();

    Some(json!({
        "sources": sources,
        "count": sources.len(),
        "availableProviders": provider_ids,
    }))
}

async fn handle_remote_auth_fields(
    state: &ServerState,
    provider: Option<&str>,
) -> Option<serde_json::Value> {
    let provider = match provider {
        Some(p) => p,
        None => return Some(json!({ "error": "Provider ID required" })),
    };

    // Get the provider adapter
    let adapter = match get_external_provider(state, provider) {
        Ok(a) => a,
        Err(e) => return Some(json!({ "error": e })),
    };

    let fields = match adapter.auth_fields().await {
        Ok(f) => f,
        Err(e) => return Some(json!({ "error": format!("Failed to get auth fields: {}", e) })),
    };
    let field_json: Vec<serde_json::Value> = fields.iter().map(|f| {
        json!({
            "name": f.name.as_str(),
            "label": f.label.as_str(),
            "description": f.description.as_ref().map(|d| d.as_str()),
            "required": f.required,
            "secret": f.secret,
            "envVar": f.env_var.as_ref().map(|e| e.as_str()),
            "default": f.default.as_ref().map(|d| d.as_str()),
        })
    }).collect();

    Some(json!({
        "provider": provider,
        "fields": field_json,
    }))
}

async fn handle_remote_authenticate(
    state: &ServerState,
    provider: Option<&str>,
    credentials: Option<std::collections::HashMap<String, String>>,
) -> Option<serde_json::Value> {
    let provider = match provider {
        Some(p) => p,
        None => return Some(json!({ "error": "Provider ID required" })),
    };

    let credentials = match credentials {
        Some(c) => c,
        None => return Some(json!({ "error": "Credentials required" })),
    };

    // Get the provider adapter
    let adapter = match get_external_provider(state, provider) {
        Ok(a) => a,
        Err(e) => return Some(json!({ "error": e })),
    };

    // Authenticate
    match adapter.authenticate(credentials).await {
        Ok(status) => {
            Some(json!({
                "success": true,
                "provider": provider,
                "authStatus": format!("{:?}", status),
            }))
        }
        Err(e) => {
            Some(json!({
                "error": format!("Authentication failed: {}", e),
                "provider": provider,
            }))
        }
    }
}

async fn handle_remote_navigate(
    state: &ServerState,
    provider: Option<&str>,
    level: Option<&str>,
    parent_scope: Option<abundantis::source::ScopeSelection>,
) -> Option<serde_json::Value> {
    let provider = match provider {
        Some(p) => p,
        None => return Some(json!({ "error": "Provider ID required" })),
    };

    let level = match level {
        Some(l) => l,
        None => return Some(json!({ "error": "Scope level required" })),
    };

    let parent = parent_scope.unwrap_or_default();

    // Get the provider adapter
    let adapter = match get_external_provider(state, provider) {
        Ok(a) => a,
        Err(e) => return Some(json!({ "error": e })),
    };

    // Convert to protocol ScopeSelection type
    let parent_protocol = abundantis::source::remote::ProtocolScopeSelection {
        selections: parent.selections,
    };

    match adapter.list_scope_options(level, &parent_protocol).await {
        Ok(options) => {
            let options_json: Vec<serde_json::Value> = options.iter().map(|o| {
                json!({
                    "id": o.id.as_str(),
                    "displayName": o.display_name.as_str(),
                    "description": o.description.as_ref().map(|d| d.as_str()),
                    "icon": o.icon.as_ref().map(|i| i.as_str()),
                })
            }).collect();

            Some(json!({
                "provider": provider,
                "level": level,
                "options": options_json,
                "count": options_json.len(),
            }))
        }
        Err(e) => {
            Some(json!({
                "error": format!("Failed to list options: {}", e),
                "provider": provider,
                "level": level,
            }))
        }
    }
}

async fn handle_remote_select(
    state: &ServerState,
    provider: Option<&str>,
    scope: Option<abundantis::source::ScopeSelection>,
) -> Option<serde_json::Value> {
    let provider = match provider {
        Some(p) => p,
        None => return Some(json!({ "error": "Provider ID required" })),
    };

    let scope = match scope {
        Some(s) => s,
        None => return Some(json!({ "error": "Scope selection required" })),
    };

    // Get the provider adapter
    let adapter = match get_external_provider(state, provider) {
        Ok(a) => a,
        Err(e) => return Some(json!({ "error": e })),
    };

    // Convert to protocol ScopeSelection type and set
    let scope_protocol = abundantis::source::remote::ProtocolScopeSelection {
        selections: scope.selections,
    };
    adapter.set_scope(scope_protocol);

    // Try to load secrets with the new scope
    match adapter.load().await {
        Ok(snapshot) => {
            // Trigger refresh so other components see the new variables
            crate::server::util::safe_refresh(
                &state.core,
                abundantis::RefreshOptions::preserve_all(),
            )
            .await;

            Some(json!({
                "success": true,
                "provider": provider,
                "secretCount": snapshot.variables.len(),
            }))
        }
        Err(e) => {
            Some(json!({
                "error": format!("Failed to fetch secrets: {}", e),
                "provider": provider,
            }))
        }
    }
}

async fn handle_remote_refresh(
    state: &ServerState,
    provider: Option<&str>,
) -> Option<serde_json::Value> {
    if let Some(provider_id) = provider {
        // Refresh specific provider
        let adapter = match get_external_provider(state, provider_id) {
            Ok(a) => a,
            Err(e) => return Some(json!({ "error": e })),
        };

        adapter.invalidate_cache();

        match adapter.refresh().await {
            Ok(changed) => {
                let snapshot = adapter.load().await.ok();
                let count = snapshot.map(|s| s.variables.len()).unwrap_or(0);

                Some(json!({
                    "success": true,
                    "provider": provider_id,
                    "changed": changed,
                    "secretCount": count,
                }))
            }
            Err(e) => {
                Some(json!({
                    "error": format!("Failed to refresh: {}", e),
                    "provider": provider_id,
                }))
            }
        }
    } else {
        // Refresh all external providers
        let adapters = state.core.registry.external_providers();
        let mut results = Vec::new();

        for adapter in &adapters {
            adapter.invalidate_cache();
            let provider_id = adapter.provider_id().to_string();

            match adapter.refresh().await {
                Ok(changed) => {
                    let snapshot = adapter.load().await.ok();
                    let count = snapshot.map(|s| s.variables.len()).unwrap_or(0);
                    results.push(json!({
                        "provider": provider_id,
                        "success": true,
                        "changed": changed,
                        "secretCount": count,
                    }));
                }
                Err(e) => {
                    results.push(json!({
                        "provider": provider_id,
                        "error": format!("{}", e),
                    }));
                }
            }
        }

        // Also trigger global refresh
        crate::server::util::safe_refresh(
            &state.core,
            abundantis::RefreshOptions::preserve_all(),
        )
        .await;

        Some(json!({
            "results": results,
            "count": results.len(),
        }))
    }
}

// External provider command handlers (out-of-process providers)

/// Lists all registered external providers with their status.
async fn handle_provider_list(state: &ServerState) -> Option<serde_json::Value> {
    // Use provider_manager instead of registry for external providers
    let providers = state.provider_manager.list_with_info();

    let mut provider_list = Vec::new();
    for info in &providers {
        provider_list.push(json!({
            "id": info.id.as_str(),
            "displayName": info.display_name.as_str(),
            "shortName": info.short_name.as_str(),
            "authStatus": format!("{:?}", info.auth_status),
            "isAuthenticated": matches!(info.auth_status, abundantis::source::remote::AuthStatus::Authenticated { .. }),
            "scope": info.scope.selections,
            "secretCount": info.secret_count,
            "loading": info.loading,
            "lastError": info.last_error.as_ref().map(|e| e.as_str()),
        }));
    }

    Some(json!({
        "providers": provider_list,
        "count": provider_list.len(),
    }))
}

/// Spawns/starts an external provider.
async fn handle_provider_spawn(
    state: &ServerState,
    provider_id: Option<&str>,
) -> Option<serde_json::Value> {
    let provider_id = match provider_id {
        Some(p) => p,
        None => return Some(json!({ "error": "Provider ID required" })),
    };

    let provider = match state.provider_manager.get(provider_id) {
        Some(p) => p,
        None => return Some(json!({ "error": format!("Unknown provider: {}", provider_id) })),
    };

    match provider.spawn().await {
        Ok(()) => {
            let info = provider.info();
            Some(json!({
                "success": true,
                "provider": provider_id,
                "state": format!("{:?}", provider.state()),
                "isAuthenticated": matches!(info.auth_status, abundantis::source::remote::AuthStatus::Authenticated { .. }),
            }))
        }
        Err(e) => Some(json!({
            "error": format!("Failed to spawn provider: {}", e),
            "provider": provider_id,
        })),
    }
}

/// Gets authentication fields for an external provider.
async fn handle_provider_auth_fields(
    state: &ServerState,
    provider_id: Option<&str>,
) -> Option<serde_json::Value> {
    let provider_id = match provider_id {
        Some(p) => p,
        None => return Some(json!({ "error": "Provider ID required" })),
    };

    let provider = match state.provider_manager.get(provider_id) {
        Some(p) => p,
        None => return Some(json!({ "error": format!("Unknown provider: {}", provider_id) })),
    };

    match provider.auth_fields().await {
        Ok(fields) => {
            let field_json: Vec<serde_json::Value> = fields.iter().map(|f| {
                json!({
                    "name": f.name.as_str(),
                    "label": f.label.as_str(),
                    "description": f.description.as_ref().map(|d| d.as_str()),
                    "required": f.required,
                    "secret": f.secret,
                    "envVar": f.env_var.as_ref().map(|e| e.as_str()),
                    "default": f.default.as_ref().map(|d| d.as_str()),
                })
            }).collect();

            Some(json!({
                "provider": provider_id,
                "fields": field_json,
            }))
        }
        Err(e) => Some(json!({
            "error": format!("Failed to get auth fields: {}", e),
            "provider": provider_id,
        })),
    }
}

/// Authenticates with an external provider.
async fn handle_provider_authenticate(
    state: &ServerState,
    provider_id: Option<&str>,
    credentials: Option<std::collections::HashMap<String, String>>,
) -> Option<serde_json::Value> {
    let provider_id = match provider_id {
        Some(p) => p,
        None => return Some(json!({ "error": "Provider ID required" })),
    };

    let credentials = match credentials {
        Some(c) => c,
        None => return Some(json!({ "error": "Credentials required" })),
    };

    let provider = match state.provider_manager.get(provider_id) {
        Some(p) => p,
        None => return Some(json!({ "error": format!("Unknown provider: {}", provider_id) })),
    };

    match provider.authenticate(credentials).await {
        Ok(status) => {
            let is_authenticated = status.is_authenticated();
            Some(json!({
                "success": is_authenticated,
                "provider": provider_id,
                "authStatus": format!("{:?}", status),
                "isAuthenticated": is_authenticated,
            }))
        }
        Err(e) => Some(json!({
            "error": format!("Authentication failed: {}", e),
            "provider": provider_id,
        })),
    }
}

/// Gets scope levels for an external provider.
async fn handle_provider_scope_levels(
    state: &ServerState,
    provider_id: Option<&str>,
) -> Option<serde_json::Value> {
    let provider_id = match provider_id {
        Some(p) => p,
        None => return Some(json!({ "error": "Provider ID required" })),
    };

    let provider = match state.provider_manager.get(provider_id) {
        Some(p) => p,
        None => return Some(json!({ "error": format!("Unknown provider: {}", provider_id) })),
    };

    match provider.scope_levels().await {
        Ok(levels) => {
            let levels_json: Vec<serde_json::Value> = levels.iter().map(|l| {
                json!({
                    "name": l.name.as_str(),
                    "displayName": l.display_name.as_str(),
                    "required": l.required,
                })
            }).collect();

            Some(json!({
                "provider": provider_id,
                "levels": levels_json,
                "count": levels_json.len(),
            }))
        }
        Err(e) => Some(json!({
            "error": format!("Failed to get scope levels: {}", e),
            "provider": provider_id,
        })),
    }
}

/// Lists scope options at a level for an external provider.
async fn handle_provider_navigate(
    state: &ServerState,
    provider_id: Option<&str>,
    level: Option<&str>,
    parent_scope: Option<abundantis::source::remote::ProtocolScopeSelection>,
) -> Option<serde_json::Value> {
    let provider_id = match provider_id {
        Some(p) => p,
        None => return Some(json!({ "error": "Provider ID required" })),
    };

    let level = match level {
        Some(l) => l,
        None => return Some(json!({ "error": "Scope level required" })),
    };

    let parent = parent_scope.unwrap_or_default();

    let provider = match state.provider_manager.get(provider_id) {
        Some(p) => p,
        None => return Some(json!({ "error": format!("Unknown provider: {}", provider_id) })),
    };

    match provider.list_scope_options(level, &parent).await {
        Ok(options) => {
            let options_json: Vec<serde_json::Value> = options.iter().map(|o| {
                json!({
                    "id": o.id.as_str(),
                    "displayName": o.display_name.as_str(),
                    "description": o.description.as_ref().map(|d| d.as_str()),
                    "icon": o.icon.as_ref().map(|i| i.as_str()),
                })
            }).collect();

            Some(json!({
                "provider": provider_id,
                "level": level,
                "options": options_json,
                "count": options_json.len(),
            }))
        }
        Err(e) => Some(json!({
            "error": format!("Failed to list options: {}", e),
            "provider": provider_id,
            "level": level,
        })),
    }
}

/// Sets scope for an external provider.
async fn handle_provider_select(
    state: &ServerState,
    provider_id: Option<&str>,
    scope: Option<abundantis::source::remote::ProtocolScopeSelection>,
) -> Option<serde_json::Value> {
    let provider_id = match provider_id {
        Some(p) => p,
        None => return Some(json!({ "error": "Provider ID required" })),
    };

    let scope = match scope {
        Some(s) => s,
        None => return Some(json!({ "error": "Scope selection required" })),
    };

    let provider = match state.provider_manager.get(provider_id) {
        Some(p) => p,
        None => return Some(json!({ "error": format!("Unknown provider: {}", provider_id) })),
    };

    // Set the scope
    provider.set_scope(scope);

    // Fetch secrets with new scope
    match provider.fetch_secrets().await {
        Ok(secrets) => {
            // Register with source registry so load_all() sees the secrets
            state
                .core
                .registry
                .register_external_provider(std::sync::Arc::clone(&provider));

            // Trigger refresh so other components see the new variables
            crate::server::util::safe_refresh(
                &state.core,
                abundantis::RefreshOptions::preserve_all(),
            )
            .await;

            Some(json!({
                "success": true,
                "provider": provider_id,
                "secretCount": secrets.len(),
            }))
        }
        Err(e) => Some(json!({
            "error": format!("Failed to fetch secrets: {}", e),
            "provider": provider_id,
        })),
    }
}

/// Refreshes secrets from an external provider.
async fn handle_provider_refresh(
    state: &ServerState,
    provider_id: Option<&str>,
) -> Option<serde_json::Value> {
    if let Some(provider_id) = provider_id {
        let provider = match state.provider_manager.get(provider_id) {
            Some(p) => p,
            None => return Some(json!({ "error": format!("Unknown provider: {}", provider_id) })),
        };

        provider.invalidate_cache();

        match provider.fetch_secrets().await {
            Ok(secrets) => {
                // Re-register to ensure latest adapter state is in registry
                state
                    .core
                    .registry
                    .register_external_provider(std::sync::Arc::clone(&provider));

                crate::server::util::safe_refresh(
                    &state.core,
                    abundantis::RefreshOptions::preserve_all(),
                )
                .await;

                Some(json!({
                    "success": true,
                    "provider": provider_id,
                    "secretCount": secrets.len(),
                }))
            }
            Err(e) => Some(json!({
                "error": format!("Failed to refresh: {}", e),
                "provider": provider_id,
            })),
        }
    } else {
        // Refresh all external providers
        let providers = state.provider_manager.list();
        let mut results = Vec::new();

        for provider in &providers {
            provider.invalidate_cache();
            let id = provider.provider_id().to_string();

            match provider.fetch_secrets().await {
                Ok(secrets) => {
                    // Register each provider with the registry
                    state
                        .core
                        .registry
                        .register_external_provider(std::sync::Arc::clone(provider));

                    results.push(json!({
                        "provider": id,
                        "success": true,
                        "secretCount": secrets.len(),
                    }));
                }
                Err(e) => {
                    results.push(json!({
                        "provider": id,
                        "error": format!("{}", e),
                    }));
                }
            }
        }

        crate::server::util::safe_refresh(
            &state.core,
            abundantis::RefreshOptions::preserve_all(),
        )
        .await;

        Some(json!({
            "results": results,
            "count": results.len(),
        }))
    }
}

/// Shuts down an external provider.
async fn handle_provider_shutdown(
    state: &ServerState,
    provider_id: Option<&str>,
) -> Option<serde_json::Value> {
    let provider_id = match provider_id {
        Some(p) => p,
        None => return Some(json!({ "error": "Provider ID required" })),
    };

    let provider = match state.provider_manager.get(provider_id) {
        Some(p) => p,
        None => return Some(json!({ "error": format!("Unknown provider: {}", provider_id) })),
    };

    match provider.shutdown().await {
        Ok(()) => Some(json!({
            "success": true,
            "provider": provider_id,
        })),
        Err(e) => Some(json!({
            "error": format!("Failed to shutdown: {}", e),
            "provider": provider_id,
        })),
    }
}
