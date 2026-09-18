//! Optional blocking TypeSafe client.

use std::fmt;
use std::time::{Duration, Instant};

use http::{HeaderMap, HeaderName, HeaderValue};
use reqwest::blocking::Client as HttpClient;
use serde_json::{Map, Value};

use crate::client::{
    ModelsOptions, SystemOneOptions, build_models_request, build_system_one_request,
};
use crate::config::Config;
use crate::error::api_error;
use crate::response::{decode_models, decode_system_one};
use crate::transport::{
    PreparedRequest, endpoint, from_reqwest_error, raw_response, redacted_headers, set_retry_count,
};
use crate::{
    Error, IntoState, JsonContent, ListModelsResponse, Question, Questions, RawResponse,
    RetryPolicy, SystemOneResponse,
};

/// Blocking TypeSafe client.
#[derive(Clone)]
pub struct Client {
    config: Config,
    http: HttpClient,
    retry: RetryPolicy,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("blocking::Client")
            .field("base_url", &self.config.base_url)
            .field("default_model", &self.config.default_model)
            .field("timeout", &self.config.timeout)
            .field("retry", &self.retry)
            .finish_non_exhaustive()
    }
}

/// Builder for a blocking [`Client`].
#[derive(Clone, Default)]
pub struct ClientBuilder {
    api_key: Option<String>,
    model: Option<String>,
    retry: Option<RetryPolicy>,
    timeout: Option<Duration>,
    headers: HeaderMap,
    base_url: Option<String>,
    http: Option<HttpClient>,
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("blocking::ClientBuilder")
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("model", &self.model)
            .field("retry", &self.retry)
            .field("timeout", &self.timeout)
            .field("headers", &redacted_headers(&self.headers))
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl ClientBuilder {
    /// Sets the API key.
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Sets the default model.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets the retry policy.
    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = Some(retry);
        self
    }

    /// Sets the per-attempt timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Adds a default header.
    pub fn header(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Result<Self, Error> {
        let name = HeaderName::from_bytes(name.as_ref().as_bytes())
            .map_err(|error| Error::configuration(format!("invalid header name: {error}")))?;
        let value = HeaderValue::from_str(value.as_ref())
            .map_err(|error| Error::configuration(format!("invalid header value: {error}")))?;
        self.headers.insert(name, value);
        Ok(self)
    }

    /// Replaces all default headers.
    pub fn headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    /// Sets the API root.
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Uses an existing blocking Reqwest client.
    pub fn http_client(mut self, http: HttpClient) -> Self {
        self.http = Some(http);
        self
    }

    /// Validates configuration and builds the client.
    pub fn build(self) -> Result<Client, Error> {
        let retry = self.retry.unwrap_or_default();
        let config = Config::resolve(
            self.api_key,
            self.base_url,
            self.model,
            self.timeout,
            self.headers,
            &retry,
        )?;
        let http = match self.http {
            Some(http) => http,
            None => HttpClient::builder()
                .build()
                .map_err(|error| Error::Connection {
                    message: error.without_url().to_string(),
                })?,
        };
        Ok(Client {
            config,
            http,
            retry,
        })
    }
}

/// Fluent builder for one blocking System One request.
#[must_use = "a System One request is not sent until send() is called"]
pub struct SystemOneRequestBuilder<'a> {
    client: &'a Client,
    state: Result<JsonContent, Error>,
    questions: Questions,
    options: SystemOneOptions,
}

impl fmt::Debug for SystemOneRequestBuilder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("blocking::SystemOneRequestBuilder")
            .field("state", &self.state)
            .field("questions", &self.questions)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

impl<'a> SystemOneRequestBuilder<'a> {
    /// Adds a named question, replacing an earlier question with the same name.
    pub fn question(mut self, name: impl Into<String>, question: impl Into<Question>) -> Self {
        self.questions.insert(name, question);
        self
    }

    /// Adds a reusable question collection. Incoming names replace existing ones.
    pub fn questions(mut self, questions: Questions) -> Self {
        self.questions.extend(questions);
        self
    }

