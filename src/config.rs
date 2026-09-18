use std::time::Duration;

use http::HeaderMap;

use crate::constants::{
    API_KEY_ENV, BASE_URL_ENV, DEFAULT_BASE_URL, DEFAULT_MODEL, DEFAULT_MODEL_ENV,
    DEFAULT_TIMEOUT_SECS,
};
use crate::{Error, RetryPolicy};

#[derive(Clone)]
pub(crate) struct Config {
    pub api_key: String,
    pub base_url: String,
    pub default_model: String,
    pub timeout: Duration,
    pub default_headers: HeaderMap,
}

impl Config {
    pub fn resolve(
        api_key: Option<String>,
        base_url: Option<String>,
        model: Option<String>,
        timeout: Option<Duration>,
        default_headers: HeaderMap,
        retry: &RetryPolicy,
    ) -> Result<Self, Error> {
        let api_key = resolve_env(api_key, API_KEY_ENV, None).ok_or_else(|| {
            Error::configuration(format!(
                "No API key was provided. Pass api_key or set the {API_KEY_ENV} environment variable."
            ))
        })?;
        let base_url = resolve_env(base_url, BASE_URL_ENV, Some(DEFAULT_BASE_URL.to_owned()))
            .expect("base URL default exists")
            .trim_end_matches('/')
            .to_owned();
        if base_url.is_empty() {
            return Err(Error::configuration("base_url must not be empty."));
        }
        url::Url::parse(&base_url)
            .map_err(|error| Error::configuration(format!("invalid base_url: {error}")))?;
        let default_model = resolve_env(model, DEFAULT_MODEL_ENV, Some(DEFAULT_MODEL.to_owned()))
            .expect("model default exists");
        let timeout = timeout.unwrap_or(Duration::from_secs(DEFAULT_TIMEOUT_SECS));
        if timeout.is_zero() {
            return Err(Error::configuration(
                "timeout must be a positive number of seconds.",
            ));
        }
        retry.validate()?;
        Ok(Self {
            api_key,
            base_url,
            default_model,
            timeout,
            default_headers,
        })
    }
}

fn resolve_env(value: Option<String>, name: &str, default: Option<String>) -> Option<String> {
    if value.is_some() {
        return value;
    }
    std::env::var(name)
        .ok()
        .and_then(|value| {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_owned())
        })
        .or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_configuration_is_preserved_and_normalized() {
        let config = Config::resolve(
            Some(" key ".to_owned()),
            Some("https://example.test/prefix///".to_owned()),
            Some("model".to_owned()),
            Some(Duration::from_secs(2)),
            HeaderMap::new(),
            &RetryPolicy::disabled(),
        )
        .unwrap();
        assert_eq!(config.api_key, " key ");
        assert_eq!(config.base_url, "https://example.test/prefix");
        assert_eq!(config.default_model, "model");
        assert_eq!(config.timeout, Duration::from_secs(2));
    }

    #[test]
    fn invalid_base_url_and_timeout_are_rejected() {
        assert!(
            Config::resolve(
                Some("key".to_owned()),
                Some("not a URL".to_owned()),
                None,
                None,
                HeaderMap::new(),
                &RetryPolicy::default(),
            )
            .is_err()
        );
        assert!(
            Config::resolve(
                Some("key".to_owned()),
                Some("https://example.test".to_owned()),
                None,
                Some(Duration::ZERO),
                HeaderMap::new(),
                &RetryPolicy::default(),
            )
            .is_err()
        );
    }
}
