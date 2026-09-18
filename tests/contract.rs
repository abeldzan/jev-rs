use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use http::{HeaderMap, HeaderValue};
use serde_json::{Value, json};
use typesafe_sdk::{
    ApiErrorKind, Choice, Client, Error, ModelsOptions, Noul, Questions, RetryPolicy, Score,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn success_body() -> Value {
    json!({
        "model": "jev-test",
        "usage": {"input_tokens": 4, "output_tokens": 2},
        "answers": {
            "billing": {"type": "noul", "noul": 0.96},
            "tone": {
                "type": "choice",
                "choice": "angry",
                "confidence": 0.8,
                "probabilities": {"calm": 0.2, "angry": 0.8}
            },
            "urgency": {
                "type": "score",
                "score": 1.7,
                "confidence": 0.75,
                "legend": {"0": "low", "1": "medium", "2": "high"},
                "probabilities": {"0": 0.05, "1": 0.2, "2": 0.75}
            }
        }
    })
}

#[tokio::test]
async fn system_one_round_trip_and_wire_contract() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/prefix/v1/systemone"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-typesafe-request-id", "req_contract")
                .set_body_json(success_body()),
        )
        .mount(&server)
        .await;

    let mut defaults = HeaderMap::new();
    defaults.insert("x-client", HeaderValue::from_static("default"));
    defaults.insert("authorization", HeaderValue::from_static("wrong"));
    let mut per_call = HeaderMap::new();
    per_call.insert("x-client", HeaderValue::from_static("override"));
    per_call.insert("x-typesafe-sdk", HeaderValue::from_static("wrong"));
    let client = Client::builder()
        .api_key("secret")
        .base_url(format!("{}/prefix///", server.uri()))
        .model("client-model")
        .headers(defaults)
        .build()
        .unwrap();

    let response = client
        .system_one(json!({"document": "charged twice", "meta": null}))
        .model("request-model")
        .headers(per_call)
        .question("billing", Noul::new("Is this billing?"))
        .question(
            "tone",
            Choice::new("Tone?")
                .option("calm")
                .option_with_description("angry", "Clearly upset"),
        )
        .question(
            "urgency",
            Score::new("Urgency?").levels(["low", "medium", "high"]),
        )
        .extra_body("model", json!("body-model"))
        .extra_body("future", json!({"beam_width": 4}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.model, "jev-test");
    assert_eq!(response.noul("billing").unwrap().noul, 0.96);
    assert_eq!(response.choice("tone").unwrap().choice, "angry");
    assert_eq!(response.score("urgency").unwrap().probabilities[&2], 0.75);
    assert_eq!(response.request_id(), Some("req_contract"));

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.headers["authorization"], "Bearer secret");
    assert_eq!(request.headers["x-client"], "override");
    assert!(
        request.headers["x-typesafe-sdk"]
            .to_str()
            .unwrap()
            .starts_with("typesafe-sdk/")
    );
    assert!(
        request.headers["x-typesafe-runtime"]
            .to_str()
            .unwrap()
            .starts_with("rust/")
    );
    assert!(request.headers.get("x-typesafe-retry-count").is_none());
    let body: Value = request.body_json().unwrap();
    assert_eq!(body["model"], "body-model");
    assert_eq!(body["future"], json!({"beam_width": 4}));
    assert_eq!(
        body["questions"]["urgency"]["criteria"],
        json!(["low", "medium", "high"])
    );
}

#[tokio::test]
async fn models_and_unknown_response_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{
                "name": "jev",
                "description": "Production model",
                "release_date": "2026-09-01",
                "future": true
            }],
            "future": {"key": "value"}
        })))
        .mount(&server)
        .await;
    let client = Client::builder()
        .api_key("key")
        .base_url(server.uri())
        .build()
        .unwrap();
    let response = client
        .models_with_options(ModelsOptions::new().timeout(Duration::from_secs(2)))
        .await
        .unwrap();
    assert_eq!(response.models[0].name, "jev");
    assert_eq!(response.raw_response().status.as_u16(), 200);
}

#[tokio::test]
async fn status_errors_are_classified_and_retain_context() {
    for (status, kind) in [
        (400, ApiErrorKind::BadRequest),
        (401, ApiErrorKind::Authentication),
        (403, ApiErrorKind::PermissionDenied),
        (404, ApiErrorKind::NotFound),
        (422, ApiErrorKind::UnprocessableEntity),
        (429, ApiErrorKind::RateLimited),
        (503, ApiErrorKind::Internal),
        (409, ApiErrorKind::Other),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(
                ResponseTemplate::new(status)
                    .insert_header("x-typesafe-request-id", "req_error")
                    .set_body_json(json!({"detail": {"message": "failed"}})),
            )
            .mount(&server)
            .await;
        let client = Client::builder()
            .api_key("key")
            .base_url(server.uri())
            .retry(RetryPolicy::disabled())
            .build()
            .unwrap();
        let error = client.models().await.unwrap_err();
        let api = error.api().unwrap();
        assert_eq!(api.kind, kind);
        assert_eq!(api.request_id(), Some("req_error"));
        assert!(api.endpoint.as_deref().unwrap().ends_with("/v1/models"));
        assert!(error.to_string().contains("failed"));
    }
}

