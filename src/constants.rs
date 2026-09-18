//! Public configuration names and defaults.

/// Environment variable containing the API key.
pub const API_KEY_ENV: &str = "TYPESAFE_API_KEY";
/// Environment variable overriding the API base URL.
pub const BASE_URL_ENV: &str = "TYPESAFE_BASE_URL";
/// Environment variable overriding the default model.
pub const DEFAULT_MODEL_ENV: &str = "TYPESAFE_DEFAULT_MODEL";
/// Default API root.
pub const DEFAULT_BASE_URL: &str = "https://api.typesafe.ai";
/// Default model alias.
pub const DEFAULT_MODEL: &str = "jev-latest";
/// Default timeout for each HTTP attempt.
pub const DEFAULT_TIMEOUT_SECS: u64 = 10;

pub(crate) const SYSTEM_ONE_PATH: &str = "/v1/systemone";
pub(crate) const MODELS_PATH: &str = "/v1/models";
pub(crate) const SDK_NAME: &str = "typesafe-sdk";
pub(crate) const SDK_HEADER: &str = "x-typesafe-sdk";
pub(crate) const RUNTIME_HEADER: &str = "x-typesafe-runtime";
pub(crate) const RETRY_COUNT_HEADER: &str = "x-typesafe-retry-count";
pub(crate) const REQUEST_ID_HEADER: &str = "x-typesafe-request-id";
pub(crate) const RETRY_AFTER_MS_HEADER: &str = "retry-after-ms";
pub(crate) const MAX_ERROR_BODY_LENGTH: usize = 200;
