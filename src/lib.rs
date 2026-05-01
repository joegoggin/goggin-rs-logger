//! Logging utilities extracted from GigLog.
//!
//! `goggin-rs-logger` provides a small [`log`] facade backend with colorized
//! terminal output, semantic status helpers, optional Axum HTTP middleware, and
//! an optional browser-to-server relay for Leptos/WASM applications.
//!
//! # Feature flags
//!
//! * No feature flags: native logger setup, level parsing helpers, semantic
//!   [`log_message`] and [`log_success`] helpers, and [`log`] macro re-exports.
//! * `axum`: enables `HttpLoggingConfig`, Axum request/response middleware,
//!   and the server-side relay receiver APIs.
//! * `leptos`: enables `WebLoggerConfig` and browser-side relay logger setup.
//! * `axum` + `leptos`: enables both sides of the browser-to-server relay.
//!
//! # Native logger
//!
//! ```no_run
//! use goggin_rs_logger::{Logger, log_message, log_success};
//!
//! fn main() {
//!     Logger::setup_logging("info", false);
//!
//!     log_message("starting application");
//!     log::info!("ready");
//!     log_success("startup complete");
//! }
//! ```
//!
//! # Axum request and response logging
//!
//! Enable the `axum` feature to use `Logger::log_request_and_response` as
//! middleware.
//!
//! ```no_run
//! # #[cfg(feature = "axum")]
//! # {
//! use axum::{Router, middleware};
//! use goggin_rs_logger::{HttpLoggingConfig, Logger};
//!
//! fn app() -> Router {
//!     Router::new().route_layer(middleware::from_fn_with_state(
//!         HttpLoggingConfig::default(),
//!         Logger::log_request_and_response,
//!     ))
//! }
//! # }
//! ```
//!
//! # Leptos/WASM browser relay
//!
//! Enable the `leptos` feature to install a browser logger that forwards
//! frontend records as JSON relay payloads.
//!
//! ```no_run
//! # #[cfg(feature = "leptos")]
//! # {
//! use goggin_rs_logger::{
//!     WebLoggerConfig, init_web_logging, init_web_logging_with_config,
//! };
//!
//! fn install_default_logger() -> Result<(), log::SetLoggerError> {
//!     init_web_logging("info")?;
//!     Ok(())
//! }
//!
//! fn install_custom_logger() -> Result<(), log::SetLoggerError> {
//!     let config = WebLoggerConfig::new("debug").with_endpoint("/internal/web-log");
//!     init_web_logging_with_config(config)?;
//!     Ok(())
//! }
//! # }
//! ```
//!
//! # Axum relay receiver
//!
//! Enable the `axum` feature to receive browser logs on the server. The browser
//! emitter posts [`RelayLogPayload`] values to
//! `DEFAULT_WEB_LOG_RELAY_ENDPOINT` by default. The receiver formats each
//! payload and forwards every formatted line to a caller-provided
//! `RelayLogSink`.
//!
//! ```no_run
//! # #[cfg(feature = "axum")]
//! # {
//! use std::sync::Arc;
//!
//! use axum::Router;
//! use goggin_rs_logger::{RelayLogSink, RelayReceiverState, relay_router};
//!
//! fn app() -> Router {
//!     let sink: RelayLogSink = Arc::new(|line| println!("{line}"));
//!     let relay_state = RelayReceiverState::new(sink, false);
//!
//!     Router::new().merge(relay_router(relay_state))
//! }
//! # }
//! ```
//!
//! The relay payload schema is stable across the emitter and receiver:
//! `level`, `message`, optional `target`, optional `file`, and optional `line`.
//! The default endpoint is `/_leptos/web-log`; browser applications can choose
//! a different path with `WebLoggerConfig::with_endpoint` and servers can
//! mount `relay_router` anywhere that matches the emitted path.

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
///
/// # Returns
///
/// This function does not return a value.
pub fn log_message(message: &str) {
    log::info!(target: backend::MESSAGE_TARGET, "{}", message);
}

/// Logs a success message using the semantic success target.
///
/// # Arguments
///
/// * `message` - The message text to emit.
///
/// # Returns
///
/// This function does not return a value.
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
