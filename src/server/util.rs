use parking_lot::RwLock;
use ropey::Rope;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Timeout for Abundantis refresh operations
const REFRESH_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for Abundantis resolution operations (get_for_file, all_for_file)
const RESOLUTION_TIMEOUT: Duration = Duration::from_secs(5);

/// Get workspace root. This is a simple read lock acquisition, fast enough
/// to run directly on the async executor.
pub async fn get_workspace_root(
    workspace: &Arc<RwLock<abundantis::workspace::WorkspaceManager>>,
) -> PathBuf {
    let guard = workspace.read();
    guard.root().to_path_buf()
}

/// Safely refresh Abundantis with a timeout to prevent indefinite hangs.
///
/// Uses direct async timeout wrapping since abundantis.refresh() is already async.
/// If the refresh times out, it logs a warning but doesn't crash the server.
pub async fn safe_refresh(
    core: &Arc<abundantis::Abundantis>,
    options: abundantis::RefreshOptions,
) {
    match tokio::time::timeout(REFRESH_TIMEOUT, core.refresh(options)).await {
        Ok(Ok(())) => {
            // Success, no logging needed in production
        }
        Ok(Err(e)) => {
            tracing::warn!("safe_refresh error: {}", e);
        }
        Err(_) => {
            tracing::warn!("safe_refresh timeout after {:?}", REFRESH_TIMEOUT);
        }
    }
}

/// Fire-and-forget refresh that won't block the current request.
///
/// Use this for refresh operations that don't need to complete before
/// responding to the user (e.g., after file changes).
pub fn spawn_background_refresh(
    core: Arc<abundantis::Abundantis>,
    options: abundantis::RefreshOptions,
) {
    tokio::spawn(async move {
        safe_refresh(&core, options).await;
    });
}

/// Safely get an environment variable for a file with timeout protection.
///
/// Uses direct async timeout wrapping since abundantis.get_for_file() is already async.
pub async fn safe_get_for_file(
    core: &Arc<abundantis::Abundantis>,
    key: &str,
    file_path: &std::path::Path,
) -> Option<std::sync::Arc<abundantis::ResolvedVariable>> {
    match tokio::time::timeout(
        RESOLUTION_TIMEOUT,
        core.get_for_file(key, file_path)
    ).await {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => {
            tracing::warn!("get_for_file error for key '{}': {}", key, e);
            None
        }
        Err(_) => {
            tracing::error!("get_for_file timeout after {:?} for key '{}'", RESOLUTION_TIMEOUT, key);
            None
        }
    }
}

/// Safely get all environment variables for a file with timeout protection.
///
/// Uses direct async timeout wrapping since abundantis.all_for_file() is already async.
pub async fn safe_all_for_file(
    core: &Arc<abundantis::Abundantis>,
    file_path: &std::path::Path,
) -> Vec<std::sync::Arc<abundantis::ResolvedVariable>> {
    match tokio::time::timeout(
        RESOLUTION_TIMEOUT,
        core.all_for_file(file_path)
    ).await {
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
