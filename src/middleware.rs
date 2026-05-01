//! Axum middleware for logging HTTP requests and responses.
//!
//! Captures request method, path, headers, and optionally the JSON body, then
//! logs the corresponding response with status code and duration. Sensitive
//! headers and JSON fields are redacted before output.

use std::time::Instant;

use axum::{
    body::{Body, HttpBody, to_bytes},
    extract::{Request, State},
    http::{
        Method,
        header::{CONTENT_LENGTH, CONTENT_TYPE, HeaderMap},
    },
    middleware::Next,
    response::Response,
};
use log::Level;
use uuid::Uuid;

use crate::{
    Logger,
    formatting::{log_compact_http, log_request, log_response},
    redaction::parse_redacted_json,
};

/// Configuration for the HTTP logging middleware.
#[derive(Debug, Clone)]
pub struct HttpLoggingConfig {
    /// Whether to capture and log request/response bodies.
    pub body_enabled: bool,
    /// Maximum body size in bytes that will be buffered for logging.
    pub max_body_bytes: usize,
    /// Use verbose multi-line output instead of compact single-line format.
    pub verbose: bool,
}

impl HttpLoggingConfig {
    /// Creates a new configuration with the given settings.
    ///
    /// # Arguments
    ///
    /// * `body_enabled` - Whether to capture and log request/response bodies.
    /// * `max_body_bytes` - Maximum body size in bytes that will be buffered
    ///   for logging.
    /// * `verbose` - Use verbose multi-line output instead of compact
    ///   single-line format.
    ///
    /// # Returns
    ///
    /// A new [`HttpLoggingConfig`] instance.
    pub fn new(body_enabled: bool, max_body_bytes: usize, verbose: bool) -> Self {
        Self {
            body_enabled,
            max_body_bytes,
            verbose,
        }
    }
}

/// Defaults to body logging disabled, 16 KB max body size, and verbose output.
impl Default for HttpLoggingConfig {
    fn default() -> Self {
        Self {
            body_enabled: false,
            max_body_bytes: 16 * 1024,
            verbose: true,
        }
    }
}

impl Logger {
    /// Axum middleware that logs each HTTP request and its response.
    ///
    /// In verbose mode, prints detailed headers and bodies; otherwise prints a
    /// compact one-line summary. Body logging is gated on
    /// [`HttpLoggingConfig::body_enabled`] and respects the configured
    /// [`max_body_bytes`](HttpLoggingConfig::max_body_bytes) limit.
    ///
    /// # Arguments
    ///
    /// * `config` - The [`HttpLoggingConfig`] extracted from Axum state.
    /// * `request` - The incoming HTTP request.
    /// * `next` - The next middleware or handler in the chain.
    ///
    /// # Returns
    ///
    /// The HTTP [`Response`] produced by downstream handlers.
    pub async fn log_request_and_response(
        State(config): State<HttpLoggingConfig>,
        request: Request,
        next: Next,
    ) -> Response {
        if !log::log_enabled!(Level::Info) {
            return next.run(request).await;
        }

        let request_id = Uuid::new_v4();
        let use_verbose_http_logs = config.verbose || config.body_enabled;
        let (request_parts, request_body) = request.into_parts();
        let method = request_parts.method.clone();
        let path = request_parts.uri.path().to_string();
        let headers = request_parts.headers.clone();

        let (logged_request_body, request_body_for_next) = if should_attempt_request_body_logging(
            &method, &headers, &config,
        ) {
            match to_bytes(request_body, config.max_body_bytes).await {
                Ok(bytes) => {
                    let logged_body = parse_redacted_json(&bytes);
                    (logged_body, Body::from(bytes))
                }
                Err(error) => {
                    log::error!(
                        "Skipping request body logging for request {} (failed to buffer request body: {})",
                        request_id,
                        error
                    );

                    (None, Body::empty())
                }
            }
        } else {
            (None, request_body)
        };

        let request = Request::from_parts(request_parts, request_body_for_next);

        if use_verbose_http_logs {
            log_request(request_id, &method, &path, &headers, logged_request_body);
        }

        let started_at = Instant::now();
        let response = next.run(request).await;
        let duration_ms = started_at.elapsed().as_millis();
        let status = response.status();

        if !use_verbose_http_logs {
            log_compact_http(request_id, &method, &path, status, duration_ms);
            return response;
        }

        let (response_parts, response_body) = response.into_parts();
        let response_body_size_hint_upper = response_body.size_hint().upper();

        if !should_attempt_response_body_logging(
            &response_parts.headers,
            response_body_size_hint_upper,
            &config,
        ) {
            log_response(request_id, status, duration_ms, None);
            return Response::from_parts(response_parts, response_body);
        }

        let (logged_response_body, response_body_for_client) = match to_bytes(
            response_body,
            config.max_body_bytes,
        )
        .await
        {
            Ok(bytes) => {
                let logged_body = parse_redacted_json(&bytes);
                (logged_body, Body::from(bytes))
            }
            Err(error) => {
                log::error!(
                    "Skipping response body logging for request {} (failed to buffer response body: {})",
                    request_id,
                    error
                );

                (None, Body::empty())
            }
        };

        log_response(request_id, status, duration_ms, logged_response_body);

        Response::from_parts(response_parts, response_body_for_client)
    }
}

