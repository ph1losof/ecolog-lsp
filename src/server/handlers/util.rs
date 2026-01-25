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

/// Extension methods for korni `Entry` filtering.
///
/// This trait provides a unified way to filter `.env` file entries,
/// skipping comments and extracting only valid key-value pairs.
pub trait KorniEntryExt<'a> {
    /// Returns the key-value pair if this entry is a non-comment pair.
    fn as_valid_pair(self) -> Option<Box<korni::KeyValuePair<'a>>>;
}

impl<'a> KorniEntryExt<'a> for korni::Entry<'a> {
    fn as_valid_pair(self) -> Option<Box<korni::KeyValuePair<'a>>> {
        match self {
            korni::Entry::Pair(kv) if !kv.is_comment => Some(kv),
            _ => None,
        }
    }
}
