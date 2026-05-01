//! Log output formatting for general log records and optional HTTP traffic.
//!
//! Provides colorized, structured output helpers used by the [`Logger`](crate::Logger)
//! backend. When the `axum` feature is enabled, this module also includes
//! request/response formatting helpers for future HTTP middleware support.

#[cfg(feature = "axum")]
use axum::http::{HeaderMap, Method, StatusCode};
#[cfg(feature = "axum")]
use colorized::colorize_print;
use colorized::{Colors, colorize_println};
use log::Record;
#[cfg(feature = "axum")]
use serde_json::Value;
#[cfg(feature = "axum")]
use uuid::Uuid;

#[cfg(feature = "axum")]
use crate::redaction::sanitize_header_value;

/// Broad classification of an HTTP status code for color selection.
#[cfg(feature = "axum")]
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StatusClass {
    /// Indicates a 2xx successful response.
    Success,
    /// Indicates a 4xx client error response.
    ClientError,
    /// Indicates a 5xx server error response.
    ServerError,
    /// Indicates any non-2xx/4xx/5xx response class.
    Other,
}

/// Maps an HTTP status code to its [`StatusClass`].
///
/// # Arguments
///
/// * `status_code` - The HTTP status code to classify.
///
/// # Returns
///
/// The corresponding [`StatusClass`] variant.
#[cfg(feature = "axum")]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn classify_status(status_code: StatusCode) -> StatusClass {
    if status_code.is_success() {
        StatusClass::Success
    } else if status_code.is_client_error() {
        StatusClass::ClientError
    } else if status_code.is_server_error() {
        StatusClass::ServerError
    } else {
        StatusClass::Other
    }
}

/// Returns a string of `#` characters with the given length, used for banner headings.
///
/// # Arguments
///
/// * `level` - The number of `#` characters to produce.
///
/// # Returns
///
/// A [`String`] containing `level` hash characters.
pub(super) fn get_hashtags(level: i8) -> String {
    let mut hashtags = String::new();

    for _ in 0..level {
        hashtags.push('#');
    }

    hashtags
}

/// Returns an indentation string of `level * 2` spaces, used for nested JSON output.
///
/// # Arguments
///
/// * `level` - The nesting depth; each level adds two spaces.
///
/// # Returns
///
/// A [`String`] containing `level * 2` space characters.
#[cfg(any(feature = "axum", test))]
pub(super) fn get_spaces(level: i8) -> String {
    let mut spaces = String::new();

    for _ in 0..level {
        spaces.push_str("  ");
    }

    spaces
}

/// Prints a colored heading surrounded by the given hashtag string.
///
/// # Arguments
///
/// * `header` - The heading text.
/// * `hashtags` - The `#` string placed on each side of the heading.
#[cfg(feature = "axum")]
#[allow(dead_code)]
fn log_header(header: &str, hashtags: &str) {
    let header_string = format!("\n{} {} {}\n", hashtags, header, hashtags);
    colorize_println(header_string, Colors::MagentaFg);
}

/// Prints a level-1 heading (6 `#` characters).
///
/// # Arguments
///
/// * `header` - The heading text.
#[cfg(feature = "axum")]
#[allow(dead_code)]
fn log_h1(header: &str) {
    log_header(header, &get_hashtags(6));
}

/// Prints a level-2 heading (5 `#` characters).
///
/// # Arguments
///
/// * `header` - The heading text.
#[cfg(feature = "axum")]
#[allow(dead_code)]
fn log_h2(header: &str) {
    log_header(header, &get_hashtags(5));
}

/// Pretty-prints a JSON value with colorized keys and indentation.
///
/// # Arguments
///
/// * `json` - The JSON value to print.
/// * `level` - The current nesting depth for indentation.
#[cfg(feature = "axum")]
#[allow(dead_code)]
pub(super) fn log_json(json: Value, level: i8) {
    match json {
        Value::Array(values) => {
            log_array(values, level);
        }
        json => {
            if let Some(json_object) = json.as_object() {
                if level == 0 {
                    println!("{{");
                }

                for (key, value) in json_object {
                    print!("{}", get_spaces(level + 1));
                    colorize_print(key, Colors::CyanFg);
                    print!(": ");

                    match value {
                        Value::Array(values) => log_array(values.clone(), level + 1),
                        Value::String(value) => {
                            colorize_print(format!("\"{}\"", value), Colors::GreenFg);
                            println!(",");
                        }
                        Value::Object(value) => {
                            println!("{{");
                            log_json(Value::Object(value.clone()), level + 2);
                            print!("{}", get_spaces(level + 1));
                            println!("}},");
                        }
                        value => {
                            colorize_print(value.to_string(), Colors::MagentaFg);
                            println!(",");
                        }
                    }
                }

                if level == 0 {
                    println!("}}");
                }
            }
        }
    }
}

