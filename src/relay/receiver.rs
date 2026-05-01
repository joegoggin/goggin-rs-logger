//! Axum relay receiver for browser log payloads.
//!
//! Formats posted [`RelayLogPayload`] values with the same ANSI style used by
//! the native logger, then forwards each formatted line to a caller-provided
//! sink.

use std::sync::Arc;

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};

use super::{DEFAULT_WEB_LOG_RELAY_ENDPOINT, RelayLogPayload};
use crate::backend::{MESSAGE_TARGET, SUCCESS_TARGET};

const ANSI_BLUE: &str = "\x1b[34m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_RED: &str = "\x1b[31m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_MAGENTA: &str = "\x1b[35m";
const ANSI_PURPLE: &str = "\x1b[35m";
const ANSI_CLEAR: &str = "\x1b[0m";

/// Receives formatted relay lines.
pub type RelayLogSink = Arc<dyn Fn(String) + Send + Sync + 'static>;

/// Shared state used by relay receiver routes.
#[derive(Clone)]
pub struct RelayReceiverState {
    /// Receives each formatted output line.
    pub sink: RelayLogSink,
    /// Controls verbose multi-line formatting behavior.
    pub verbose: bool,
}

impl RelayReceiverState {
    /// Creates relay receiver state.
    ///
    /// # Arguments
    ///
    /// * `sink` - Callback invoked once for each formatted output line.
    /// * `verbose` - Enables expanded multi-line formatting when `true`.
    ///
    /// # Returns
    ///
    /// A new [`RelayReceiverState`] instance.
    pub fn new(sink: RelayLogSink, verbose: bool) -> Self {
        Self { sink, verbose }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SemanticLogKind {
    Message,
    Success,
}

/// Classifies web log payload levels into relay formatting categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebLogLevel {
    /// Represents error-level browser logs.
    Error,
    /// Represents warning-level browser logs.
    Warn,
    /// Represents info-level browser logs.
    Info,
    /// Represents debug-level browser logs.
    Debug,
    /// Represents trace-level browser logs.
    Trace,
}

impl WebLogLevel {
    /// Parses a raw level string into a normalized [`WebLogLevel`].
    ///
    /// # Arguments
    ///
    /// * `raw` - Raw level string from an incoming JSON payload.
    ///
    /// # Returns
    ///
    /// A parsed [`WebLogLevel`], defaulting to [`WebLogLevel::Info`].
    fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "error" => Self::Error,
            "warn" | "warning" => Self::Warn,
            "debug" => Self::Debug,
            "trace" => Self::Trace,
            "info" => Self::Info,
            _ => Self::Info,
        }
    }
}

/// Formats a relay payload into one or more ANSI-decorated lines.
///
/// # Arguments
///
/// * `payload` - Incoming web log payload.
/// * `verbose` - Enables expanded multi-line formatting when `true`.
///
/// # Returns
///
/// A formatted string ready for newline splitting.
pub fn format_relay_line(payload: &RelayLogPayload, verbose: bool) -> String {
    let level = WebLogLevel::parse(&payload.level);
    let message = payload.message.trim_end().to_string();
    let semantic_kind = parse_semantic_log_kind(payload.target.as_deref());

    if let Some(semantic_kind) = semantic_kind {
        return format_semantic_line(semantic_kind, &message, verbose);
    }

    let file = payload
        .file
        .as_deref()
        .map(extract_after_src)
        .filter(|value| !value.is_empty())
        .or_else(|| payload.target.clone())
        .unwrap_or_default();
    let line_number = payload
        .line
        .map(|line| line.to_string())
        .unwrap_or_default();

    if !verbose {
        return format_non_verbose(level, &message);
    }

    match level {
        WebLogLevel::Info => format!("\n{ANSI_BLUE}{message}{ANSI_CLEAR}"),
        WebLogLevel::Error => format_error_like("Error", ANSI_RED, &file, &line_number, &message),
        WebLogLevel::Debug => {
            format_error_like("Debug", ANSI_YELLOW, &file, &line_number, &message)
        }
        WebLogLevel::Warn | WebLogLevel::Trace => format!(
            "\n{ANSI_PURPLE}File: {file}{ANSI_CLEAR}\n{ANSI_PURPLE}Line Number: {line_number}{ANSI_CLEAR}\n{message}"
        ),
    }
}

