use parking_lot::RwLock;
use ropey::Rope;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const REFRESH_TIMEOUT: Duration = Duration::from_secs(10);

const RESOLUTION_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn get_workspace_root(
    workspace: &Arc<RwLock<abundantis::workspace::WorkspaceManager>>,
) -> PathBuf {
    let guard = workspace.read();
    guard.root().to_path_buf()
}

pub async fn safe_refresh(core: &Arc<abundantis::Abundantis>, options: abundantis::RefreshOptions) {
    match tokio::time::timeout(REFRESH_TIMEOUT, core.refresh(options)).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            tracing::warn!("safe_refresh error: {}", e);
        }
        Err(_) => {
            tracing::warn!("safe_refresh timeout after {:?}", REFRESH_TIMEOUT);
        }
    }
}

pub fn spawn_background_refresh(
    core: Arc<abundantis::Abundantis>,
    options: abundantis::RefreshOptions,
) {
    tokio::spawn(async move {
        safe_refresh(&core, options).await;
    });
}

pub async fn safe_get_for_file(
    core: &Arc<abundantis::Abundantis>,
    key: &str,
    file_path: &std::path::Path,
) -> Option<std::sync::Arc<abundantis::ResolvedVariable>> {
    match tokio::time::timeout(RESOLUTION_TIMEOUT, core.get_for_file(key, file_path)).await {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => {
            tracing::warn!("get_for_file error for key '{}': {}", key, e);
            None
        }
        Err(_) => {
            tracing::error!(
                "get_for_file timeout after {:?} for key '{}'",
                RESOLUTION_TIMEOUT,
                key
            );
            None
        }
    }
}

pub async fn safe_all_for_file(
    core: &Arc<abundantis::Abundantis>,
    file_path: &std::path::Path,
) -> Vec<std::sync::Arc<abundantis::ResolvedVariable>> {
    match tokio::time::timeout(RESOLUTION_TIMEOUT, core.all_for_file(file_path)).await {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => {
            tracing::warn!("all_for_file error: {}", e);
            Vec::new()
        }
        Err(_) => {
            tracing::error!("all_for_file timeout after {:?}", RESOLUTION_TIMEOUT);
            Vec::new()
        }
    }
}

pub fn offset_to_linecol(content: &str, offset: usize) -> (u32, u32) {
    if offset >= content.len() {
        return (0, 0);
    }

    let rope = Rope::from_str(content);
    let line_idx = rope.byte_to_line(offset);
    let line_start_byte = rope.line_to_byte(line_idx);
    let col_char = rope.byte_slice(line_start_byte..offset).len_chars();

    (line_idx as u32, col_char as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_to_linecol_single_line() {
        let content = "hello world";
        assert_eq!(offset_to_linecol(content, 0), (0, 0));
        assert_eq!(offset_to_linecol(content, 6), (0, 6));
        assert_eq!(offset_to_linecol(content, 10), (0, 10));
    }

    #[test]
    fn test_offset_to_linecol_multiple_lines() {
        let content = "line1\nline2\nline3";
        // "line1\n" = 6 bytes, so offset 6 is start of line 2
        assert_eq!(offset_to_linecol(content, 0), (0, 0));
        assert_eq!(offset_to_linecol(content, 5), (0, 5)); // 'n' in line1
        assert_eq!(offset_to_linecol(content, 6), (1, 0)); // start of line2
        assert_eq!(offset_to_linecol(content, 12), (2, 0)); // start of line3
    }

    #[test]
    fn test_offset_to_linecol_out_of_bounds() {
        let content = "hello";
        assert_eq!(offset_to_linecol(content, 100), (0, 0));
        assert_eq!(offset_to_linecol(content, 5), (0, 0)); // at length (out of bounds)
    }

    #[test]
    fn test_offset_to_linecol_empty_string() {
        let content = "";
        assert_eq!(offset_to_linecol(content, 0), (0, 0));
    }

    #[test]
    fn test_offset_to_linecol_with_unicode() {
        // "ä" is 2 bytes in UTF-8, "日" is 3 bytes
        let content = "äb日c";
        // byte offsets: ä=0-1, b=2, 日=3-5, c=6
        assert_eq!(offset_to_linecol(content, 0), (0, 0)); // start of ä
        assert_eq!(offset_to_linecol(content, 2), (0, 1)); // 'b'
        assert_eq!(offset_to_linecol(content, 3), (0, 2)); // start of 日
        assert_eq!(offset_to_linecol(content, 6), (0, 3)); // 'c'
    }

    #[test]
    fn test_offset_to_linecol_windows_line_endings() {
        let content = "line1\r\nline2";
        // "line1\r\n" = 7 bytes
        assert_eq!(offset_to_linecol(content, 7), (1, 0)); // start of line2
    }

    #[test]
    fn test_offset_to_linecol_empty_lines() {
        let content = "a\n\nb";
        assert_eq!(offset_to_linecol(content, 2), (1, 0)); // empty line
        assert_eq!(offset_to_linecol(content, 3), (2, 0)); // 'b'
    }
}
