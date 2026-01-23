use crate::analysis::binding_graph::EnvVarLocationKind;
use crate::analysis::BindingResolver;
use crate::server::config::InlayHintConfig;
use crate::server::handlers::util::resolve_env_var_value;
use crate::server::state::ServerState;
use compact_str::CompactString;
use rustc_hash::FxHashMap;
use std::time::Instant;
use tower_lsp::lsp_types::{
    InlayHint, InlayHintKind, InlayHintLabel, InlayHintParams, InlayHintTooltip, Range,
};

pub async fn handle_inlay_hints(
    params: InlayHintParams,
    state: &ServerState,
) -> Option<Vec<InlayHint>> {
    let uri = &params.text_document.uri;
    let range = params.range;
    let start = Instant::now();
    tracing::debug!(
        "[HANDLE_INLAY_HINTS_ENTER] uri={} range={}:{}-{}:{}",
        uri,
        range.start.line,
        range.start.character,
        range.end.line,
        range.end.character
    );

    // 1. Feature flag + config
    let config = {
        let config_arc = state.config.get_config();
        let config = config_arc.read().await;
        if !config.features.inlay_hints {
            tracing::debug!("[HANDLE_INLAY_HINTS_EXIT] feature disabled");
            return None;
        }
        config.inlay_hints.clone()
    };

    // 2. Get binding graph
    let graph = state.document_manager.get_binding_graph(uri)?;
    let resolver = BindingResolver::new(&graph);

    // 3. Get all env vars
    let env_vars = resolver.all_env_vars();
    if env_vars.is_empty() {
        tracing::debug!(
            "[HANDLE_INLAY_HINTS_EXIT] no env vars, elapsed_ms={}",
            start.elapsed().as_millis()
        );
        return Some(vec![]);
    }

    // 4. Resolve values (batch)
    let file_path = uri.to_file_path().ok()?;
    let mut resolved: FxHashMap<CompactString, (String, String)> = FxHashMap::default();
    for var in &env_vars {
        if let Some(r) = resolve_env_var_value(var, &file_path, state).await {
            let display = format_value(&r.value, &config);
            resolved.insert(var.clone(), (display, r.source));
        }
    }

    // 5. Build hints
    let mut hints = Vec::new();
    let mut per_line: FxHashMap<u32, usize> = FxHashMap::default();

    for var in &env_vars {
        let Some((display, source)) = resolved.get(var) else {
            continue;
        };
        let Some(locations) = graph.get_env_var_locations(var) else {
            continue;
        };

        for loc in locations {
            if !overlaps(loc.range, range) {
                continue;
            }
            if !should_show(loc.kind, &config) {
                continue;
            }

            let line = loc.range.end.line;
            if config.max_hints_per_line > 0 {
                let count = per_line.entry(line).or_insert(0);
                if *count >= config.max_hints_per_line {
                    continue;
                }
                *count += 1;
            }

            hints.push(InlayHint {
                position: loc.range.end,
                label: InlayHintLabel::String(format!(": \"{}\"", display)),
                kind: Some(InlayHintKind::TYPE),
                text_edits: None,
                tooltip: Some(InlayHintTooltip::String(format!("Source: {}", source))),
                padding_left: Some(false),
                padding_right: Some(true),
                data: None,
            });
        }
    }

    tracing::debug!(
        "[HANDLE_INLAY_HINTS_EXIT] count={} elapsed_ms={}",
        hints.len(),
        start.elapsed().as_millis()
    );
    Some(hints)
}

fn format_value(value: &str, config: &InlayHintConfig) -> String {
    let value = if value.contains('\n') {
        format!("{}...", value.lines().next().unwrap_or(""))
    } else {
        value.to_string()
    };

    if value.len() > config.max_value_length {
        format!("{}...", &value[..config.max_value_length])
    } else {
        value
    }
}

fn should_show(kind: EnvVarLocationKind, config: &InlayHintConfig) -> bool {
    match kind {
        EnvVarLocationKind::DirectReference => config.direct_references,
        EnvVarLocationKind::BindingDeclaration => config.binding_declarations,
        EnvVarLocationKind::BindingUsage => config.binding_usages,
        EnvVarLocationKind::PropertyAccess => config.property_accesses,
    }
}

