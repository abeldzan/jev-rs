use std::collections::BTreeMap;

use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use indexmap::IndexMap;
use serde_json::{Map, Value};

use crate::constants::REQUEST_ID_HEADER;
use crate::error::response_validation_error;
use crate::{Error, JsonContent};

/// Owned HTTP response metadata and bytes retained after decoding.
#[derive(Clone, Debug, PartialEq)]
pub struct RawResponse {
    /// HTTP status.
    pub status: StatusCode,
    /// HTTP response headers.
    pub headers: HeaderMap,
    /// Exact response body bytes.
    pub body: Bytes,
    endpoint: String,
}

impl RawResponse {
    pub(crate) fn new(
        status: StatusCode,
        headers: HeaderMap,
        body: Bytes,
        endpoint: String,
    ) -> Self {
        Self {
            status,
            headers,
            body,
            endpoint,
        }
    }

    /// Returns the sanitized request endpoint.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Returns the TypeSafe request ID when present and valid UTF-8.
    pub fn request_id(&self) -> Option<&str> {
        self.headers.get(REQUEST_ID_HEADER)?.to_str().ok()
    }
}

/// A yes/no answer represented as a probability of yes.
#[derive(Clone, Debug, PartialEq)]
pub struct NoulAnswer {
    /// Probability of a yes answer, from zero to one.
    pub noul: f64,
}

/// A selected label and its probability distribution.
#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceAnswer {
    /// Selected label.
    pub choice: String,
    /// Reported confidence in the selected label.
    pub confidence: f64,
    /// Probabilities keyed by label.
    pub probabilities: IndexMap<String, f64>,
}

/// An expected score, rubric, and probability distribution.
#[derive(Clone, Debug, PartialEq)]
pub struct ScoreAnswer {
    /// Expected score; it may lie between rubric levels.
    pub score: f64,
    /// Reported confidence in the score.
    pub confidence: f64,
    /// Rubric descriptions keyed by integer score.
    pub legend: BTreeMap<u32, JsonContent>,
    /// Probabilities keyed by integer score.
    pub probabilities: BTreeMap<u32, f64>,
}

/// A typed answer to one named question.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    /// Noul answer.
    Noul(NoulAnswer),
    /// Choice answer.
    Choice(ChoiceAnswer),
    /// Score answer.
    Score(ScoreAnswer),
}

impl Answer {
    /// Returns the inner noul answer when types match.
    pub fn as_noul(&self) -> Option<&NoulAnswer> {
        match self {
            Self::Noul(answer) => Some(answer),
            _ => None,
        }
    }

    /// Returns the inner choice answer when types match.
    pub fn as_choice(&self) -> Option<&ChoiceAnswer> {
        match self {
            Self::Choice(answer) => Some(answer),
            _ => None,
        }
    }

    /// Returns the inner score answer when types match.
    pub fn as_score(&self) -> Option<&ScoreAnswer> {
        match self {
            Self::Score(answer) => Some(answer),
            _ => None,
        }
    }
}

/// Token counts reported for a System One request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// Input tokens, when reported.
    pub input_tokens: Option<i64>,
    /// Output tokens, when reported.
    pub output_tokens: Option<i64>,
}

/// System One answers and response metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct SystemOneResponse {
    /// Model that answered the request.
    pub model: String,
    /// Token counts.
    pub usage: Usage,
    /// Typed answers keyed by request question name.
    pub answers: IndexMap<String, Answer>,
    raw: RawResponse,
}

impl SystemOneResponse {
    /// Returns the retained HTTP response.
    pub fn raw_response(&self) -> &RawResponse {
        &self.raw
    }

    /// Returns the TypeSafe request ID, when present.
    pub fn request_id(&self) -> Option<&str> {
        self.raw.request_id()
    }

    /// Gets a named noul answer.
    pub fn noul(&self, name: &str) -> Option<&NoulAnswer> {
        self.answers.get(name).and_then(Answer::as_noul)
    }

    /// Gets a named choice answer.
    pub fn choice(&self, name: &str) -> Option<&ChoiceAnswer> {
        self.answers.get(name).and_then(Answer::as_choice)
    }

    /// Gets a named score answer.
    pub fn score(&self, name: &str) -> Option<&ScoreAnswer> {
        self.answers.get(name).and_then(Answer::as_score)
    }

    /// Iterates over noul answers without allocating a grouped map.
    pub fn nouls(&self) -> impl Iterator<Item = (&str, &NoulAnswer)> {
        self.answers
            .iter()
            .filter_map(|(name, answer)| answer.as_noul().map(|answer| (name.as_str(), answer)))
    }

