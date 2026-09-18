use std::fmt;
use std::time::{Duration, SystemTime};

use http::HeaderMap;
use serde_json::Value;
use thiserror::Error as ThisError;

use crate::constants::{MAX_ERROR_BODY_LENGTH, REQUEST_ID_HEADER, RETRY_AFTER_MS_HEADER};

/// A configuration, input, transport, or API failure.
#[derive(Debug, ThisError)]
pub enum Error {
    /// Invalid or missing SDK configuration.
    #[error("{0}")]
    Configuration(String),
    /// Invalid public input.
    #[error("{0}")]
    Validation(String),
    /// A request body could not be encoded.
    #[error("The request body could not be encoded as JSON: {message}")]
    Serialization {
        /// Serialization failure detail.
        message: String,
    },
    /// The request did not receive an HTTP response.
    #[error("Connection error: {message}")]
    Connection {
        /// Transport failure detail.
        message: String,
    },
    /// The request exceeded its per-attempt timeout.
    #[error("Request timed out (timeout={timeout:?}).")]
    Timeout {
        /// Timeout applied to the attempt.
        timeout: Duration,
    },
    /// The API returned an unsuccessful response, or a successful body was invalid.
    #[error(transparent)]
    Api(Box<ApiError>),
}

impl Error {
    pub(crate) fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration(message.into())
    }

    pub(crate) fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    /// Returns API response details when this is an API error.
    pub fn api(&self) -> Option<&ApiError> {
        match self {
            Self::Api(error) => Some(error),
            _ => None,
        }
    }
}

/// Classification of an API error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiErrorKind {
    /// HTTP 400.
    BadRequest,
    /// HTTP 401.
    Authentication,
    /// HTTP 403.
    PermissionDenied,
    /// HTTP 404.
    NotFound,
    /// HTTP 422.
    UnprocessableEntity,
    /// HTTP 429.
    RateLimited,
    /// HTTP 5xx.
    Internal,
    /// A successful response body did not match the required schema.
    ResponseValidation,
    /// Another unsuccessful HTTP status.
    Other,
}

impl ApiErrorKind {
    pub(crate) fn from_status(status: u16) -> Self {
        match status {
            400 => Self::BadRequest,
            401 => Self::Authentication,
            403 => Self::PermissionDenied,
            404 => Self::NotFound,
            422 => Self::UnprocessableEntity,
            429 => Self::RateLimited,
            500..=599 => Self::Internal,
            _ => Self::Other,
        }
    }
}

/// Parsed representation of an HTTP error body.
#[derive(Clone, Debug, PartialEq)]
pub enum ErrorBody {
    /// Valid JSON content.
    Json(Value),
    /// Non-JSON text, decoded lossily as UTF-8.
    Text(String),
}

/// Details retained from an unsuccessful or structurally invalid HTTP response.
#[derive(Clone, Debug)]
pub struct ApiError {
    /// HTTP status code.
    pub status: u16,
    /// Error classification.
    pub kind: ApiErrorKind,
    /// Parsed response body, or `None` for an empty body/JSON null.
    pub body: Option<ErrorBody>,
    /// Response headers.
    pub headers: HeaderMap,
    /// Sanitized `METHOD URL`, without credentials, query, or fragment.
    pub endpoint: Option<String>,
    /// Dotted response field path when validation failed.
    pub field_path: Option<String>,
    /// Server-requested retry delay.
    pub retry_after: Option<Duration>,
    message: String,
}