/// Splits formatted output into line records while preserving blanks.
///
/// # Arguments
///
/// * `formatted` - Formatted relay output string.
///
/// # Returns
///
/// A vector of per-line strings suitable for log emission.
pub fn split_formatted_lines(formatted: &str) -> Vec<String> {
    formatted.split('\n').map(str::to_string).collect()
}

/// Accepts posted browser logs and forwards formatted lines to the sink.
///
/// # Arguments
///
/// * `state` - Shared relay receiver state.
/// * `payload` - Posted browser log payload.
///
/// # Returns
///
/// [`StatusCode::NO_CONTENT`] after the payload has been accepted.
pub async fn relay_log_handler(
    State(state): State<RelayReceiverState>,
    Json(payload): Json<RelayLogPayload>,
) -> StatusCode {
    let formatted = format_relay_line(&payload, state.verbose);

    for line in split_formatted_lines(&formatted) {
        (state.sink)(line);
    }

    StatusCode::NO_CONTENT
}

/// Builds an Axum router for receiving browser relay logs.
///
/// # Arguments
///
/// * `state` - Shared relay receiver state.
///
/// # Returns
///
/// A router that accepts `POST /` and `POST /_leptos/web-log`.
pub fn relay_router(state: RelayReceiverState) -> Router {
    Router::new()
        .route("/", post(relay_log_handler))
        .route(DEFAULT_WEB_LOG_RELAY_ENDPOINT, post(relay_log_handler))
        .with_state(state)
}

fn parse_semantic_log_kind(target: Option<&str>) -> Option<SemanticLogKind> {
    match target {
        Some(MESSAGE_TARGET) => Some(SemanticLogKind::Message),
        Some(SUCCESS_TARGET) => Some(SemanticLogKind::Success),
        _ => None,
    }
}

fn format_semantic_line(kind: SemanticLogKind, message: &str, verbose: bool) -> String {
    let (color, hashtags) = match kind {
        SemanticLogKind::Message => (ANSI_BLUE, "######"),
        SemanticLogKind::Success => (ANSI_GREEN, "######"),
    };

    if verbose {
        return format!("\n{color}{hashtags} {message} {hashtags}{ANSI_CLEAR}\n");
    }

    format!("{color}{message}{ANSI_CLEAR}")
}

fn format_non_verbose(level: WebLogLevel, message: &str) -> String {
    match level {
        WebLogLevel::Error => format!("{ANSI_RED}[ERROR] {message}{ANSI_CLEAR}"),
        WebLogLevel::Warn => format!("{ANSI_YELLOW}[WARN] {message}{ANSI_CLEAR}"),
        WebLogLevel::Info => message.to_string(),
        WebLogLevel::Debug => format!("{ANSI_BLUE}[DEBUG] {message}{ANSI_CLEAR}"),
        WebLogLevel::Trace => format!("{ANSI_MAGENTA}[TRACE] {message}{ANSI_CLEAR}"),
    }
}

fn format_error_like(
    title: &str,
    color: &str,
    file: &str,
    line_number: &str,
    message: &str,
) -> String {
    let hashtags = "######";
    format!(
        "\n{color}{hashtags} {title} {hashtags}{ANSI_CLEAR}\n{color}File: {file}{ANSI_CLEAR}\n{color}Line Number: {line_number}{ANSI_CLEAR}\n\n{color}{message}{ANSI_CLEAR}"
    )
}

