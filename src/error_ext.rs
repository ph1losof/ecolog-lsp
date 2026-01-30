//! Error handling extensions for more ergonomic error management.
//!
//! Provides extension traits for Result types that enable logging errors
//! while converting to Option, avoiding silent error swallowing.

use tracing::{error, warn};

/// Extension trait for Result types that provides logging variants of `.ok()`.
///
/// Instead of silently discarding errors with `.ok()`, use these methods
/// to log the error before converting to Option.
pub trait ResultExt<T, E: std::fmt::Display> {
    /// Convert to Option, logging the error at error level if Err.
    ///
    /// Use this when the error represents a significant problem that
    /// should be investigated.
    fn ok_logged(self, context: &str) -> Option<T>;

    /// Convert to Option, logging the error at warn level if Err.
    ///
    /// Use this when the error is expected in some circumstances
    /// but should still be tracked.
    fn ok_warn(self, context: &str) -> Option<T>;
}

impl<T, E: std::fmt::Display> ResultExt<T, E> for Result<T, E> {
    fn ok_logged(self, context: &str) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(e) => {
                error!(context = %context, error = %e, "Operation failed");
                None
            }
        }
    }

    fn ok_warn(self, context: &str) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(e) => {
                warn!(context = %context, error = %e, "Operation failed (expected in some cases)");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ok_logged_with_ok() {
        let result: Result<i32, &str> = Ok(42);
        assert_eq!(result.ok_logged("test"), Some(42));
    }

    #[test]
    fn test_ok_logged_with_err() {
        let result: Result<i32, &str> = Err("test error");
        assert_eq!(result.ok_logged("test context"), None);
    }

    #[test]
    fn test_ok_warn_with_ok() {
        let result: Result<i32, &str> = Ok(42);
        assert_eq!(result.ok_warn("test"), Some(42));
    }

    #[test]
    fn test_ok_warn_with_err() {
        let result: Result<i32, &str> = Err("test error");
        assert_eq!(result.ok_warn("test context"), None);
    }
}