impl ApiError {
    /// Returns the TypeSafe request ID, when present and valid UTF-8.
    pub fn request_id(&self) -> Option<&str> {
        header_str(&self.headers, REQUEST_ID_HEADER)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(endpoint) = &self.endpoint {
            write!(f, "{endpoint}: ")?;
        }
        write!(f, "{} {}", self.status, self.message)?;
        if let Some(request_id) = self.request_id() {
            write!(f, " (request_id={request_id})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}

pub(crate) fn deserialize_body(content: &[u8]) -> Option<ErrorBody> {
    if content.is_empty() {
        return None;
    }
    match serde_json::from_slice::<Value>(content) {
        Ok(Value::Null) => None,
        Ok(value) => Some(ErrorBody::Json(value)),
        Err(_) => Some(ErrorBody::Text(
            String::from_utf8_lossy(content).into_owned(),
        )),
    }
}

pub(crate) fn api_error(
    status: u16,
    body: &[u8],
    headers: HeaderMap,
    endpoint: Option<String>,
) -> Error {
    let body = deserialize_body(body);
    let message = error_message(&body);
    let retry_after = parse_retry_after(&headers);
    Error::Api(Box::new(ApiError {
        status,
        kind: ApiErrorKind::from_status(status),
        body,
        headers,
        endpoint,
        field_path: None,
        retry_after,
        message,
    }))
}

pub(crate) fn response_validation_error(
    status: u16,
    body: &[u8],
    headers: HeaderMap,
    endpoint: Option<String>,
    field_path: impl Into<String>,
) -> Error {
    let field_path = field_path.into();
    Error::Api(Box::new(ApiError {
        status,
        kind: ApiErrorKind::ResponseValidation,
        body: deserialize_body(body),
        headers,
        endpoint,
        field_path: Some(field_path.clone()),
        retry_after: None,
        message: format!("Invalid response data at '{field_path}'."),
    }))
}

fn error_message(body: &Option<ErrorBody>) -> String {
    match body {
        None => "status code (no body)".to_owned(),
        Some(ErrorBody::Text(text)) => truncate(text),
        Some(ErrorBody::Json(value)) => extract_message(value)
            .filter(|message| !message.is_empty())
            .unwrap_or_else(|| truncate(&value.to_string())),
    }
}

pub(crate) fn extract_message(body: &Value) -> Option<String> {
    if let Value::String(message) = body {
        return (!message.is_empty()).then(|| message.clone());
    }
    let object = body.as_object()?;
    match object.get("error") {
        Some(Value::String(message)) => return Some(message.clone()),
        Some(Value::Object(error)) => {
            if let Some(Value::String(message)) = error.get("message") {
                return Some(message.clone());
            }
        }
        _ => {}
    }
    if let Some(Value::String(message)) = object.get("message") {
        return Some(message.clone());
    }
    match object.get("detail") {
        Some(Value::String(message)) => Some(message.clone()),
        Some(Value::Object(detail)) => detail
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_owned),
        Some(Value::Array(entries)) => {
            let parts = entries
                .iter()
                .filter_map(|entry| {
                    let entry = entry.as_object()?;
                    let message = entry.get("msg")?.as_str()?;
                    let path = entry
                        .get("loc")
                        .and_then(Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|item| match item {
                                    Value::String(value) if value == "body" => None,
                                    Value::String(value) => Some(value.clone()),
                                    Value::Number(value) => Some(value.to_string()),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join(".")
                        })
                        .unwrap_or_default();
                    Some(if path.is_empty() {
                        message.to_owned()
                    } else {
                        format!("{path}: {message}")
                    })
                })
                .collect::<Vec<_>>();
            (!parts.is_empty()).then(|| parts.join("; "))
        }
        _ => None,
    }
}

fn truncate(value: &str) -> String {
    if value.chars().count() <= MAX_ERROR_BODY_LENGTH {
        return value.to_owned();
    }
    format!(
        "{}…",
        value
            .chars()
            .take(MAX_ERROR_BODY_LENGTH)
            .collect::<String>()
    )
}

pub(crate) fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    if let Some(raw) = header_str(headers, RETRY_AFTER_MS_HEADER)
        && let Some(duration) = numeric_delay(raw, 0.001, true)
    {
        return Some(duration);
    }
    let raw = header_str(headers, http::header::RETRY_AFTER.as_str())?;
    if let Some(duration) = numeric_delay(raw, 1.0, true) {
        return Some(duration);
    }
    let date = httpdate::parse_http_date(raw).ok()?;
    Some(date.duration_since(SystemTime::now()).unwrap_or_default())
}

fn numeric_delay(raw: &str, multiplier: f64, empty_is_zero: bool) -> Option<Duration> {
    let trimmed = raw.trim();
    let value = if trimmed.is_empty() && empty_is_zero {
        0.0
    } else {
        trimmed.parse::<f64>().ok()?
    };
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    Duration::try_from_secs_f64(value * multiplier).ok()
}

pub(crate) fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

#[cfg(test)]
mod tests {
    use http::HeaderValue;
    use serde_json::json;

    use super::*;

    #[test]
    fn extracts_supported_message_shapes() {
        assert_eq!(
            extract_message(&json!({"error": "bad"})).as_deref(),
            Some("bad")
        );
        assert_eq!(
            extract_message(
                &json!({"detail": [{"loc": ["body", "questions", 0], "msg": "required"}]})
            )
            .as_deref(),
            Some("questions.0: required")
        );
    }

    #[test]
    fn retry_after_ms_wins() {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER_MS_HEADER, HeaderValue::from_static("250"));
        headers.insert(http::header::RETRY_AFTER, HeaderValue::from_static("10"));
        assert_eq!(
            parse_retry_after(&headers),
            Some(Duration::from_millis(250))
        );
    }

    #[test]
    fn empty_retry_after_is_immediate() {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER_MS_HEADER, HeaderValue::from_static(""));
        assert_eq!(parse_retry_after(&headers), Some(Duration::ZERO));
    }
}
