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
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, Diagnostic, DiagnosticSeverity,
    Documentation, ExecuteCommandParams, Hover, HoverContents, HoverParams, Location,
    MarkupContent, MarkupKind, NumberOrString, Position, PrepareRenameResponse, Range,
    ReferenceParams, RenameParams, TextDocumentPositionParams, TextEdit, Url, WorkspaceEdit,
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

/// Result of resolving an environment variable with masking applied.
struct ResolvedEnvVarValue {
    /// The masked or unmasked value
    value: String,
    /// Source description (file path, "System Environment", etc.)
    source: String,
    /// Optional description from the env var definition
    description: Option<compact_str::CompactString>,
}

/// Resolve an environment variable value.
/// This is the core resolution logic used by all hover handlers.
async fn resolve_env_var_value(
    env_var_name: &str,
    file_path: &Path,
    state: &ServerState,
) -> Option<ResolvedEnvVarValue> {
    let context = {
        let workspace = state.core.workspace.read();
        workspace.context_for_file(file_path)?
    };

    let registry = &state.core.registry;
    let resolved = state
        .core
        .resolution
        .resolve(env_var_name, &context, registry)
        .await
        .ok()??;

    let source_str = format_source(&resolved.source, &context.workspace_root);

    Some(ResolvedEnvVarValue {
        value: resolved.resolved_value.to_string(),
        source: source_str,
        description: resolved.description.clone(),
    })
}

