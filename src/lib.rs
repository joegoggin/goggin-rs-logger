mod backend;
mod formatting;
pub mod level;
#[cfg(feature = "axum")]
mod middleware;
#[cfg(any(feature = "axum", test))]
mod redaction;
pub mod relay;

pub use backend::Logger;
pub use level::{is_off, level_for_logger, parse_level_filter};
/// Convenience re-exports of the [`log`] facade macros.
pub use log::{debug, error, info, trace, warn};
#[cfg(feature = "axum")]
pub use middleware::HttpLoggingConfig;
#[cfg(any(feature = "axum", feature = "leptos"))]
pub use relay::DEFAULT_WEB_LOG_RELAY_ENDPOINT;
pub use relay::RelayLogPayload;
#[cfg(feature = "axum")]
pub use relay::{
    RelayLogSink, RelayReceiverState, format_relay_line, relay_log_handler, relay_router,
    split_formatted_lines,
};
#[cfg(feature = "leptos")]
pub use relay::{WebLoggerConfig, init_web_logging, init_web_logging_with_config};

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

#[cfg(all(test, feature = "leptos"))]
mod leptos_public_api_tests {
    use super::{
        DEFAULT_WEB_LOG_RELAY_ENDPOINT, WebLoggerConfig, init_web_logging,
        init_web_logging_with_config,
    };

    #[test]
    fn leptos_public_api_is_exported_from_crate_root() {
        let _: &'static str = DEFAULT_WEB_LOG_RELAY_ENDPOINT;
        let _: fn(&'static str) -> Result<WebLoggerConfig, log::SetLoggerError> = init_web_logging;
        let _: fn(WebLoggerConfig) -> Result<WebLoggerConfig, log::SetLoggerError> =
            init_web_logging_with_config;
    }
}

#[cfg(all(test, feature = "axum"))]
mod axum_public_api_tests {
    use std::{future::Future, sync::Arc};

    use axum::{Json, extract::State, http::StatusCode};

    use super::{
        DEFAULT_WEB_LOG_RELAY_ENDPOINT, RelayLogPayload, RelayLogSink, RelayReceiverState,
        format_relay_line, relay_log_handler, relay_router, split_formatted_lines,
    };

    #[test]
    fn axum_relay_public_api_is_exported_from_crate_root() {
        let _: &'static str = DEFAULT_WEB_LOG_RELAY_ENDPOINT;
        let sink: RelayLogSink = Arc::new(|_| {});
        let state = RelayReceiverState::new(sink, true);
        let _: fn(&RelayLogPayload, bool) -> String = format_relay_line;
        let _: fn(&str) -> Vec<String> = split_formatted_lines;
        let _ = relay_router(state);
    }

    #[test]
    fn axum_relay_handler_return_type_is_status_code() {
        fn assert_status_future<F>(_future: F)
        where
            F: Future<Output = StatusCode>,
        {
        }

        let state = RelayReceiverState::new(Arc::new(|_| {}), false);
        assert_status_future(relay_log_handler(State(state), Json(sample_payload())));
    }

    fn sample_payload() -> RelayLogPayload {
        RelayLogPayload {
            level: "info".to_string(),
            message: "hello".to_string(),
            target: None,
            file: None,
            line: None,
        }
    }
}