/// Returns `true` if the request body should be logged based on method,
/// content type, and size.
///
/// # Arguments
///
/// * `method` - The HTTP method of the request.
/// * `headers` - The request headers.
/// * `config` - The logging configuration.
///
/// # Returns
///
/// `true` if the body should be captured for logging.
fn should_attempt_request_body_logging(
    method: &Method,
    headers: &HeaderMap,
    config: &HttpLoggingConfig,
) -> bool {
    if !config.body_enabled {
        return false;
    }

    if matches!(method, &Method::GET | &Method::DELETE | &Method::HEAD) {
        return false;
    }

    if !is_json_content_type(headers) {
        return false;
    }

    match content_length(headers) {
        Some(length) => length <= config.max_body_bytes,
        None => false,
    }
}

/// Returns `true` if the response body should be logged based on content type
/// and size.
///
/// # Arguments
///
/// * `headers` - The response headers.
/// * `body_size_hint_upper` - Optional upper bound of the body size from the
///   response body's size hint.
/// * `config` - The logging configuration.
///
/// # Returns
///
/// `true` if the body should be captured for logging.
fn should_attempt_response_body_logging(
    headers: &HeaderMap,
    body_size_hint_upper: Option<u64>,
    config: &HttpLoggingConfig,
) -> bool {
    if !config.body_enabled {
        return false;
    }

    if !is_json_content_type(headers) {
        return false;
    }

    match content_length(headers)
        .or_else(|| body_size_hint_upper.and_then(|upper_bound| usize::try_from(upper_bound).ok()))
    {
        Some(length) => length <= config.max_body_bytes,
        None => false,
    }
}

/// Returns `true` if the `Content-Type` header indicates JSON.
///
/// # Arguments
///
/// * `headers` - The HTTP header map to inspect.
///
/// # Returns
///
/// `true` if the `Content-Type` contains `application/json` or `+json`.
fn is_json_content_type(headers: &HeaderMap) -> bool {
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase());

    match content_type {
        Some(content_type) => {
            content_type.contains("application/json") || content_type.contains("+json")
        }
        None => false,
    }
}

/// Parses the `Content-Length` header as a `usize`, returning `None` if absent
/// or invalid.
///
/// # Arguments
///
/// * `headers` - The HTTP header map to inspect.
///
/// # Returns
///
/// The parsed content length, or `None` if the header is missing or cannot be
/// parsed.
fn content_length(headers: &HeaderMap) -> Option<usize> {
    headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
}

#[cfg(test)]
mod tests {
    use axum::http::{Method, header};

    use super::{
        HttpLoggingConfig, should_attempt_request_body_logging,
        should_attempt_response_body_logging,
    };

    #[test]
    fn request_body_logging_requires_json_with_allowed_method_and_size() {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::CONTENT_LENGTH,
            header::HeaderValue::from_static("8"),
        );

        let config = HttpLoggingConfig {
            body_enabled: true,
            max_body_bytes: 16,
            verbose: true,
        };

        assert!(should_attempt_request_body_logging(
            &Method::POST,
            &headers,
            &config
        ));
        assert!(!should_attempt_request_body_logging(
            &Method::GET,
            &headers,
            &config
        ));

        headers.insert(
            header::CONTENT_LENGTH,
            header::HeaderValue::from_static("32"),
        );

        assert!(!should_attempt_request_body_logging(
            &Method::POST,
            &headers,
            &config
        ));
    }

    #[test]
    fn response_body_logging_uses_content_length_or_size_hint() {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::CONTENT_LENGTH,
            header::HeaderValue::from_static("48"),
        );

        let config = HttpLoggingConfig {
            body_enabled: true,
            max_body_bytes: 128,
            verbose: true,
        };

        assert!(should_attempt_response_body_logging(
            &headers, None, &config
        ));

        headers.remove(header::CONTENT_LENGTH);

        assert!(should_attempt_response_body_logging(
            &headers,
            Some(48),
            &config
        ));
        assert!(!should_attempt_response_body_logging(
            &headers,
            Some(256),
            &config
        ));
        assert!(!should_attempt_response_body_logging(
            &headers, None, &config
        ));
    }

    #[test]
    fn body_logging_is_disabled_outside_development() {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::CONTENT_LENGTH,
            header::HeaderValue::from_static("8"),
        );

        let config = HttpLoggingConfig {
            body_enabled: false,
            max_body_bytes: 128,
            verbose: true,
        };

        assert!(!should_attempt_request_body_logging(
            &Method::POST,
            &headers,
            &config
        ));
    }

    #[test]
    fn body_logging_does_not_depend_on_verbose_flag() {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::CONTENT_LENGTH,
            header::HeaderValue::from_static("8"),
        );

        let config = HttpLoggingConfig {
            body_enabled: true,
            max_body_bytes: 128,
            verbose: false,
        };

        assert!(should_attempt_request_body_logging(
            &Method::POST,
            &headers,
            &config
        ));
    }
}