fn overlaps(inner: Range, outer: Range) -> bool {
    !(inner.end.line < outer.start.line
        || (inner.end.line == outer.start.line && inner.end.character <= outer.start.character)
        || inner.start.line > outer.end.line
        || (inner.start.line == outer.end.line && inner.start.character >= outer.end.character))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::Position;

    fn make_range(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Range {
        Range::new(
            Position::new(start_line, start_char),
            Position::new(end_line, end_char),
        )
    }

    #[test]
    fn test_format_value_simple() {
        let config = InlayHintConfig::default();
        let result = format_value("simple_value", &config);
        assert_eq!(result, "simple_value");
    }

    #[test]
    fn test_format_value_truncation() {
        let config = InlayHintConfig {
            max_value_length: 10,
            ..Default::default()
        };
        let result = format_value("this_is_a_very_long_value", &config);
        assert_eq!(result, "this_is_a_...");
    }

    #[test]
    fn test_format_value_multiline() {
        let config = InlayHintConfig::default();
        let result = format_value("line1\nline2\nline3", &config);
        assert_eq!(result, "line1...");
    }

    #[test]
    fn test_should_show_direct_reference() {
        let config = InlayHintConfig {
            direct_references: true,
            binding_declarations: false,
            binding_usages: false,
            property_accesses: false,
            ..Default::default()
        };
        assert!(should_show(EnvVarLocationKind::DirectReference, &config));
        assert!(!should_show(EnvVarLocationKind::BindingDeclaration, &config));
        assert!(!should_show(EnvVarLocationKind::BindingUsage, &config));
        assert!(!should_show(EnvVarLocationKind::PropertyAccess, &config));
    }

    #[test]
    fn test_should_show_binding_declaration() {
        let config = InlayHintConfig {
            direct_references: false,
            binding_declarations: true,
            binding_usages: false,
            property_accesses: false,
            ..Default::default()
        };
        assert!(!should_show(EnvVarLocationKind::DirectReference, &config));
        assert!(should_show(EnvVarLocationKind::BindingDeclaration, &config));
    }

    #[test]
    fn test_should_show_property_access() {
        let config = InlayHintConfig {
            direct_references: false,
            binding_declarations: false,
            binding_usages: false,
            property_accesses: true,
            ..Default::default()
        };
        assert!(should_show(EnvVarLocationKind::PropertyAccess, &config));
    }

    #[test]
    fn test_overlaps_fully_contained() {
        let outer = make_range(0, 0, 100, 0);
        let inner = make_range(10, 5, 20, 10);
        assert!(overlaps(inner, outer));
    }

    #[test]
    fn test_overlaps_partial_start() {
        let outer = make_range(10, 0, 20, 0);
        let inner = make_range(5, 0, 15, 0);
        assert!(overlaps(inner, outer));
    }

    #[test]
    fn test_overlaps_partial_end() {
        let outer = make_range(10, 0, 20, 0);
        let inner = make_range(15, 0, 25, 0);
        assert!(overlaps(inner, outer));
    }

    #[test]
    fn test_overlaps_no_overlap_before() {
        let outer = make_range(10, 0, 20, 0);
        let inner = make_range(0, 0, 5, 0);
        assert!(!overlaps(inner, outer));
    }

    #[test]
    fn test_overlaps_no_overlap_after() {
        let outer = make_range(10, 0, 20, 0);
        let inner = make_range(25, 0, 30, 0);
        assert!(!overlaps(inner, outer));
    }

    #[test]
    fn test_overlaps_same_line_no_overlap() {
        let outer = make_range(5, 10, 5, 20);
        let inner = make_range(5, 25, 5, 30);
        assert!(!overlaps(inner, outer));
    }

    #[test]
    fn test_overlaps_same_line_overlap() {
        let outer = make_range(5, 10, 5, 25);
        let inner = make_range(5, 20, 5, 30);
        assert!(overlaps(inner, outer));
    }

    #[test]
    fn test_overlaps_exact_boundary() {
        let outer = make_range(5, 10, 5, 20);
        let inner = make_range(5, 20, 5, 25);
        // inner starts exactly where outer ends - no overlap
        assert!(!overlaps(inner, outer));
    }
}
