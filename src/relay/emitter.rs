//! Browser-side log relay emitter for Leptos WASM applications.
//!
//! Captures records emitted through the [`log`] facade and forwards them to a
//! server relay endpoint as [`RelayLogPayload`] JSON.

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

use gloo_net::http::Request;
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use wasm_bindgen_futures::spawn_local;

use super::{DEFAULT_WEB_LOG_RELAY_ENDPOINT, RelayLogPayload};
use crate::level::{is_off, parse_level_filter};

const MAX_RELAY_QUEUE_LEN: usize = 1024;

/// Stores resolved browser logger configuration values.
#[derive(Debug, Clone, Copy)]
pub struct WebLoggerConfig {
    /// Configured level string used for parsing.
    pub configured_level: &'static str,
    /// Parsed [`LevelFilter`] used by the logger.
    pub level_filter: LevelFilter,
    /// Endpoint path used for browser-to-server log relay requests.
    pub endpoint: &'static str,
}

impl WebLoggerConfig {
    /// Creates a browser logger configuration from the compile-time web level.
    ///
    /// `WEB_LOG_LEVEL` takes precedence when it is set at compile time.
    /// Otherwise, `default_level` is used.
    ///
    /// # Arguments
    ///
    /// * `default_level` - Fallback level string used when `WEB_LOG_LEVEL` is
    ///   not set.
    ///
    /// # Returns
    ///
    /// A [`WebLoggerConfig`] using the resolved level and the default relay
    /// endpoint.
    pub fn new(default_level: &'static str) -> Self {
        let configured_level = option_env!("WEB_LOG_LEVEL").unwrap_or(default_level);
        let level_filter = parse_level_filter(configured_level);

        Self {
            configured_level,
            level_filter,
            endpoint: DEFAULT_WEB_LOG_RELAY_ENDPOINT,
        }
    }

    /// Returns this configuration with a custom relay endpoint.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - Endpoint path used for browser-to-server relay requests.
    ///
    /// # Returns
    ///
    /// A [`WebLoggerConfig`] with the same level settings and the provided
    /// endpoint.
    pub fn with_endpoint(self, endpoint: &'static str) -> Self {
        Self { endpoint, ..self }
    }
}

/// Implements a [`Log`] sink that relays frontend records over HTTP.
struct WebRelayLogger;

static WEB_RELAY_LOGGER: WebRelayLogger = WebRelayLogger;

thread_local! {
    static RELAY_QUEUE: RefCell<VecDeque<RelayLogPayload>> = RefCell::new(VecDeque::new());
    static RELAY_SENDING: Cell<bool> = const { Cell::new(false) };
    static RELAY_ENDPOINT: Cell<&'static str> = const { Cell::new(DEFAULT_WEB_LOG_RELAY_ENDPOINT) };
}

/// Initializes browser logging with the default relay endpoint.
///
/// Resolves the log level from `WEB_LOG_LEVEL`, updates the max log level, and
/// installs the relay logger when the level is not `off`.
///
/// # Arguments
///
/// * `default_level` - Fallback level string used when `WEB_LOG_LEVEL` is not
///   set.
///
/// # Returns
///
/// The resolved [`WebLoggerConfig`] used to initialize browser logging.
///
/// # Errors
///
/// Returns [`SetLoggerError`] if another logger has already been installed.
pub fn init_web_logging(default_level: &'static str) -> Result<WebLoggerConfig, SetLoggerError> {
    init_web_logging_with_config(WebLoggerConfig::new(default_level))
}

/// Initializes browser logging with the provided configuration.
///
/// Stores the configured endpoint before registering the logger so records
/// emitted immediately after registration use the selected relay path.
///
/// # Arguments
///
/// * `config` - Resolved browser logger configuration.
///
/// # Returns
///
/// The [`WebLoggerConfig`] used to initialize browser logging.
///
/// # Errors
///
/// Returns [`SetLoggerError`] if another logger has already been installed.
pub fn init_web_logging_with_config(
    config: WebLoggerConfig,
) -> Result<WebLoggerConfig, SetLoggerError> {
    store_endpoint(config.endpoint);
    log::set_max_level(config.level_filter);

    if is_off(config.level_filter) {
        return Ok(config);
    }

    log::set_logger(&WEB_RELAY_LOGGER)?;

    Ok(config)
}

impl Log for WebRelayLogger {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let payload = RelayLogPayload {
            level: record.level().to_string().to_ascii_lowercase(),
            message: record.args().to_string(),
            target: Some(record.target().to_string()),
            file: record.file().map(str::to_string),
            line: record.line(),
        };

        enqueue_payload(payload);
    }

    fn flush(&self) {}
}

/// Enqueues a relay payload and starts the sender task when idle.
fn enqueue_payload(payload: RelayLogPayload) {
    RELAY_QUEUE.with(|queue| {
        let mut queue = queue.borrow_mut();
        if queue.len() >= MAX_RELAY_QUEUE_LEN {
            let _ = queue.pop_front();
        }
        queue.push_back(payload);
    });

    start_sender_if_needed();
}

/// Starts the async relay sender loop when it is not already running.
fn start_sender_if_needed() {
    let should_start = RELAY_SENDING.with(|sending| {
        if sending.get() {
            false
        } else {
            sending.set(true);
            true
        }
    });

    if !should_start {
        return;
    }

    spawn_local(async {
        loop {
            let next = RELAY_QUEUE.with(|queue| queue.borrow_mut().pop_front());
            match next {
                Some(payload) => send_payload(payload).await,
                None => {
                    RELAY_SENDING.with(|sending| sending.set(false));
                    break;
                }
            }
        }
    });
}

/// Sends a single relay payload to the configured backend endpoint.
async fn send_payload(payload: RelayLogPayload) {
    let request = match Request::post(configured_endpoint()).json(&payload) {
        Ok(request) => request,
        Err(_) => return,
    };

    let _ = request.send().await;
}

fn store_endpoint(endpoint: &'static str) {
    RELAY_ENDPOINT.with(|stored_endpoint| stored_endpoint.set(endpoint));
}

fn configured_endpoint() -> &'static str {
    RELAY_ENDPOINT.with(Cell::get)
}

#[cfg(test)]
mod tests {
    use log::LevelFilter;

    use super::{
        DEFAULT_WEB_LOG_RELAY_ENDPOINT, WebLoggerConfig, configured_endpoint,
        init_web_logging_with_config,
    };

    #[test]
    fn config_new_uses_default_endpoint() {
        let config = WebLoggerConfig::new("debug");

        assert_eq!(config.endpoint, DEFAULT_WEB_LOG_RELAY_ENDPOINT);
    }

    #[test]
    fn config_with_endpoint_overrides_endpoint() {
        let config = WebLoggerConfig {
            configured_level: "info",
            level_filter: LevelFilter::Info,
            endpoint: DEFAULT_WEB_LOG_RELAY_ENDPOINT,
        };

        let config = config.with_endpoint("/custom/web-log");

        assert_eq!(config.configured_level, "info");
        assert_eq!(config.level_filter, LevelFilter::Info);
        assert_eq!(config.endpoint, "/custom/web-log");
    }

    #[test]
    fn init_with_config_stores_endpoint_before_registration() {
        let config = WebLoggerConfig {
            configured_level: "off",
            level_filter: LevelFilter::Off,
            endpoint: "/custom/web-log",
        };

        let config = init_web_logging_with_config(config).expect("off level should not set logger");

        assert_eq!(config.endpoint, "/custom/web-log");
        assert_eq!(configured_endpoint(), "/custom/web-log");
    }
}