fn extract_after_src(path: &str) -> String {
    let src_prefix = "src/";
    if let Some(start_index) = path.find(src_prefix) {
        return path[start_index + src_prefix.len()..].to_string();
    }

    path.to_string()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        body::Body,
        http::{Method, Request, StatusCode, header},
    };
    use tower::ServiceExt;

    use super::{
        RelayReceiverState, WebLogLevel, extract_after_src, format_non_verbose, format_relay_line,
        relay_router, split_formatted_lines,
    };
    use crate::relay::{DEFAULT_WEB_LOG_RELAY_ENDPOINT, RelayLogPayload};

    #[test]
    fn parse_level_defaults_to_info() {
        assert_eq!(WebLogLevel::parse("wat"), WebLogLevel::Info);
        assert_eq!(WebLogLevel::parse("warn"), WebLogLevel::Warn);
    }

    #[test]
    fn extract_after_src_returns_relative_suffix() {
        assert_eq!(
            extract_after_src("/tmp/project/src/main.rs"),
            "main.rs".to_string()
        );
    }

    #[test]
    fn non_verbose_error_uses_api_style_prefix() {
        let formatted = format_non_verbose(WebLogLevel::Error, "boom");
        assert!(formatted.contains("[ERROR] boom"));
        assert!(formatted.contains("\u{1b}[31m"));
    }

    #[test]
    fn verbose_debug_uses_banner_format() {
        let payload = RelayLogPayload {
            level: "debug".to_string(),
            message: "test log".to_string(),
            target: Some("app::module".to_string()),
            file: Some("/tmp/project/src/app/mod.rs".to_string()),
            line: Some(42),
        };

        let formatted = format_relay_line(&payload, true);
        assert!(formatted.contains("###### Debug ######"));
        assert!(formatted.contains("File: app/mod.rs"));
        assert!(formatted.contains("Line Number: 42"));
        assert!(formatted.contains("test log"));
    }

    #[test]
    fn split_formatted_lines_preserves_blank_lines() {
        let formatted = "\n\x1b[31m###### Error ######\x1b[0m\n\n\x1b[31mboom\x1b[0m\n";
        let lines = split_formatted_lines(formatted);

        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "");
        assert!(lines[1].contains("###### Error ######"));
        assert_eq!(lines[2], "");
        assert!(lines[3].contains("boom"));
        assert_eq!(lines[4], "");
    }

    #[test]
    fn semantic_success_uses_api_style_formatting() {
        let payload = RelayLogPayload {
            level: "info".to_string(),
            message: "Database connection established".to_string(),
            target: Some("goggin_rs_logger::success".to_string()),
            file: Some("/tmp/project/src/app/mod.rs".to_string()),
            line: Some(42),
        };

        let verbose = format_relay_line(&payload, true);
        assert!(verbose.contains("###### Database connection established ######"));
        assert!(verbose.contains("\u{1b}[32m"));

        let non_verbose = format_relay_line(&payload, false);
        assert!(non_verbose.contains("Database connection established"));
        assert!(non_verbose.contains("\u{1b}[32m"));
    }

    #[tokio::test]
    async fn relay_router_posts_payload_to_sink() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let sink_received = Arc::clone(&received);
        let state = RelayReceiverState::new(
            Arc::new(move |line| {
                sink_received
                    .lock()
                    .expect("received lines should not be poisoned")
                    .push(line);
            }),
            false,
        );
        let app = relay_router(state);

        let response = app
            .clone()
            .oneshot(relay_request(DEFAULT_WEB_LOG_RELAY_ENDPOINT))
            .await
            .expect("default endpoint request should route");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let response = app
            .oneshot(relay_request("/"))
            .await
            .expect("root endpoint request should route");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let lines = received
            .lock()
            .expect("received lines should not be poisoned");
        assert_eq!(
            lines.as_slice(),
            [
                "\u{1b}[31m[ERROR] boom\u{1b}[0m",
                "\u{1b}[31m[ERROR] boom\u{1b}[0m"
            ]
        );
    }

    fn relay_request(uri: &str) -> Request<Body> {
        let payload = RelayLogPayload {
            level: "error".to_string(),
            message: "boom".to_string(),
            target: Some("app::module".to_string()),
            file: Some("/tmp/project/src/app/mod.rs".to_string()),
            line: Some(42),
        };
        let body = serde_json::to_vec(&payload).expect("payload should serialize");

        Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .expect("request should build")
    }
}