    /// Overrides the client default model.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.options.model = Some(model.into());
        self
    }

    /// Replaces the client retry policy for this request.
    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.options.retry = Some(retry);
        self
    }

    /// Overrides the per-attempt timeout for this request.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.options.timeout = Some(timeout);
        self
    }

    /// Replaces request-specific headers.
    pub fn headers(mut self, headers: HeaderMap) -> Self {
        self.options.extra_headers = headers;
        self
    }

    /// Adds a top-level request body field. Later values win, including core fields.
    pub fn extra_body(mut self, name: impl Into<String>, value: Value) -> Self {
        self.options.extra_body.insert(name.into(), value);
        self
    }

    /// Adds top-level request body fields. Incoming values win.
    pub fn extra_body_map(mut self, body: Map<String, Value>) -> Self {
        self.options.extra_body.extend(body);
        self
    }

    /// Validates, sends, and decodes the request.
    pub fn send(self) -> Result<SystemOneResponse, Error> {
        let state = self.state?;
        let (request, retry) = build_system_one_request(
            &self.client.config,
            &self.client.retry,
            state,
            self.questions,
            self.options,
        )?;
        decode_system_one(self.client.send(request, &retry)?)
    }
}

impl Client {
    /// Returns a blocking-client builder.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Builds a blocking client with an explicit API key.
    pub fn new(api_key: impl Into<String>) -> Result<Self, Error> {
        Self::builder().api_key(api_key).build()
    }

    /// Builds a blocking client from environment configuration.
    pub fn from_env() -> Result<Self, Error> {
        Self::builder().build()
    }

    /// Starts a fluent blocking System One request.
    pub fn system_one<S>(&self, state: S) -> SystemOneRequestBuilder<'_>
    where
        S: IntoState,
    {
        SystemOneRequestBuilder {
            client: self,
            state: state.into_state(),
            questions: Questions::new(),
            options: SystemOneOptions::default(),
        }
    }

    /// Lists models using client defaults.
    pub fn models(&self) -> Result<ListModelsResponse, Error> {
        self.models_with_options(ModelsOptions::default())
    }

    /// Lists models with per-call overrides.
    pub fn models_with_options(&self, options: ModelsOptions) -> Result<ListModelsResponse, Error> {
        let (request, retry) = build_models_request(&self.config, &self.retry, options)?;
        decode_models(self.send(request, &retry)?)
    }

    fn send(&self, request: PreparedRequest, retry: &RetryPolicy) -> Result<RawResponse, Error> {
        let started = Instant::now();
        let mut retry_count = 0;
        loop {
            let mut headers = request.headers.clone();
            set_retry_count(&mut headers, retry_count);
            let result = self.execute(&request, headers).and_then(|raw| {
                if raw.status.is_success() {
                    Ok(raw)
                } else {
                    let endpoint = raw.endpoint().to_owned();
                    Err(api_error(
                        raw.status.as_u16(),
                        &raw.body,
                        raw.headers,
                        Some(endpoint),
                    ))
                }
            });
            match result {
                Ok(raw) => return Ok(raw),
                Err(error) if retry_count < retry.max_retries && retry.retryable(&error) => {
                    let wait = retry.wait(retry_count + 1, &error);
                    if retry
                        .timeout
                        .is_some_and(|budget| started.elapsed() + wait >= budget)
                    {
                        return Err(error);
                    }
                    retry_count += 1;
                    tracing::info!(
                        target: "typesafe_sdk",
                        method = %request.method,
                        url = %request.url,
                        retry = retry_count,
                        "retrying blocking request"
                    );
                    std::thread::sleep(wait);
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn execute(&self, request: &PreparedRequest, headers: HeaderMap) -> Result<RawResponse, Error> {
        let endpoint = endpoint(&request.method, &request.url);
        tracing::debug!(
            target: "typesafe_sdk",
            method = %request.method,
            url = %request.url,
            headers = ?redacted_headers(&headers),
            body = ?request.body,
            "sending blocking request"
        );
        let started = Instant::now();
        let mut builder = self
            .http
            .request(request.method.clone(), request.url.clone())
            .headers(headers)
            .timeout(request.timeout);
        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }
        let response = builder
            .send()
            .map_err(|error| from_reqwest_error(error, request.timeout))?;
        let status = response.status();
        let headers = response.headers().clone();
        let body = response
            .bytes()
            .map_err(|error| from_reqwest_error(error, request.timeout))?;
        tracing::info!(
            target: "typesafe_sdk",
            method = %request.method,
            url = %request.url,
            status = status.as_u16(),
            elapsed_ms = started.elapsed().as_millis(),
            "blocking request completed"
        );
        Ok(raw_response(status, headers, body, endpoint))
    }
}