/// Pretty-prints a JSON array with indentation at the given nesting level.
///
/// # Arguments
///
/// * `values` - The array elements to print.
/// * `level` - The current nesting depth for indentation.
#[cfg(feature = "axum")]
#[allow(dead_code)]
fn log_array(values: Vec<Value>, level: i8) {
    if values.is_empty() {
        println!("[],");
        return;
    }

    if level > 0 {
        print!("\n{}", get_spaces(level));
    }

    println!("[");

    for (index, value) in values.iter().enumerate() {
        print!("{}", get_spaces(level + 1));
        println!("{{");
        log_json(value.to_owned(), level + 2);
        print!("{}", get_spaces(level + 1));

        if index == values.len() - 1 {
            println!("}}");
        } else {
            println!("}},");
        }
    }

    if level > 0 {
        print!("{}", get_spaces(level));
    }

    if level == 0 {
        println!("]");
    } else {
        println!("],");
    }
}

/// Prints HTTP headers with sensitive values redacted.
///
/// # Arguments
///
/// * `headers` - The HTTP header map to print.
#[cfg(feature = "axum")]
#[allow(dead_code)]
pub(super) fn log_headers(headers: &HeaderMap) {
    for (key, value) in headers {
        colorize_print(format!("{}: ", key), Colors::CyanFg);
        let value = value.to_str().unwrap_or("<non-utf8>");
        let sanitized = sanitize_header_value(key.as_str(), value);
        println!("{}", sanitized);
    }
}

/// Prints a verbose HTTP request log with headers and optional body.
///
/// # Arguments
///
/// * `id` - Unique request identifier.
/// * `method` - The HTTP method.
/// * `route` - The request path.
/// * `headers` - The request headers.
/// * `req_body` - Optional redacted JSON body to log.
#[cfg(feature = "axum")]
#[allow(dead_code)]
pub(super) fn log_request(
    id: Uuid,
    method: &Method,
    route: &str,
    headers: &HeaderMap,
    req_body: Option<Value>,
) {
    log_h1("Request");

    colorize_print("Request ID: ", Colors::CyanFg);
    println!("{}\n", id);

    colorize_print(format!("{} ", method), Colors::CyanFg);
    println!("{}", route);

    log_h2("Headers");
    log_headers(headers);

    if let Some(req_body) = req_body {
        log_h2("Body");
        log_json(req_body, 0);
    }
}

/// Prints a verbose HTTP response log with status, duration, and optional body.
///
/// # Arguments
///
/// * `id` - Unique request identifier.
/// * `status_code` - The HTTP response status.
/// * `duration_ms` - Request processing time in milliseconds.
/// * `res_body` - Optional redacted JSON body to log.
#[cfg(feature = "axum")]
#[allow(dead_code)]
pub(super) fn log_response(
    id: Uuid,
    status_code: StatusCode,
    duration_ms: u128,
    res_body: Option<Value>,
) {
    log_h1("Response");

    colorize_print("Request ID: ", Colors::CyanFg);
    println!("{}\n", id);

    colorize_print("Status Code: ", Colors::CyanFg);

    match classify_status(status_code) {
        StatusClass::Success => colorize_println(status_code.to_string(), Colors::GreenFg),
        StatusClass::ClientError => colorize_println(status_code.to_string(), Colors::YellowFg),
        StatusClass::ServerError => colorize_println(status_code.to_string(), Colors::RedFg),
        StatusClass::Other => colorize_println(status_code.to_string(), Colors::MagentaFg),
    }

    colorize_print("Duration: ", Colors::CyanFg);
    colorize_println(format!("{} ms", duration_ms), Colors::BlueFg);

    if let Some(res_body) = res_body {
        log_h2("Body");
        log_json(res_body, 0);
    }
}

