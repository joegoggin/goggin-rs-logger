//! Shared browser-to-server log relay payloads.

use serde::{Deserialize, Serialize};

#[cfg(feature = "leptos")]
mod emitter;
#[cfg(feature = "axum")]
mod receiver;

#[cfg(feature = "leptos")]
pub use emitter::{WebLoggerConfig, init_web_logging, init_web_logging_with_config};
#[cfg(feature = "axum")]
pub use receiver::{
    RelayLogSink, RelayReceiverState, format_relay_line, relay_log_handler, relay_router,
    split_formatted_lines,
};

/// Default endpoint path for browser-to-server log relay requests.
pub const DEFAULT_WEB_LOG_RELAY_ENDPOINT: &str = "/_leptos/web-log";

/// Stable JSON schema for a browser-to-server log relay payload.
///
/// This type defines the shared on-wire contract between relay emitters and
/// receivers. The endpoint path is intentionally not part of the schema so
/// applications can choose or configure their own relay route.
#[derive(Serialize, Deserialize)]
pub struct RelayLogPayload {
    /// Log level name, such as `"error"`, `"warn"`, `"info"`, `"debug"`, or `"trace"`.
    pub level: String,
    /// Rendered log message.
    pub message: String,
    /// Optional log target/module path.
    pub target: Option<String>,
    /// Optional source file path reported by the caller.
    pub file: Option<String>,
    /// Optional source line number reported by the caller.
    pub line: Option<u32>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::RelayLogPayload;

    #[test]
    fn serializes_relay_payload_fields() {
        let payload = RelayLogPayload {
            level: "info".to_string(),
            message: "saved settings".to_string(),
            target: Some("app::settings".to_string()),
            file: Some("src/settings.rs".to_string()),
            line: Some(42),
        };

        let value = serde_json::to_value(&payload).expect("payload should serialize");

        assert_eq!(
            value,
            json!({
                "level": "info",
                "message": "saved settings",
                "target": "app::settings",
                "file": "src/settings.rs",
                "line": 42
            })
        );
    }

    #[test]
    fn deserializes_missing_optional_fields_as_none() {
        let payload: RelayLogPayload = serde_json::from_value(json!({
            "level": "error",
            "message": "request failed"
        }))
        .expect("payload should deserialize");

        assert_eq!(payload.level, "error");
        assert_eq!(payload.message, "request failed");
        assert!(payload.target.is_none());
        assert!(payload.file.is_none());
        assert!(payload.line.is_none());
    }
}
