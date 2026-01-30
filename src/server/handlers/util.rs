use crate::server::state::ServerState;
use abundantis::source::VariableSource;
use std::path::Path;
use std::time::Instant;
use tower_lsp::lsp_types::{Position, Range, Url};

pub(crate) fn format_source(source: &VariableSource, root: &Path) -> String {
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

pub(crate) struct ResolvedEnvVarValue {
    pub value: String,
    pub source: String,
    pub description: Option<compact_str::CompactString>,
}

pub(crate) async fn resolve_env_var_value(
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

pub(crate) fn format_hover_markdown(
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

pub(crate) fn get_line_col(content: &str, offset: usize) -> (u32, u32) {
    if offset >= content.len() {
        return (0, 0);
    }

    let rope = ropey::Rope::from_str(content);
    let line_idx = rope.byte_to_line(offset);
    let line_start_byte = rope.line_to_byte(line_idx);
    let col_char = rope.byte_slice(line_start_byte..offset).len_chars();

    (line_idx as u32, col_char as u32)
}

pub(crate) fn korni_span_to_range(content: &str, span: korni::Span) -> Range {
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

pub(crate) fn offset_to_line_col(content: &str, offset: usize) -> (u32, u32) {
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

pub(crate) fn is_valid_env_var_name(name: &str) -> bool {
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

pub(crate) async fn get_identifier_at_position(
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

// Re-export KorniEntryExt from types for backwards compatibility
pub use crate::types::KorniEntryExt;

#[cfg(test)]
mod tests {
    use super::*;
    use abundantis::source::VariableSource;
    use std::path::PathBuf;

    // =========================================================================
    // format_source tests
    // =========================================================================

    #[test]
    fn test_format_source_file_relative() {
        let root = PathBuf::from("/workspace");
        let source = VariableSource::File {
            path: PathBuf::from("/workspace/config/.env"),
            offset: 0,
        };
        let result = format_source(&source, &root);
        assert_eq!(result, "config/.env");
    }

    #[test]
    fn test_format_source_file_absolute() {
        let root = PathBuf::from("/workspace");
        let source = VariableSource::File {
            path: PathBuf::from("/other/path/.env"),
            offset: 0,
        };
        let result = format_source(&source, &root);
        assert_eq!(result, "/other/path/.env");
    }

    #[test]
    fn test_format_source_shell() {
        let root = PathBuf::from("/workspace");
        let source = VariableSource::Shell;
        let result = format_source(&source, &root);
        assert_eq!(result, "System Environment");
    }

    #[test]
    fn test_format_source_memory() {
        let root = PathBuf::from("/workspace");
        let source = VariableSource::Memory;
        let result = format_source(&source, &root);
        assert_eq!(result, "In-Memory");
    }

    #[test]
    fn test_format_source_remote_with_path() {
        let root = PathBuf::from("/workspace");
        let source = VariableSource::Remote {
            provider: "aws".into(),
            path: Some("secrets/prod".to_string()),
        };
        let result = format_source(&source, &root);
        assert_eq!(result, "Remote (aws: secrets/prod)");
    }

    #[test]
    fn test_format_source_remote_without_path() {
        let root = PathBuf::from("/workspace");
        let source = VariableSource::Remote {
            provider: "vault".into(),
            path: None,
        };
        let result = format_source(&source, &root);
        assert_eq!(result, "Remote (vault)");
    }

    // =========================================================================
    // format_hover_markdown tests
    // =========================================================================

    #[test]
    fn test_format_hover_markdown_simple() {
        let resolved = ResolvedEnvVarValue {
            value: "postgres://localhost".to_string(),
            source: ".env".to_string(),
            description: None,
        };
        let result = format_hover_markdown("DATABASE_URL", None, &resolved);
        assert!(result.contains("**`DATABASE_URL`**"));
        assert!(result.contains("`postgres://localhost`"));
        assert!(result.contains("`.env`"));
    }

    #[test]
    fn test_format_hover_markdown_with_binding() {
        let resolved = ResolvedEnvVarValue {
            value: "secret".to_string(),
            source: ".env.local".to_string(),
            description: None,
        };
        let result = format_hover_markdown("API_KEY", Some("apiKey"), &resolved);
        assert!(result.contains("**`apiKey`** → **`API_KEY`**"));
    }

    #[test]
    fn test_format_hover_markdown_same_binding_name() {
        let resolved = ResolvedEnvVarValue {
            value: "8080".to_string(),
            source: ".env".to_string(),
            description: None,
        };
        // When binding name is same as env var name, no arrow
        let result = format_hover_markdown("PORT", Some("PORT"), &resolved);
        assert!(result.contains("**`PORT`**"));
        assert!(!result.contains("→"));
    }

    #[test]
    fn test_format_hover_markdown_with_description() {
        let resolved = ResolvedEnvVarValue {
            value: "true".to_string(),
            source: ".env".to_string(),
            description: Some(compact_str::CompactString::from("Enable debug mode")),
        };
        let result = format_hover_markdown("DEBUG", None, &resolved);
        assert!(result.contains("*Enable debug mode*"));
    }

    #[test]
    fn test_format_hover_markdown_multiline_value() {
        let resolved = ResolvedEnvVarValue {
            value: "line1\nline2".to_string(),
            source: ".env".to_string(),
            description: None,
        };
        let result = format_hover_markdown("MULTILINE", None, &resolved);
        // Newlines should be formatted specially
        assert!(result.contains("`line1`\n`line2`"));
    }

    // =========================================================================
    // get_line_col tests
    // =========================================================================

    #[test]
    fn test_get_line_col_first_line() {
        let content = "hello world";
        assert_eq!(get_line_col(content, 0), (0, 0));
        assert_eq!(get_line_col(content, 6), (0, 6));
    }

    #[test]
    fn test_get_line_col_multiple_lines() {
        let content = "line1\nline2\nline3";
        assert_eq!(get_line_col(content, 6), (1, 0)); // start of line2
        assert_eq!(get_line_col(content, 12), (2, 0)); // start of line3
    }

    #[test]
    fn test_get_line_col_out_of_bounds() {
        let content = "short";
        assert_eq!(get_line_col(content, 100), (0, 0));
    }

    // =========================================================================
    // offset_to_line_col tests
    // =========================================================================

    #[test]
    fn test_offset_to_line_col_single_line() {
        let content = "hello world";
        assert_eq!(offset_to_line_col(content, 0), (0, 0));
        assert_eq!(offset_to_line_col(content, 6), (0, 6));
    }

    #[test]
    fn test_offset_to_line_col_after_newline() {
        let content = "abc\ndefg";
        assert_eq!(offset_to_line_col(content, 4), (1, 0)); // 'd' after newline
        assert_eq!(offset_to_line_col(content, 6), (1, 2)); // 'f'
    }

    #[test]
    fn test_offset_to_line_col_multiple_newlines() {
        let content = "a\nb\nc";
        assert_eq!(offset_to_line_col(content, 0), (0, 0)); // 'a'
        assert_eq!(offset_to_line_col(content, 2), (1, 0)); // 'b'
        assert_eq!(offset_to_line_col(content, 4), (2, 0)); // 'c'
    }

    // =========================================================================
    // is_valid_env_var_name tests
    // =========================================================================

    #[test]
    fn test_is_valid_env_var_name_valid() {
        assert!(is_valid_env_var_name("DATABASE_URL"));
        assert!(is_valid_env_var_name("API_KEY"));
        assert!(is_valid_env_var_name("_PRIVATE"));
        assert!(is_valid_env_var_name("VAR1"));
        assert!(is_valid_env_var_name("A"));
        assert!(is_valid_env_var_name("_"));
        assert!(is_valid_env_var_name("__name__"));
    }

    #[test]
    fn test_is_valid_env_var_name_invalid() {
        assert!(!is_valid_env_var_name("")); // empty
        assert!(!is_valid_env_var_name("1VAR")); // starts with number
        assert!(!is_valid_env_var_name("VAR-NAME")); // contains hyphen
        assert!(!is_valid_env_var_name("VAR.NAME")); // contains dot
        assert!(!is_valid_env_var_name("VAR NAME")); // contains space
        assert!(!is_valid_env_var_name("VAR@NAME")); // contains special char
    }

    #[test]
    fn test_is_valid_env_var_name_unicode() {
        // Unicode letters are not valid in env var names (ASCII only)
        assert!(!is_valid_env_var_name("日本語"));
        assert!(!is_valid_env_var_name("VARäble"));
    }

    // =========================================================================
    // korni_span_to_range tests
    // =========================================================================

    #[test]
    fn test_korni_span_to_range_single_line() {
        let content = "KEY=value";
        let span = korni::Span {
            start: korni::Position { offset: 0 },
            end: korni::Position { offset: 3 },
        };
        let range = korni_span_to_range(content, span);
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.line, 0);
        assert_eq!(range.end.character, 3);
    }

    #[test]
    fn test_korni_span_to_range_multiline() {
        let content = "KEY1=value1\nKEY2=value2";
        let span = korni::Span {
            start: korni::Position { offset: 12 }, // start of KEY2
            end: korni::Position { offset: 16 },   // end of KEY2
        };
        let range = korni_span_to_range(content, span);
        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.line, 1);
        assert_eq!(range.end.character, 4);
    }
}