    /// Iterates over choice answers without allocating a grouped map.
    pub fn choices(&self) -> impl Iterator<Item = (&str, &ChoiceAnswer)> {
        self.answers
            .iter()
            .filter_map(|(name, answer)| answer.as_choice().map(|answer| (name.as_str(), answer)))
    }

    /// Iterates over score answers without allocating a grouped map.
    pub fn scores(&self) -> impl Iterator<Item = (&str, &ScoreAnswer)> {
        self.answers
            .iter()
            .filter_map(|(name, answer)| answer.as_score().map(|answer| (name.as_str(), answer)))
    }
}

/// Metadata for an available TypeSafe model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelMetadata {
    /// Model name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Release date as reported by the API.
    pub release_date: String,
}

/// Response from the models endpoint.
#[derive(Clone, Debug, PartialEq)]
pub struct ListModelsResponse {
    /// Available models.
    pub models: Vec<ModelMetadata>,
    raw: RawResponse,
}

impl ListModelsResponse {
    /// Returns the retained HTTP response.
    pub fn raw_response(&self) -> &RawResponse {
        &self.raw
    }

    /// Returns the TypeSafe request ID, when present.
    pub fn request_id(&self) -> Option<&str> {
        self.raw.request_id()
    }
}

pub(crate) fn decode_system_one(raw: RawResponse) -> Result<SystemOneResponse, Error> {
    let root: Value = serde_json::from_slice(&raw.body).map_err(|_| invalid(&raw, "model"))?;
    let object = root.as_object().ok_or_else(|| invalid(&raw, "model"))?;
    let model = required_str(object, "model", &raw)?.to_owned();
    let usage_value = object.get("usage").ok_or_else(|| invalid(&raw, "usage"))?;
    let usage_object = usage_value
        .as_object()
        .ok_or_else(|| invalid(&raw, "usage"))?;
    let usage = Usage {
        input_tokens: optional_i64(usage_object, "input_tokens", "usage.input_tokens", &raw)?,
        output_tokens: optional_i64(usage_object, "output_tokens", "usage.output_tokens", &raw)?,
    };
    let answers_value = object
        .get("answers")
        .ok_or_else(|| invalid(&raw, "answers"))?;
    let answer_object = answers_value
        .as_object()
        .ok_or_else(|| invalid(&raw, "answers"))?;
    let mut answers = IndexMap::new();
    for (name, value) in answer_object {
        let answer = value
            .as_object()
            .ok_or_else(|| invalid(&raw, format!("answers.{name}")))?;
        let kind = answer
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid(&raw, format!("answers.{name}.type")))?;
        let decoded = match kind {
            "noul" => Answer::Noul(NoulAnswer {
                noul: required_f64(answer, "noul", &format!("answers.{name}.noul"), &raw)?,
            }),
            "choice" => Answer::Choice(ChoiceAnswer {
                choice: required_str_at(answer, "choice", &format!("answers.{name}.choice"), &raw)?
                    .to_owned(),
                confidence: required_f64(
                    answer,
                    "confidence",
                    &format!("answers.{name}.confidence"),
                    &raw,
                )?,
                probabilities: string_probabilities(
                    answer,
                    "probabilities",
                    &format!("answers.{name}.probabilities"),
                    &raw,
                )?,
            }),
            "score" => Answer::Score(ScoreAnswer {
                score: required_f64(answer, "score", &format!("answers.{name}.score"), &raw)?,
                confidence: required_f64(
                    answer,
                    "confidence",
                    &format!("answers.{name}.confidence"),
                    &raw,
                )?,
                legend: score_legend(answer, "legend", &format!("answers.{name}.legend"), &raw)?,
                probabilities: score_probabilities(
                    answer,
                    "probabilities",
                    &format!("answers.{name}.probabilities"),
                    &raw,
                )?,
            }),
            unknown => {
                tracing::warn!(
                    target: "typesafe_sdk",
                    answer = %name,
                    answer_type = %unknown,
                    "ignoring answer with unrecognized type"
                );
                continue;
            }
        };
        answers.insert(name.clone(), decoded);
    }
    Ok(SystemOneResponse {
        model,
        usage,
        answers,
        raw,
    })
}

pub(crate) fn decode_models(raw: RawResponse) -> Result<ListModelsResponse, Error> {
    let root: Value = serde_json::from_slice(&raw.body).map_err(|_| invalid(&raw, "models"))?;
    let items = root
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(&raw, "models"))?;
    let mut models = Vec::with_capacity(items.len());
    for (index, value) in items.iter().enumerate() {
        let object = value
            .as_object()
            .ok_or_else(|| invalid(&raw, format!("models.{index}")))?;
        models.push(ModelMetadata {
            name: required_str_at(object, "name", &format!("models.{index}.name"), &raw)?
                .to_owned(),
            description: required_str_at(
                object,
                "description",
                &format!("models.{index}.description"),
                &raw,
            )?
            .to_owned(),
            release_date: required_str_at(
                object,
                "release_date",
                &format!("models.{index}.release_date"),
                &raw,
            )?
            .to_owned(),
        });
    }
    Ok(ListModelsResponse { models, raw })
}

