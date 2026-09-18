use std::time::Duration;

use bytes::Bytes;
use http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode};
use serde_json::Value;
use url::Url;

use crate::config::Config;
use crate::constants::{RETRY_COUNT_HEADER, RUNTIME_HEADER, SDK_HEADER, SDK_NAME};
use crate::{Error, RawResponse};

#[derive(Clone, Debug)]
pub(crate) struct PreparedRequest {
    pub method: Method,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Option<Bytes>,
    pub timeout: Duration,
}

pub(crate) fn prepare(
    config: &Config,
    method: Method,
    path: &str,
    body: Option<&Value>,
    timeout: Option<Duration>,
    extra_headers: &HeaderMap,
) -> Result<PreparedRequest, Error> {
    let timeout = timeout.unwrap_or(config.timeout);
    if timeout.is_zero() {
        return Err(Error::configuration(
            "timeout must be a positive number of seconds.",
        ));
    }
    let url = Url::parse(&format!("{}{path}", config.base_url))
        .map_err(|error| Error::configuration(format!("invalid request URL: {error}")))?;
    let mut headers = config.default_headers.clone();
    headers.extend(extra_headers.clone());
    headers.remove(RETRY_COUNT_HEADER);
    let identity = format!("{SDK_NAME}/{}", env!("CARGO_PKG_VERSION"));
    set_header(
        &mut headers,
        AUTHORIZATION,
        &format!("Bearer {}", config.api_key),
    )?;
    set_header(&mut headers, ACCEPT, "application/json")?;
    set_header(&mut headers, USER_AGENT, &identity)?;
    set_header(&mut headers, HeaderName::from_static(SDK_HEADER), &identity)?;
    set_header(
        &mut headers,
        HeaderName::from_static(RUNTIME_HEADER),
        &runtime_header(),
    )?;
    let body = body
        .map(|body| {
            serde_json::to_vec(body)
                .map(Bytes::from)
                .map_err(|error| Error::Serialization {
                    message: error.to_string(),
                })
        })
        .transpose()?;
    if body.is_some() {
        set_header(&mut headers, CONTENT_TYPE, "application/json")?;
    }
    Ok(PreparedRequest {
        method,
        url,
        headers,
        body,
        timeout,
    })
}

fn set_header(headers: &mut HeaderMap, name: HeaderName, value: &str) -> Result<(), Error> {
    let value = HeaderValue::from_str(value)
        .map_err(|error| Error::configuration(format!("invalid header value: {error}")))?;
    headers.insert(name, value);
    Ok(())
}

pub(crate) fn set_retry_count(headers: &mut HeaderMap, retry_count: u32) {
    if retry_count == 0 {
        headers.remove(RETRY_COUNT_HEADER);
    } else if let Ok(value) = HeaderValue::from_str(&retry_count.to_string()) {
        headers.insert(HeaderName::from_static(RETRY_COUNT_HEADER), value);
    }
}

pub(crate) fn endpoint(method: &Method, url: &Url) -> String {
    let mut sanitized = url.clone();
    let _ = sanitized.set_username("");
    let _ = sanitized.set_password(None);
    sanitized.set_query(None);
    sanitized.set_fragment(None);
    format!("{method} {sanitized}")
}

pub(crate) fn from_reqwest_error(error: reqwest::Error, timeout: Duration) -> Error {
    if error.is_timeout() {
        Error::Timeout { timeout }
    } else {
        Error::Connection {
            message: error.without_url().to_string(),
        }
    }
}

pub(crate) fn raw_response(
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
    endpoint: String,
) -> RawResponse {
    RawResponse::new(status, headers, body, endpoint)
}

pub(crate) fn redacted_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            let name = name.as_str().to_owned();
            let lowered = name.to_ascii_lowercase();
            let secret = matches!(
                lowered.as_str(),
                "authorization"
                    | "proxy-authorization"
                    | "x-api-key"
                    | "api-key"
                    | "cookie"
                    | "set-cookie"
            ) || lowered.contains("token")
                || lowered.contains("secret");
            let value = if secret {
                "***".to_owned()
            } else {
                value.to_str().unwrap_or("<non-utf8>").to_owned()
            };
            (name, value)
        })
        .collect()
}

fn runtime_header() -> String {
    format!(
        "rust/{} ({}; {})",
        env!("TYPESAFE_RUSTC_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_removes_credentials_query_and_fragment() {
        let url = Url::parse("https://user:pass@example.test/path?secret=1#fragment").unwrap();
        assert_eq!(
            endpoint(&Method::POST, &url),
            "POST https://example.test/path"
        );
    }

    #[test]
    fn logging_headers_are_redacted() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));
        headers.insert("x-access-token", HeaderValue::from_static("token"));
        headers.insert("x-safe", HeaderValue::from_static("visible"));
        let shown = redacted_headers(&headers);
        assert!(shown.contains(&("authorization".to_owned(), "***".to_owned())));
        assert!(shown.contains(&("x-access-token".to_owned(), "***".to_owned())));
        assert!(shown.contains(&("x-safe".to_owned(), "visible".to_owned())));
    }
}
