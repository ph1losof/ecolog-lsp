use crate::server::handlers::util::{format_source, resolve_env_var_value};
use crate::server::state::ServerState;
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

            // Empty precedence means all sources enabled (default behavior)
            if new_precedence.is_empty() {
                new_precedence = vec![
                    SourcePrecedence::Shell,
                    SourcePrecedence::File,
                    SourcePrecedence::Remote,
                ];
            }

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
        _ => None,
    }
}