fn invalid(raw: &RawResponse, path: impl Into<String>) -> Error {
    response_validation_error(
        raw.status.as_u16(),
        &raw.body,
        raw.headers.clone(),
        Some(raw.endpoint.clone()),
        path,
    )
}

fn required_str<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    raw: &RawResponse,
) -> Result<&'a str, Error> {
    required_str_at(object, field, field, raw)
}

fn required_str_at<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    path: &str,
    raw: &RawResponse,
) -> Result<&'a str, Error> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(raw, path))
}

fn required_f64(
    object: &Map<String, Value>,
    field: &str,
    path: &str,
    raw: &RawResponse,
) -> Result<f64, Error> {
    object
        .get(field)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid(raw, path))
}

fn optional_i64(
    object: &Map<String, Value>,
    field: &str,
    path: &str,
    raw: &RawResponse,
) -> Result<Option<i64>, Error> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_i64().map(Some).ok_or_else(|| invalid(raw, path)),
    }
}

fn string_probabilities(
    object: &Map<String, Value>,
    field: &str,
    path: &str,
    raw: &RawResponse,
) -> Result<IndexMap<String, f64>, Error> {
    let values = object
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| invalid(raw, path))?;
    values
        .iter()
        .map(|(key, value)| {
            value
                .as_f64()
                .filter(|value| value.is_finite())
                .map(|value| (key.clone(), value))
                .ok_or_else(|| invalid(raw, format!("{path}.{key}")))
        })
        .collect()
}

fn score_probabilities(
    object: &Map<String, Value>,
    field: &str,
    path: &str,
    raw: &RawResponse,
) -> Result<BTreeMap<u32, f64>, Error> {
    let values = object
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| invalid(raw, path))?;
    values
        .iter()
        .map(|(key, value)| {
            let score = key
                .parse::<u32>()
                .map_err(|_| invalid(raw, format!("{path}.{key}")))?;
            let probability = value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| invalid(raw, format!("{path}.{key}")))?;
            Ok((score, probability))
        })
        .collect()
}

fn score_legend(
    object: &Map<String, Value>,
    field: &str,
    path: &str,
    raw: &RawResponse,
) -> Result<BTreeMap<u32, JsonContent>, Error> {
    let values = object
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| invalid(raw, path))?;
    values
        .iter()
        .map(|(key, value)| {
            let score = key
                .parse::<u32>()
                .map_err(|_| invalid(raw, format!("{path}.{key}")))?;
            let content = JsonContent::try_new(value.clone())
                .map_err(|_| invalid(raw, format!("{path}.{key}")))?;
            Ok((score, content))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use http::HeaderValue;
    use serde_json::json;

    use super::*;

    fn raw(body: Value) -> RawResponse {
        let mut headers = HeaderMap::new();
        headers.insert(REQUEST_ID_HEADER, HeaderValue::from_static("req_1"));
        RawResponse::new(
            StatusCode::OK,
            headers,
            Bytes::from(serde_json::to_vec(&body).unwrap()),
            "POST https://api.test/v1/systemone".to_owned(),
        )
    }

    #[test]
    fn decodes_answers_and_skips_unknown_types() {
        let response = decode_system_one(raw(json!({
            "model": "jev-latest",
            "usage": {"input_tokens": 2},
            "answers": {
                "yes": {"type": "noul", "noul": 0.9, "future_field": 1},
                "rating": {"type": "score", "score": 0.8, "confidence": 0.7, "legend": {"0": "low", "1": {"label": "high"}}, "probabilities": {"0": 0.2, "1": 0.8}},
                "future": {"type": "future", "value": 1}
            },
            "future_field": true
        }))).unwrap();
        assert_eq!(response.request_id(), Some("req_1"));
        assert_eq!(response.noul("yes").unwrap().noul, 0.9);
        assert_eq!(response.score("rating").unwrap().probabilities[&1], 0.8);
        assert!(!response.answers.contains_key("future"));
        assert!(
            response
                .raw_response()
                .body
                .windows(6)
                .any(|part| part == b"future")
        );
    }

    #[test]
    fn reports_nested_validation_path() {
        let error = decode_system_one(raw(json!({
            "model": "jev-latest",
            "usage": {},
            "answers": {"tone": {"type": "choice", "choice": "calm", "probabilities": {}}}
        })))
        .unwrap_err();
        assert_eq!(
            error.api().unwrap().field_path.as_deref(),
            Some("answers.tone.confidence")
        );
    }
}