#[tokio::test]
async fn retry_recovers_and_sets_retry_header() {
    let server = MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let responder_calls = Arc::clone(&calls);
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(move |_request: &wiremock::Request| {
            if responder_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                ResponseTemplate::new(503).insert_header("retry-after-ms", "0")
            } else {
                ResponseTemplate::new(200).set_body_json(json!({"models": []}))
            }
        })
        .mount(&server)
        .await;
    let client = Client::builder()
        .api_key("key")
        .base_url(server.uri())
        .build()
        .unwrap();
    assert!(client.models().await.unwrap().models.is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let requests = server.received_requests().await.unwrap();
    assert!(requests[0].headers.get("x-typesafe-retry-count").is_none());
    assert_eq!(requests[1].headers["x-typesafe-retry-count"], "1");
}

#[tokio::test]
async fn retry_budget_stops_before_server_delay() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(503).insert_header("retry-after", "1"))
        .mount(&server)
        .await;
    let policy = RetryPolicy {
        timeout: Some(Duration::from_millis(100)),
        ..RetryPolicy::default()
    };
    let client = Client::builder()
        .api_key("key")
        .base_url(server.uri())
        .retry(policy)
        .build()
        .unwrap();
    assert_eq!(
        client.models().await.unwrap_err().api().unwrap().status,
        503
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn timeout_and_response_validation_are_distinct() {
    let timeout_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(100))
                .set_body_json(json!({"models": []})),
        )
        .mount(&timeout_server)
        .await;
    let client = Client::builder()
        .api_key("key")
        .base_url(timeout_server.uri())
        .retry(RetryPolicy::disabled())
        .build()
        .unwrap();
    let error = client
        .models_with_options(ModelsOptions::new().timeout(Duration::from_millis(10)))
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Timeout { .. }));

    let invalid_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": [{}]})))
        .mount(&invalid_server)
        .await;
    let client = Client::builder()
        .api_key("key")
        .base_url(invalid_server.uri())
        .build()
        .unwrap();
    let error = client.models().await.unwrap_err();
    assert_eq!(error.api().unwrap().kind, ApiErrorKind::ResponseValidation);
    assert_eq!(
        error.api().unwrap().field_path.as_deref(),
        Some("models.0.name")
    );
}

#[cfg(feature = "blocking")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocking_client_uses_the_same_contract() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": []})))
        .mount(&server)
        .await;
    let uri = server.uri();
    let models = tokio::task::spawn_blocking(move || {
        typesafe_sdk::blocking::Client::builder()
            .api_key("key")
            .base_url(uri)
            .build()
            .unwrap()
            .models()
            .unwrap()
            .models
    })
    .await
    .unwrap();
    assert!(models.is_empty());
}

#[cfg(feature = "blocking")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocking_system_one_uses_the_fluent_builder() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body()))
        .mount(&server)
        .await;
    let uri = server.uri();
    let answer = tokio::task::spawn_blocking(move || {
        let client = typesafe_sdk::blocking::Client::builder()
            .api_key("key")
            .base_url(uri)
            .build()
            .unwrap();
        client
            .system_one("ticket")
            .question("billing", Noul::new("Billing?"))
            .question("tone", Choice::new("Tone?").options(["calm", "angry"]))
            .question(
                "urgency",
                Score::new("Urgency?").levels(["low", "medium", "high"]),
            )
            .send()
            .unwrap()
            .choice("tone")
            .unwrap()
            .choice
            .clone()
    })
    .await
    .unwrap();
    assert_eq!(answer, "angry");
}

#[tokio::test]
async fn reusable_questions_use_the_same_builder_seam() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body()))
        .mount(&server)
        .await;
    let questions = Questions::with_capacity(3)
        .add("billing", Noul::new("Billing?"))
        .add("tone", Choice::new("Tone?").options(["calm", "angry"]))
        .add(
            "urgency",
            Score::new("Urgency?").levels(["low", "medium", "high"]),
        );
    let client = Client::builder()
        .api_key("key")
        .base_url(server.uri())
        .build()
        .unwrap();
    let response = client
        .system_one("ticket")
        .questions(questions)
        .send()
        .await
        .unwrap();
    assert_eq!(response.answers.len(), 3);
}

#[tokio::test]
async fn request_builder_validates_before_network_io() {
    let server = MockServer::start().await;
    let client = Client::builder()
        .api_key("key")
        .base_url(server.uri())
        .build()
        .unwrap();
    assert!(
        client
            .system_one(json!(null))
            .question("billing", Noul::new("Billing?"))
            .send()
            .await
            .is_err()
    );
    assert!(client.system_one("ticket").send().await.is_err());
    assert!(server.received_requests().await.unwrap().is_empty());
}
