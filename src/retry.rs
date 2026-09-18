use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::Error;

/// HTTP statuses retried by a policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RetryStatuses {
    /// HTTP 408, 429, and every 5xx response.
    Default,
    /// An exact custom status set.
    Custom(HashSet<u16>),
}

impl RetryStatuses {
    pub(crate) fn contains(&self, status: u16) -> bool {
        match self {
            Self::Default => matches!(status, 408 | 429 | 500..=599),
            Self::Custom(statuses) => statuses.contains(&status),
        }
    }
}

/// Thread-safe callback that can opt additional errors into retry behavior.
pub type RetryPredicate = Arc<dyn Fn(&Error) -> bool + Send + Sync>;

/// Retry behavior for a client or individual call.
#[derive(Clone)]
pub struct RetryPolicy {
    /// Retries after the initial attempt.
    pub max_retries: u32,
    /// Initial exponential-backoff delay.
    pub backoff_initial: Duration,
    /// Maximum exponential-backoff delay.
    pub backoff_max: Duration,
    /// Fraction randomly subtracted from each delay, in `0.0..=1.0`.
    pub backoff_jitter: f64,
    /// HTTP statuses that trigger retries.
    pub http_statuses: RetryStatuses,
    /// Whether server retry headers override backoff.
    pub respect_retry_after: bool,
    /// Whether connection failures are retried.
    pub api_connection_error: bool,
    /// Whether per-attempt timeouts are retried.
    pub api_timeout_error: bool,
    /// Total retry budget, including attempts and delays; `None` disables it.
    pub timeout: Option<Duration>,
    /// Optional callback evaluated after built-in retry conditions.
    pub predicate: Option<RetryPredicate>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            backoff_initial: Duration::from_millis(500),
            backoff_max: Duration::from_secs(5),
            backoff_jitter: 0.25,
            http_statuses: RetryStatuses::Default,
            respect_retry_after: true,
            api_connection_error: true,
            api_timeout_error: true,
            timeout: Some(Duration::from_secs(30)),
            predicate: None,
        }
    }
}

impl fmt::Debug for RetryPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RetryPolicy")
            .field("max_retries", &self.max_retries)
            .field("backoff_initial", &self.backoff_initial)
            .field("backoff_max", &self.backoff_max)
            .field("backoff_jitter", &self.backoff_jitter)
            .field("http_statuses", &self.http_statuses)
            .field("respect_retry_after", &self.respect_retry_after)
            .field("api_connection_error", &self.api_connection_error)
            .field("api_timeout_error", &self.api_timeout_error)
            .field("timeout", &self.timeout)
            .field("predicate", &self.predicate.as_ref().map(|_| "<predicate>"))
            .finish()
    }
}

impl RetryPolicy {
    /// Returns a policy that never retries.
    pub fn disabled() -> Self {
        Self {
            max_retries: 0,
            timeout: None,
            ..Self::default()
        }
    }

    /// Adds a custom retry predicate in addition to built-in conditions.
    pub fn with_predicate(
        mut self,
        predicate: impl Fn(&Error) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.predicate = Some(Arc::new(predicate));
        self
    }

    pub(crate) fn validate(&self) -> Result<(), Error> {
        if !self.backoff_jitter.is_finite() || !(0.0..=1.0).contains(&self.backoff_jitter) {
            return Err(Error::configuration(
                "backoff_jitter must be between zero and one.",
            ));
        }
        if self.timeout.is_some_and(|timeout| timeout.is_zero()) {
            return Err(Error::configuration("retry timeout must be positive."));
        }
        Ok(())
    }

    pub(crate) fn retryable(&self, error: &Error) -> bool {
        let built_in = match error {
            Error::Timeout { .. } => self.api_timeout_error,
            Error::Connection { .. } => self.api_connection_error,
            Error::Api(api) => self.http_statuses.contains(api.status),
            Error::Configuration(_) | Error::Validation(_) | Error::Serialization { .. } => false,
        };
        built_in
            || self
                .predicate
                .as_ref()
                .is_some_and(|predicate| predicate(error))
    }

    pub(crate) fn wait(&self, attempt: u32, error: &Error) -> Duration {
        if self.respect_retry_after
            && let Error::Api(api) = error
            && let Some(delay) = api.retry_after
        {
            return delay;
        }
        backoff(
            attempt,
            self.backoff_initial,
            self.backoff_max,
            self.backoff_jitter,
            fastrand::f64(),
        )
    }
}

pub(crate) fn backoff(
    attempt: u32,
    initial: Duration,
    maximum: Duration,
    jitter: f64,
    random: f64,
) -> Duration {
    if initial.is_zero() || maximum.is_zero() {
        return Duration::ZERO;
    }
    let initial = initial.as_secs_f64();
    let maximum = maximum.as_secs_f64();
    let exponent = attempt.saturating_sub(1);
    let exponential = if f64::from(exponent) >= maximum.log2() - initial.log2() {
        maximum
    } else {
        initial * 2f64.powf(f64::from(exponent))
    };
    let seconds =
        exponential.min((exponential * (1.0 - random * jitter) * 1000.0).round() / 1000.0);
    Duration::from_secs_f64(seconds.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_retries_transient_failures() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 2);
        assert!(policy.http_statuses.contains(408));
        assert!(policy.http_statuses.contains(429));
        assert!(policy.http_statuses.contains(599));
        assert!(!policy.http_statuses.contains(404));
    }

    #[test]
    fn backoff_caps_and_subtracts_jitter() {
        assert_eq!(
            backoff(
                1,
                Duration::from_millis(500),
                Duration::from_secs(5),
                0.25,
                0.0
            ),
            Duration::from_millis(500)
        );
        assert_eq!(
            backoff(
                20,
                Duration::from_millis(500),
                Duration::from_secs(5),
                0.25,
                1.0
            ),
            Duration::from_millis(3750)
        );
    }

    #[test]
    fn custom_predicate_extends_retryability() {
        let policy =
            RetryPolicy::disabled().with_predicate(|error| matches!(error, Error::Validation(_)));
        assert!(policy.retryable(&Error::validation("retry me")));
        assert!(!policy.retryable(&Error::configuration("do not retry")));
    }
}
