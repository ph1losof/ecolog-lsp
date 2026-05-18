use crate::server::handlers::util::get_line_col;
use crate::server::state::ServerState;
use compact_str::CompactString;
use futures::future::join_all;
use korni::{Error as KorniError, ParseOptions};
use rustc_hash::{FxHashMap, FxHashSet};
use std::time::Instant;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Url};

/// Type alias for diagnostic analysis results: (direct references, env var symbols, property accesses)
type DiagnosticAnalysisResult = (
    Vec<crate::types::EnvReference>,
    Vec<(compact_str::CompactString, Range)>,
    Vec<(compact_str::CompactString, Range)>,
);

pub async fn compute_diagnostics(uri: &Url, state: &ServerState) -> Vec<Diagnostic> {
    tracing::debug!("[COMPUTE_DIAGNOSTICS_ENTER] uri={}", uri);
    let start = Instant::now();

    if !state.config.is_diagnostics_enabled() {
        tracing::debug!(
            "[COMPUTE_DIAGNOSTICS_EXIT] disabled elapsed_ms={}",
            start.elapsed().as_millis()
        );
        return vec![];
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

    let (references, env_var_symbols, property_accesses): DiagnosticAnalysisResult = {
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

    let is_env_file = state.config.is_env_file(&file_name);

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
        // Check for syntax errors in the parsed tree
        let syntax_errors = state.document_manager.get_syntax_errors(uri);
        for (range, message) in syntax_errors {
            let error_message = message.unwrap_or_else(|| "Syntax error".to_string());
            diagnostics.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String("syntax-error".to_string())),
                source: Some("ecolog".to_string()),
                message: error_message,
                ..Default::default()
            });
        }

        // Collect all unique env var names from all three sources
        let mut all_names: FxHashSet<CompactString> = FxHashSet::default();
        for reference in &references {
            all_names.insert(reference.name.clone());
        }
        for (env_name, _) in &env_var_symbols {
            all_names.insert(env_name.clone());
        }
        for (env_name, _) in &property_accesses {
            all_names.insert(env_name.clone());
        }

        // Resolve all env vars in parallel
        let names_vec: Vec<CompactString> = all_names.into_iter().collect();
        let resolution_futures = names_vec.iter().map(|name| {
            crate::server::util::safe_get_for_file(&state.core, name, &file_path)
        });
        let results = join_all(resolution_futures).await;

        let resolved_map: FxHashMap<CompactString, bool> = names_vec
            .into_iter()
            .zip(results)
            .map(|(name, result)| (name, result.is_some()))
            .collect();

        // Build diagnostics using the pre-resolved map
        for reference in references {
            if !resolved_map.get(&reference.name).copied().unwrap_or(false) {
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
            if !resolved_map.get(&env_name).copied().unwrap_or(false) {
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
            if !resolved_map.get(&env_name).copied().unwrap_or(false) {
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