/// Format hover markdown with optional arrow notation.
/// Shows `**identifier** → **env_var**` if names differ, otherwise just `**env_var**`.
fn format_hover_markdown(
    env_var_name: &str,
    identifier_name: Option<&str>,
    resolved: &ResolvedEnvVarValue,
) -> String {
    let header = match identifier_name {
        Some(id) if id != env_var_name => format!("**`{}`** → **`{}`**", id, env_var_name),
        _ => format!("**`{}`**", env_var_name),
    };

    // For multi-line values, wrap each line in backticks to preserve formatting
    // Inline code blocks don't interpret markdown, so asterisks are safe
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

    // 0. Check if hover is enabled
    {
        let config = state.config.get_config();
        let config = config.read().await;
        if !config.features.hover {
            tracing::debug!("Hover feature disabled");
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
            // Fallback: Try cross-module resolution for imported symbols
            return handle_hover_cross_module(params, state).await;
        };

    // Resolve value using unified helper
    let file_path = uri.to_file_path().ok()?;

    if let Some(resolved) = resolve_env_var_value(&env_var_name, &file_path, state).await {
        // Build markdown with binding indicator if applicable
        let markdown = if is_binding {
            let b_name = binding_name.as_deref().unwrap_or(env_var_name.as_str());
            format_hover_markdown(&env_var_name, Some(b_name), &resolved)
        } else {
            // Direct reference: show just value and source without header
            // Wrap each line in backticks - inline code doesn't interpret markdown
            let value_formatted = if resolved.value.contains('\n') {
                format!("`{}`", resolved.value.replace('\n', "`\n`"))
            } else {
                format!("`{}`", resolved.value)
            };
            let mut md = format!(
                "**Value**: {}\n\n**Source**: `{}`",
                value_formatted, resolved.source
            );
            if let Some(desc) = &resolved.description {
                md.push_str(&format!("\n\n*{}*", desc));
            }
            md
        };

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

/// Handle hover for imported symbols using cross-module resolution.
///
/// This is called when local resolution fails (no direct reference, binding, or usage).
/// It checks if the identifier at the position is an imported symbol and tries to
/// resolve it through the export chain to find if it represents an env var.
async fn handle_hover_cross_module(params: HoverParams, state: &ServerState) -> Option<Hover> {
    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    // Get document state for import context (scoped to drop MappedRef before await)
    let (import_ctx, tree, content) = {
        let doc = state.document_manager.get(uri)?;
        (doc.import_context.clone(), doc.tree.clone(), doc.content.clone())
    };

    // Get the identifier at position from the syntax tree
    let (identifier_name, identifier_range) =
        get_identifier_at_position(state, uri, position).await?;

    // Check if this identifier is an import alias
    let (module_path, original_name) = match import_ctx.aliases.get(&identifier_name) {
        Some(alias) => alias.clone(),
        None => {
            // Not a direct import alias - check if it's a property access on an imported env object
            // e.g., `env.SECRET_KEY` where `env` is imported as an env object
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

    // Only resolve relative imports (workspace-internal)
    if !module_path.starts_with("./") && !module_path.starts_with("../") {
        return None;
    }

    // Create CrossModuleResolver and try to resolve the import
    let cross_resolver = CrossModuleResolver::new(
        state.workspace_index.clone(),
        state.module_resolver.clone(),
        state.languages.clone(),
    );

    // For default imports, the original_name is the module path itself
    // e.g., import foo from "./config" -> aliases: {"foo": ("./config", "./config")}
    // For named imports, original_name is the exported name
    // e.g., import { a } from "./config" -> aliases: {"a": ("./config", "a")}
    let is_default = original_name == module_path;

    match cross_resolver.resolve_import(uri, &module_path, &original_name, is_default) {
        CrossModuleResolution::EnvVar {
            name: env_var_name,
            ..
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
            // The import resolves to an env object (e.g., process.env)
            // Only show arrow if the identifier name differs from canonical name
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

/// Handle hover on a property access where the object is an imported env object.
/// e.g., `env.SECRET_KEY` where `env` is imported as process.env
///
/// This function is language-agnostic: it delegates AST node type detection to
/// the LanguageSupport trait via `extract_property_access()`.
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

    // Get the language for this file
    let language = state.languages.get_for_uri(uri)?;

    // Convert LSP position to byte offset
    let rope = ropey::Rope::from_str(content);
    let line_start = rope.try_line_to_char(position.line as usize).ok()?;
    let char_offset = line_start + position.character as usize;
    let byte_offset = rope.try_char_to_byte(char_offset).ok()?;

    // Use language-agnostic property access extraction
    // This handles different AST node types per language:
    // - JavaScript/TypeScript: member_expression → property_identifier
    // - Python: attribute node
    // - Rust: field_expression → field_identifier
    // - Go: selector_expression
    let (object_name, _extracted_property) = language.extract_property_access(tree, content, byte_offset)?;

    // Check if the object is an imported env object
    let (module_path, original_name) = import_ctx.aliases.get(object_name.as_str())?;

    // Only resolve relative imports
    if !module_path.starts_with("./") && !module_path.starts_with("../") {
        return None;
    }

    // Create CrossModuleResolver and check if the import resolves to an env object
    let cross_resolver = CrossModuleResolver::new(
        state.workspace_index.clone(),
        state.module_resolver.clone(),
        state.languages.clone(),
    );

    let is_default = original_name == module_path;

    match cross_resolver.resolve_import(uri, module_path, original_name, is_default) {
        CrossModuleResolution::EnvObject { .. } => {
            // The import resolves to an env object!
            // The property name is the env var name
            let env_var_name = property_name.as_str();
            let file_path = uri.to_file_path().ok()?;

            let markdown = if let Some(resolved) =
                resolve_env_var_value(env_var_name, &file_path, state).await
            {
                format_hover_markdown(env_var_name, None, &resolved)
            } else {
                // Env var not found in sources, but we know it's an env var access
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

/// Check if the completion context is an imported env object.
/// e.g., `env.` where `env` is imported as process.env
async fn check_imported_env_object_completion(
    uri: &Url,
    position: Position,
    state: &ServerState,
) -> bool {
    // Get the object name from the completion context
    let obj_name = match state
        .document_manager
        .check_completion_context(uri, position)
        .await
    {
        Some(name) => name,
        None => return false,
    };

    // Get import context
    let import_ctx = match state.document_manager.get(uri) {
        Some(doc) => doc.import_context.clone(),
        None => return false,
    };

    // Check if the object is an import alias
    let (module_path, original_name) = match import_ctx.aliases.get(obj_name.as_str()) {
        Some(alias) => alias.clone(),
        None => return false,
    };

    // Only resolve relative imports
    if !module_path.starts_with("./") && !module_path.starts_with("../") {
        return false;
    }

    // Create CrossModuleResolver and check if the import resolves to an env object
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

/// Get the identifier at a position in the document.
async fn get_identifier_at_position(
    state: &ServerState,
    uri: &Url,
    position: Position,
) -> Option<(compact_str::CompactString, Range)> {
    let doc = state.document_manager.get(uri)?;
    let tree = doc.tree.as_ref()?;
    let content = &doc.content;

    // Convert LSP position to byte offset
    let rope = ropey::Rope::from_str(content);
    let line_start = rope.try_line_to_char(position.line as usize).ok()?;
    let char_offset = line_start + position.character as usize;
    let byte_offset = rope.try_char_to_byte(char_offset).ok()?;

    // Find the node at position
    let node = tree
        .root_node()
        .descendant_for_byte_range(byte_offset, byte_offset)?;

    // Check if it's an identifier
    if node.kind() == "identifier"
        || node.kind() == "property_identifier"
        || node.kind() == "shorthand_property_identifier"
    {
        let name = node.utf8_text(content.as_bytes()).ok()?;
        let range = Range::new(
            Position::new(node.start_position().row as u32, node.start_position().column as u32),
            Position::new(node.end_position().row as u32, node.end_position().column as u32),
        );
        return Some((compact_str::CompactString::from(name), range));
    }

    None
}

pub async fn handle_completion(
    params: CompletionParams,
    state: &ServerState,
) -> Option<Vec<CompletionItem>> {
    let is_strict = {
        let config_manager = state.config.get_config();
        let config = config_manager.read().await;
        if !config.features.completion {
            tracing::debug!("Completion feature disabled");
            return None;
        }
        config.strict.completion
    };

    let uri = &params.text_document_position.text_document.uri;

    // Strict Mode Check
    if is_strict {
        let position = params.text_document_position.position;
        if !state.document_manager.check_completion(uri, position).await {
            // Not in valid local context - check if it's an imported env object
            if !check_imported_env_object_completion(uri, position, state).await {
                return None;
            }
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
        Some(
            all_vars
                .into_iter()
                .map(|var| {
                    let value = var.resolved_value.to_string();
                    let source_str = format_source(&var.source, &context.workspace_root);

                    // Wrap each line in backticks - inline code doesn't interpret markdown
                    let value_formatted = if value.contains('\n') {
                        format!("`{}`", value.replace('\n', "`\n`"))
                    } else {
                        format!("`{}`", value)
                    };

                    let mut doc = format!("**Value**: {}\n\n**Source**: `{}`", value_formatted, source_str);
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
            tracing::debug!("Definition feature disabled");
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
        // Fallback: Try cross-module resolution for imported symbols
        return handle_definition_cross_module(&params, state).await;
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

/// Handle go-to-definition for imported symbols using cross-module resolution.
async fn handle_definition_cross_module(
    params: &tower_lsp::lsp_types::GotoDefinitionParams,
    state: &ServerState,
) -> Option<tower_lsp::lsp_types::GotoDefinitionResponse> {
    use tower_lsp::lsp_types::{GotoDefinitionResponse, Location};

    let uri = &params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    // Get document state for import context (scoped to drop MappedRef before await)
    let import_ctx = {
        let doc = state.document_manager.get(uri)?;
        doc.import_context.clone()
    };

    // Get the identifier at position
    let (identifier_name, _) = get_identifier_at_position(state, uri, position).await?;

    // Check if this identifier is an import alias
    let (module_path, original_name) = import_ctx.aliases.get(&identifier_name)?.clone();

    // Only resolve relative imports
    if !module_path.starts_with("./") && !module_path.starts_with("../") {
        return None;
    }

    // Create CrossModuleResolver
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
            // Option 1: Jump to the export declaration in the source file
            // Option 2: Jump to the .env file definition
            // We'll prefer the .env file definition if it exists

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
                // If the env var is defined in a file, go there
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

            // Fall back to the export declaration
            Some(GotoDefinitionResponse::Scalar(Location {
                uri: defining_file,
                range: declaration_range,
            }))
        }
        CrossModuleResolution::EnvObject { defining_file, .. } => {
            // Go to the export declaration
            // We don't have the exact range, so use a default range
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

    // 0. Check if diagnostics are enabled
    {
        let config = state.config.get_config();
        let config = config.read().await;
        if !config.features.diagnostics {
            tracing::debug!("Diagnostics feature disabled");
            return vec![];
        }
    }

    let mut diagnostics = Vec::new();

    // 1. Get document content
    let content = {
        let doc_ref = state.document_manager.get(uri);
        let Some(doc) = doc_ref else {
            tracing::debug!("Document not found for diagnostics: {}", uri);
            return vec![];
        };
        doc.content.clone()
    };

    // 2. Get references from BindingGraph (direct refs + symbols resolving to env vars + property accesses)
    let (references, env_var_symbols, property_accesses): (
        Vec<crate::types::EnvReference>,
        Vec<(compact_str::CompactString, tower_lsp::lsp_types::Range)>,
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
            // Collect property accesses on env object aliases (e.g., env.DEBUG2)
            let prop_accesses: Vec<_> = graph
                .usages()
                .iter()
                .filter_map(|usage| {
                    // Only care about usages with property access
                    let prop_name = usage.property_access.as_ref()?;
                    // Check if the symbol is an env object
                    let symbol = graph.get_symbol(usage.symbol_id)?;
                    if matches!(
                        graph.resolve_to_env(symbol.id),
                        Some(crate::types::ResolvedEnv::Object(_))
                    ) {
                        // Use property_access_range if available, otherwise fall back to usage.range
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

            // Check property accesses on env object aliases (e.g., env.DEBUG2)
            for (env_name, range) in property_accesses {
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
        "ecolog.generateEnvExample" => {
            // Generate .env.example content from all env vars used in the workspace
            // Returns buffer contents - the client decides what to do with it
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

            // Generate .env.example content
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
        "ecolog.variable.get" => {
            // Get a specific variable by name
            // Arguments: [variable_name: string]
            let var_name = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str())
                .map(|s| s.to_string());

            let Some(name) = var_name else {
                return Some(json!({ "error": "Variable name required" }));
            };

            let root = {
                let workspace = state.core.workspace.read();
                workspace.root().to_path_buf()
            };

            // Resolve the variable
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
            // List all workspaces (for monorepo support)
            // Returns the current workspace info - full monorepo detection would require
            // more integration with abundantis workspace detection
            let workspace_info = {
                let workspace = state.core.workspace.read();
                json!({
                    "name": workspace.root().file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("workspace"),
                    "path": workspace.root().display().to_string(),
                    "isActive": true
                })
            };

            Some(json!({
                "workspaces": [workspace_info],
                "count": 1
            }))
        }
        "ecolog.workspace.switch" => {
            // Switch workspace (for monorepo support)
            // Arguments: [workspace_path: string]
            // Note: Full workspace switching would require reloading the workspace context
            // For now, we just acknowledge the request
            let workspace_path = params
                .arguments
                .first()
                .and_then(|arg| arg.as_str())
                .map(|s| s.to_string());

            let Some(path) = workspace_path else {
                return Some(json!({ "error": "Workspace path required" }));
            };

            // Validate the path exists
            let path_buf = std::path::PathBuf::from(&path);
            if !path_buf.exists() {
                return Some(json!({ "error": format!("Workspace path does not exist: {}", path) }));
            }

            // TODO: Actually switch workspace context in abundantis
            // For now, just return success as a placeholder
            Some(json!({
                "success": true,
                "workspace": path,
                "message": "Workspace switch acknowledged (full implementation pending)"
            }))
        }
        _ => None,
    }
}

// ============================================================================
// Find References Handler
// ============================================================================

/// Handle textDocument/references request.
///
/// Finds all references to the environment variable at the given position
/// across the entire workspace.
pub async fn handle_references(
    params: ReferenceParams,
    state: &ServerState,
) -> Option<Vec<Location>> {
    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let include_declaration = params.context.include_declaration;

    // Step 1: Get env var name at position (now includes cross-module resolution)
    let env_var_name = get_env_var_at_position(state, uri, position).await?;

    // Step 2: Query workspace index for files that reference this env var
    let files = state.workspace_index.files_for_env_var(&env_var_name);

    let mut locations = Vec::new();

    // Step 3: For each file, collect usages
    for file_uri in &files {
        let usages = get_env_var_usages_in_file(state, file_uri, &env_var_name).await;
        for usage in usages {
            // Skip binding usages - they're usages of the local binding name,
            // not actual references to the env var name. Including them would be
            // confusing for rename preview (since they won't be renamed).
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

    // Step 4: Include .env definition if requested
    if include_declaration {
        if let Some(def_location) = find_env_definition(state, &env_var_name).await {
            // Only add if not already included (might already be in indexed files)
            if !locations.iter().any(|loc| loc == &def_location) {
                locations.push(def_location);
            }
        }
    }

    if locations.is_empty() {
        None
    } else {
        Some(locations)
    }
}

/// Get the environment variable name at a position in a document.
async fn get_env_var_at_position(
    state: &ServerState,
    uri: &Url,
    position: Position,
) -> Option<String> {
    // Try direct reference first
    if let Some(reference) = state.document_manager.get_env_reference_cloned(uri, position) {
        return Some(reference.name.to_string());
    }

    // Try binding
    if let Some(binding) = state.document_manager.get_env_binding_cloned(uri, position) {
        return Some(binding.env_var_name.to_string());
    }

    // Try binding usage
    if let Some(usage) = state.document_manager.get_binding_usage_cloned(uri, position) {
        return Some(usage.env_var_name.to_string());
    }

    // Fallback: Try cross-module resolution for imported symbols
    if let Some(env_var_name) =
        get_env_var_from_cross_module(state, uri, position).await
    {
        return Some(env_var_name);
    }

    None
}

/// Try to resolve an imported symbol to an env var via cross-module resolution.
async fn get_env_var_from_cross_module(
    state: &ServerState,
    uri: &Url,
    position: Position,
) -> Option<String> {
    // Get document state for import context (scoped to drop MappedRef before await)
    let import_ctx = {
        let doc = state.document_manager.get(uri)?;
        doc.import_context.clone()
    };

    // Get the identifier at position
    let (identifier_name, _) = get_identifier_at_position(state, uri, position).await?;

    // Check if this identifier is an import alias
    let (module_path, original_name) = import_ctx.aliases.get(&identifier_name)?.clone();

    // Only resolve relative imports
    if !module_path.starts_with("./") && !module_path.starts_with("../") {
        return None;
    }

    // Create CrossModuleResolver
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

/// Get all usages of an env var in a file.
///
/// If the file is open in DocumentManager, uses the existing BindingGraph.
/// Otherwise, parses the file on demand.
async fn get_env_var_usages_in_file(
    state: &ServerState,
    uri: &Url,
    env_var_name: &str,
) -> Vec<crate::analysis::resolver::EnvVarUsageLocation> {
    // Try to get existing binding graph from DocumentManager
    if let Some(graph_ref) = state.document_manager.get_binding_graph(uri) {
        let resolver = BindingResolver::new(&*graph_ref);
        return resolver.find_env_var_usages(env_var_name);
    }

    // Parse file on demand
    if let Some(graph) = parse_file_for_binding_graph(state, uri).await {
        let resolver = BindingResolver::new(&graph);
        return resolver.find_env_var_usages(env_var_name);
    }

    Vec::new()
}

/// Parse a file and return its BindingGraph.
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

/// Find the definition location of an env var in .env files.
async fn find_env_definition(state: &ServerState, env_var_name: &str) -> Option<Location> {
    // Get workspace root
    let workspace_root = {
        let workspace = state.core.workspace.read();
        workspace.root().to_path_buf()
    };

    // Get env file patterns from config
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

    // Search for definition in env files
    for pattern in env_patterns {
        let env_path = workspace_root.join(&pattern);
        if env_path.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&env_path).await {
                let entries = korni::parse_with_options(&content, ParseOptions::full());

                for entry in entries {
                    if let korni::Entry::Pair(kv) = entry {
                        if kv.key.as_ref() == env_var_name {
                            if let Some(key_span) = kv.key_span {
                                // Convert korni span to LSP range
                                let range = korni_span_to_range(&content, key_span);
                                let uri =
                                    Url::from_file_path(&env_path).ok()?;
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

/// Convert a korni Span to an LSP Range.
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

/// Convert a byte offset to line/column.
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

// ============================================================================
// Rename Handlers
// ============================================================================

/// Check if a URI points to an env file based on config patterns.
async fn is_env_file_uri(state: &ServerState, uri: &Url) -> bool {
    let file_name = match uri.to_file_path().ok().and_then(|p| {
        p.file_name()
            .map(|s| s.to_string_lossy().to_string())
    }) {
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

/// Get the environment variable name and range at a position in a .env file.
async fn get_env_var_in_env_file(
    state: &ServerState,
    uri: &Url,
    position: Position,
) -> Option<(String, Range)> {
    // Get content from document manager or read from disk
    // Note: We scope the document access to drop MappedRef before any potential await
    let content = {
        let doc_content = state.document_manager.get(uri).map(|doc| doc.content.clone());
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

    // Parse the env file
    let entries = korni::parse_with_options(&content, ParseOptions::full());

    for entry in entries {
        if let korni::Entry::Pair(kv) = entry {
            if let Some(key_span) = kv.key_span {
                let range = korni_span_to_range(&content, key_span);

                // Check if position is within this key's range
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

/// Handle textDocument/prepareRename request.
///
/// Validates that rename is possible at the position and returns the range
/// of text that will be renamed. Works for both code files and .env files.
pub async fn handle_prepare_rename(
    params: TextDocumentPositionParams,
    state: &ServerState,
) -> Option<PrepareRenameResponse> {
    let uri = &params.text_document.uri;
    let position = params.position;

    // Check if this is a .env file first
    if is_env_file_uri(state, uri).await {
        let (env_var_name, range) = get_env_var_in_env_file(state, uri, position).await?;
        if !is_valid_env_var_name(&env_var_name) {
            return None;
        }
        return Some(PrepareRenameResponse::Range(range));
    }

    // Get env var name and range at position (code files)
    let (env_var_name, range) = get_env_var_with_range(state, uri, position)?;

    // Validate it's a reasonable env var name
    if !is_valid_env_var_name(&env_var_name) {
        return None;
    }

    Some(PrepareRenameResponse::Range(range))
}

/// Handle textDocument/rename request.
///
/// Renames the environment variable at the given position across the entire
/// workspace, including .env file definitions and all code references.
/// Works for both code files and .env files.
pub async fn handle_rename(params: RenameParams, state: &ServerState) -> Option<WorkspaceEdit> {
    let uri = &params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;
    let new_name = &params.new_name;

    // Validate new name
    if !is_valid_env_var_name(new_name) {
        return None;
    }

    // Check if we're renaming from a .env file
    let is_source_env_file = is_env_file_uri(state, uri).await;

    // Get old env var name and its range if in .env file
    let (old_name, source_range) = if is_source_env_file {
        let (name, range) = get_env_var_in_env_file(state, uri, position).await?;
        (name, Some(range))
    } else {
        (get_env_var_at_position(state, uri, position).await?, None)
    };

    // Collect all edits
    let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();

    // Step 1: If source is .env file, include it directly (avoids path canonicalization issues)
    if let Some(range) = source_range {
        changes.entry(uri.clone()).or_default().push(TextEdit {
            range,
            new_text: new_name.to_string(),
        });
    }

    // Step 2: Collect all code references across workspace
    let files = state.workspace_index.files_for_env_var(&old_name);
    for file_uri in &files {
        let edits = collect_rename_edits(state, file_uri, &old_name, new_name).await;
        if !edits.is_empty() {
            changes.insert(file_uri.clone(), edits);
        }
    }

    // Step 3: Rename in .env file(s) if the var is project-defined
    // For system/shell variables (e.g., PATH, HOME), this will return None,
    // so only code references are renamed - we never modify system environment.
    if let Some(def_location) = find_env_definition(state, &old_name).await {
        // Only add if not already in changes (avoid duplicate edits due to path differences)
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
        None
    } else {
        Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        })
    }
}

/// Get the environment variable name and its range at a position.
fn get_env_var_with_range(
    state: &ServerState,
    uri: &Url,
    position: Position,
) -> Option<(String, Range)> {
    // Try direct reference first
    if let Some(reference) = state.document_manager.get_env_reference_cloned(uri, position) {
        return Some((reference.name.to_string(), reference.name_range));
    }

    // Try binding - for destructured bindings, use the key range if available
    if let Some(binding) = state.document_manager.get_env_binding_cloned(uri, position) {
        // For destructured bindings like `{ API_KEY: apiKey }`, use the key range
        let range = binding
            .destructured_key_range
            .unwrap_or(binding.binding_range);
        return Some((binding.env_var_name.to_string(), range));
    }

    // Try binding usage - the usage refers to a binding, not the env var directly
    // We can't rename the env var from here, but return info for validation
    if let Some(usage) = state.document_manager.get_binding_usage_cloned(uri, position) {
        return Some((usage.env_var_name.to_string(), usage.range));
    }

    None
}

/// Collect rename edits for a file.
async fn collect_rename_edits(
    state: &ServerState,
    uri: &Url,
    old_name: &str,
    new_name: &str,
) -> Vec<TextEdit> {
    let mut edits = Vec::new();

    let usages = get_env_var_usages_in_file(state, uri, old_name).await;

    for usage in usages {
        // Determine the edit based on usage kind
        let edit = match usage.kind {
            crate::analysis::resolver::UsageKind::DirectReference
            | crate::analysis::resolver::UsageKind::PropertyAccess
            | crate::analysis::resolver::UsageKind::BindingDeclaration => {
                // DirectReference: process.env.VAR -> process.env.NEW_NAME
                // PropertyAccess: env.VAR -> env.NEW_NAME (via alias)
                // BindingDeclaration: for destructuring patterns where the env var key appears
                //   - { VAR } = process.env -> { NEW_NAME } = process.env
                //   - { VAR: v } = process.env -> { NEW_NAME: v } = process.env
                TextEdit {
                    range: usage.range,
                    new_text: new_name.to_string(),
                }
            }
            crate::analysis::resolver::UsageKind::BindingUsage => {
                // Binding usages refer to the local binding name, not the env var
                // e.g., 'a' in 'console.log(a)' where 'const a = process.env.VAR'
                // We don't rename these - they keep their local binding name
                continue;
            }
        };

        edits.push(edit);
    }

    edits
}

/// Check if a string is a valid environment variable name.
fn is_valid_env_var_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    // First char must be letter or underscore
    let mut chars = name.chars();
    if let Some(first) = chars.next() {
        if !first.is_ascii_alphabetic() && first != '_' {
            return false;
        }
    }

    // Rest must be alphanumeric or underscore
    for ch in chars {
        if !ch.is_ascii_alphanumeric() && ch != '_' {
            return false;
        }
    }

    true
}
