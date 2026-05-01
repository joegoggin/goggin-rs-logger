mod backend;
mod formatting;
pub mod level;
#[cfg(feature = "axum")]
mod middleware;
#[cfg(any(feature = "axum", test))]
mod redaction;

pub use backend::Logger;
pub use level::{is_off, level_for_logger, parse_level_filter};
/// Convenience re-exports of the [`log`] facade macros.
pub use log::{debug, error, info, trace, warn};
#[cfg(feature = "axum")]
pub use middleware::HttpLoggingConfig;

/// Logs an informational message using the semantic message target.
///
/// # Arguments
///
/// * `message` - The message text to emit.
pub fn log_message(message: &str) {
    log::info!(target: backend::MESSAGE_TARGET, "{}", message);
}

/// Logs a success message using the semantic success target.
///
/// # Arguments
///
/// * `message` - The message text to emit.
pub fn log_success(message: &str) {
    log::info!(target: backend::SUCCESS_TARGET, "{}", message);
}
