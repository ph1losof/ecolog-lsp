//! Environment variable service for resolving env vars.
//!
//! This service wraps `abundantis::Abundantis` and provides a focused interface
//! for environment variable resolution with ~90% cohesion.

use abundantis::{Abundantis, ResolvedVariable};
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Timeout for resolution operations.
const RESOLUTION_TIMEOUT: Duration = Duration::from_secs(5);

/// Timeout for refresh operations.
const REFRESH_TIMEOUT: Duration = Duration::from_secs(10);

/// Service for environment variable resolution.
///
/// Provides a cohesive interface for all env var resolution operations including:
/// - Resolving individual env vars
/// - Getting all env vars for a file context
/// - Refreshing env var sources
/// - Managing active files and workspace root
pub struct EnvService {
    core: Arc<Abundantis>,
}

impl EnvService {
    /// Creates a new EnvService wrapping the given Abundantis core.
    pub fn new(core: Arc<Abundantis>) -> Self {
        Self { core }
    }

    /// Returns a reference to the underlying Abundantis core.
    ///
    /// This allows direct access when needed for operations not yet wrapped
    /// by the service interface.
    #[inline]
    pub fn core(&self) -> &Arc<Abundantis> {
        &self.core
    }

    /// Gets the workspace root path.
    pub async fn get_workspace_root(&self) -> PathBuf {
        let guard = self.core.workspace.read();
        guard.root().to_path_buf()
    }

    /// Returns a reference to the workspace manager.
    pub fn workspace(&self) -> &Arc<RwLock<abundantis::workspace::WorkspaceManager>> {
        &self.core.workspace
    }

    /// Gets the workspace context for a file.
    pub fn get_context_for_file(&self, file_path: &Path) -> Option<abundantis::WorkspaceContext> {
        let workspace = self.core.workspace.read();
        workspace.context_for_file(file_path)
    }

    /// Resolves a single env var for a file context with timeout protection.
    pub async fn get_for_file(
        &self,
        key: &str,
        file_path: &Path,
    ) -> Option<Arc<ResolvedVariable>> {
        match tokio::time::timeout(RESOLUTION_TIMEOUT, self.core.get_for_file(key, file_path)).await
        {
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

    /// Gets all env vars for a file context with timeout protection.
    pub async fn all_for_file(&self, file_path: &Path) -> Vec<Arc<ResolvedVariable>> {
        match tokio::time::timeout(RESOLUTION_TIMEOUT, self.core.all_for_file(file_path)).await {
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

    /// Refreshes env var sources with timeout protection.
    pub async fn refresh(&self, options: abundantis::RefreshOptions) {
        match tokio::time::timeout(REFRESH_TIMEOUT, self.core.refresh(options)).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::warn!("refresh error: {}", e);
            }
            Err(_) => {
                tracing::warn!("refresh timeout after {:?}", REFRESH_TIMEOUT);
            }
        }
    }

    /// Spawns a background refresh task.
    pub fn spawn_background_refresh(&self, options: abundantis::RefreshOptions) {
        let core = Arc::clone(&self.core);
        tokio::spawn(async move {
            match tokio::time::timeout(REFRESH_TIMEOUT, core.refresh(options)).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    tracing::warn!("background refresh error: {}", e);
                }
                Err(_) => {
                    tracing::warn!("background refresh timeout after {:?}", REFRESH_TIMEOUT);
                }
            }
        });
    }

    /// Sets the active env files filter.
    pub fn set_active_files(&self, patterns: &[String]) {
        self.core.set_active_files(patterns);
    }

    /// Clears the active env files filter.
    pub fn clear_active_files(&self) {
        self.core.clear_active_files();
    }

    /// Gets the active env files for a path.
    pub fn active_env_files(&self, path: impl AsRef<Path>) -> Vec<PathBuf> {
        self.core.active_env_files(path)
    }

    /// Sets a new workspace root.
    pub async fn set_root(&self, new_root: &Path) -> Result<(), abundantis::AbundantisError> {
        self.core.set_root(new_root).await
    }

    /// Gets all registered file paths.
    /// Delegates to core.registry.registered_file_paths().
    pub fn registered_file_paths(&self) -> Vec<PathBuf> {
        self.core.registry.registered_file_paths()
    }
}

impl Clone for EnvService {
    fn clone(&self) -> Self {
        Self {
            core: Arc::clone(&self.core),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full testing of EnvService requires setting up abundantis::Abundantis
    // which involves filesystem and workspace setup. These tests focus on the
    // service wrapper behavior.

    #[test]
    fn test_resolution_timeout_constant() {
        assert_eq!(RESOLUTION_TIMEOUT, Duration::from_secs(5));
    }

    #[test]
    fn test_refresh_timeout_constant() {
        assert_eq!(REFRESH_TIMEOUT, Duration::from_secs(10));
    }
}