/// Prints a single-line compact HTTP log: `[id] METHOD /path -> STATUS (duration ms)`.
///
/// # Arguments
///
/// * `id` - Unique request identifier.
/// * `method` - The HTTP method.
/// * `route` - The request path.
/// * `status_code` - The HTTP response status.
/// * `duration_ms` - Request processing time in milliseconds.
#[cfg(feature = "axum")]
#[allow(dead_code)]
pub(super) fn log_compact_http(
    id: Uuid,
    method: &Method,
    route: &str,
    status_code: StatusCode,
    duration_ms: u128,
) {
    colorize_print(format!("[{}] ", id), Colors::CyanFg);
    colorize_print(format!("{} {} -> ", method, route), Colors::BlueFg);

    match classify_status(status_code) {
        StatusClass::Success => colorize_print(status_code.to_string(), Colors::GreenFg),
        StatusClass::ClientError => colorize_print(status_code.to_string(), Colors::YellowFg),
        StatusClass::ServerError => colorize_print(status_code.to_string(), Colors::RedFg),
        StatusClass::Other => colorize_print(status_code.to_string(), Colors::MagentaFg),
    }

    colorize_println(format!(" ({} ms)", duration_ms), Colors::BlueFg);
}

/// Extracts the path suffix after `src/`, returning an empty string if not found.
///
/// # Arguments
///
/// * `path` - An optional full file path.
///
/// # Returns
///
/// The portion of the path after `src/`, or an empty [`String`] if
/// `path` is `None` or does not contain `src/`.
pub(super) fn extract_after_src(path: Option<&str>) -> String {
    match path {
        Some(path) => {
            let src_prefix = "src/";

            if let Some(start_index) = path.find(src_prefix) {
                let start_index = start_index + src_prefix.len();
                path[start_index..].to_string()
            } else {
                String::new()
            }
        }
        None => String::new(),
    }
}

/// Prints a verbose error record with file path, line number, and red coloring.
///
/// # Arguments
///
/// * `record` - The log record to print.
pub(super) fn log_error(record: &Record<'_>) {
    let hashtags = get_hashtags(6);
    let error_header = format!("{} Error {}", hashtags, hashtags);
    let file_path = extract_after_src(record.file());
    let line_number = record
        .line()
        .map(|line| line.to_string())
        .unwrap_or_default();

    println!();
    colorize_println(error_header, Colors::RedFg);
    colorize_println(format!("File: {}", file_path), Colors::RedFg);
    colorize_println(format!("Line Number: {}", line_number), Colors::RedFg);
    println!();
    colorize_println(format!("{}", record.args()), Colors::RedFg);
}

/// Prints a verbose debug record with file path, line number, and yellow coloring.
///
/// # Arguments
///
/// * `record` - The log record to print.
pub(super) fn log_debug(record: &Record<'_>) {
    let hashtags = get_hashtags(6);
    let debug_header = format!("{} Debug {}", hashtags, hashtags);
    let file_path = extract_after_src(record.file());
    let line_number = record
        .line()
        .map(|line| line.to_string())
        .unwrap_or_default();

    println!();
    colorize_println(debug_header, Colors::YellowFg);
    colorize_println(format!("File: {}", file_path), Colors::YellowFg);
    colorize_println(format!("Line Number: {}", line_number), Colors::YellowFg);
    println!();
    colorize_println(format!("{}", record.args()), Colors::YellowFg);
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "axum")]
    use axum::http::StatusCode;

    #[cfg(feature = "axum")]
    use super::{StatusClass, classify_status};
    use super::{extract_after_src, get_hashtags, get_spaces};

    #[cfg(feature = "axum")]
    #[test]
    fn classify_status_maps_expected_classes() {
        assert_eq!(classify_status(StatusCode::OK), StatusClass::Success);
        assert_eq!(
            classify_status(StatusCode::BAD_REQUEST),
            StatusClass::ClientError
        );
        assert_eq!(
            classify_status(StatusCode::INTERNAL_SERVER_ERROR),
            StatusClass::ServerError
        );
        assert_eq!(
            classify_status(StatusCode::SWITCHING_PROTOCOLS),
            StatusClass::Other
        );
    }

    #[test]
    fn extract_after_src_returns_relative_suffix() {
        let extracted = extract_after_src(Some("/tmp/project/src/core/logger/mod.rs"));
        assert_eq!(extracted, "core/logger/mod.rs");
    }

    #[test]
    fn extract_after_src_returns_empty_when_src_not_found() {
        let extracted = extract_after_src(Some("/tmp/project/core/logger/mod.rs"));
        assert!(extracted.is_empty());
    }

    #[test]
    fn extract_after_src_returns_empty_for_none() {
        let extracted = extract_after_src(None);
        assert!(extracted.is_empty());
    }

    #[test]
    fn banner_and_indent_helpers_are_deterministic() {
        assert_eq!(get_hashtags(0), "");
        assert_eq!(get_hashtags(3), "###");
        assert_eq!(get_spaces(0), "");
        assert_eq!(get_spaces(2), "    ");
    }
}
