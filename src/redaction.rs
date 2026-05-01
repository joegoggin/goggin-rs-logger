//! Sensitive value redaction for JSON bodies and HTTP headers.
//!
//! Replaces values of keys that match known sensitive patterns (passwords,
//! tokens, secrets, API keys) with `"(hidden)"` before they reach log output.

use serde_json::{Value, from_slice};

/// Parses bytes as JSON and redacts all sensitive fields, returning `None` for empty or invalid input.
///
/// # Arguments
///
/// * `bytes` - The raw byte slice to parse.
///
/// # Returns
///
/// The redacted JSON [`Value`], or `None` if `bytes` is empty or
/// not valid JSON.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn parse_redacted_json(bytes: &[u8]) -> Option<Value> {
    if bytes.is_empty() {
        return None;
    }

    let mut value = from_slice::<Value>(bytes).ok()?;
    redact_json_value(&mut value);

    Some(value)
}

/// Recursively replaces values of sensitive keys with `"(hidden)"`.
///
/// # Arguments
///
/// * `value` - The JSON value to redact in place.
pub(super) fn redact_json_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                if is_sensitive_json_key(key) {
                    *value = Value::String("(hidden)".to_string());
                } else {
                    redact_json_value(value);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_json_value(item);
            }
        }
        _ => {}
    }
}

/// Returns `true` if the JSON key matches a sensitive pattern (case-insensitive).
///
/// # Arguments
///
/// * `key` - The JSON field name to check.
///
/// # Returns
///
/// `true` if the key matches a known sensitive pattern.
pub(super) fn is_sensitive_json_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();

    matches!(
        key.as_str(),
        "password"
            | "confirm"
            | "token"
            | "access_token"
            | "refresh_token"
            | "authorization"
            | "auth"
            | "auth_code"
            | "jwt"
            | "session"
            | "session_id"
            | "cookie"
            | "set_cookie"
            | "secret"
            | "api_key"
    ) || key.contains("token")
        || key.contains("authorization")
        || key.contains("secret")
        || key.contains("session")
        || key.contains("cookie")
        || key.contains("jwt")
        || key.contains("password")
}

/// Returns `"(hidden)"` for sensitive headers, or the original value otherwise.
///
/// # Arguments
///
/// * `name` - The header name.
/// * `value` - The header value.
///
/// # Returns
///
/// `"(hidden)"` if the header is sensitive, or the original `value`.
pub(super) fn sanitize_header_value(name: &str, value: &str) -> String {
    if is_sensitive_header(name) {
        "(hidden)".to_string()
    } else {
        value.to_string()
    }
}

/// Returns `true` if the header name matches a sensitive pattern (case-insensitive).
///
/// # Arguments
///
/// * `name` - The header name to check.
///
/// # Returns
///
/// `true` if the header name matches a known sensitive pattern.
pub(super) fn is_sensitive_header(name: &str) -> bool {
    let name = name.to_ascii_lowercase();

    matches!(
        name.as_str(),
        "authorization" | "cookie" | "set-cookie" | "x-api-key" | "x-auth-token"
    ) || name.contains("token")
        || name.contains("secret")
        || name.contains("session")
        || name.contains("jwt")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{parse_redacted_json, redact_json_value, sanitize_header_value};

    #[test]
    fn redact_json_masks_sensitive_keys() {
        let mut value = json!({
            "password": "secret",
            "nested": {
                "auth_code": "123456",
                "name": "Taylor"
            }
        });

        redact_json_value(&mut value);

        assert_eq!(value["password"], "(hidden)");
        assert_eq!(value["nested"]["auth_code"], "(hidden)");
        assert_eq!(value["nested"]["name"], "Taylor");
    }

    #[test]
    fn redact_json_masks_case_insensitive_and_substring_keys() {
        let mut value = json!({
            "Access_Token": "abc",
            "Authorization": "Bearer abc",
            "session_id": "session-1",
            "cookie": "sid=session-1",
            "jwt": "ey...",
            "dbPasswordHash": "hashed",
            "nested": {
                "clientSecret": "top-secret",
                "safe": "visible"
            },
            "items": [
                {
                    "refresh_token": "token-1"
                },
                {
                    "note": "still-visible"
                }
            ]
        });

        redact_json_value(&mut value);

        assert_eq!(value["Access_Token"], "(hidden)");
        assert_eq!(value["Authorization"], "(hidden)");
        assert_eq!(value["session_id"], "(hidden)");
        assert_eq!(value["cookie"], "(hidden)");
        assert_eq!(value["jwt"], "(hidden)");
        assert_eq!(value["dbPasswordHash"], "(hidden)");
        assert_eq!(value["nested"]["clientSecret"], "(hidden)");
        assert_eq!(value["nested"]["safe"], "visible");
        assert_eq!(value["items"][0]["refresh_token"], "(hidden)");
        assert_eq!(value["items"][1]["note"], "still-visible");
    }

    #[test]
    fn sanitize_header_value_masks_sensitive_headers() {
        assert_eq!(
            sanitize_header_value("Authorization", "Bearer abc"),
            "(hidden)"
        );
        assert_eq!(sanitize_header_value("X-Auth-Token", "abc"), "(hidden)");
        assert_eq!(sanitize_header_value("X-Session-Id", "abc"), "(hidden)");
        assert_eq!(sanitize_header_value("X-JWT", "abc"), "(hidden)");
        assert_eq!(
            sanitize_header_value("X-Client-Secret-Key", "xyz"),
            "(hidden)"
        );
        assert_eq!(sanitize_header_value("X-Request-Id", "123"), "123");
    }

    #[test]
    fn parse_redacted_json_returns_none_for_invalid_json() {
        let value = parse_redacted_json(b"not-json");
        assert!(value.is_none());
    }

    #[test]
    fn parse_redacted_json_redacts_sensitive_fields() {
        let value = parse_redacted_json(br#"{"password":"abc","name":"Taylor"}"#)
            .expect("expected valid JSON");

        assert_eq!(value["password"], "(hidden)");
        assert_eq!(value["name"], "Taylor");
    }
}
