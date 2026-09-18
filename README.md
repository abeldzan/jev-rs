# TypeSafe AI Rust SDK

An async-first Rust client for the [TypeSafe AI](https://typesafe.ai) API.
It provides typed System One questions and answers, model discovery, configurable
retries, structured errors, and an optional blocking client.

## Requirements

- Rust 1.98.1 or newer
- A TypeSafe API key
- Tokio when using the default asynchronous client

The crate is not yet published to crates.io. To use a local checkout:

```toml
[dependencies]
typesafe-sdk = { path = "../typesafe-sdk" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Quickstart

Set your API key without committing it to source control:

```console
export TYPESAFE_API_KEY="your-api-key"
```

Then create a client and send a System One request:

```rust
use typesafe_sdk::{Choice, Client, Noul, Score};

#[tokio::main]
async fn main() -> Result<(), typesafe_sdk::Error> {
    let client = Client::from_env()?;
    let response = client
        .system_one("I was charged twice. Please fix this ASAP.")
        .question("billing", Noul::new("Is this ticket about billing?"))
        .question(
            "tone",
            Choice::new("What is the customer's tone?")
                .options(["calm", "frustrated", "angry"]),
        )
        .question(
            "urgency",
            Score::new("How urgent is this ticket?")
                .levels(["can wait", "this week", "today"]),
        )
        .send()
        .await?;

    println!("billing: {}", response.noul("billing").unwrap().noul);
    println!("tone: {}", response.choice("tone").unwrap().choice);
    println!("urgency: {}", response.score("urgency").unwrap().score);
    Ok(())
}
```

Question collections can be built once and reused:

```rust
use typesafe_sdk::{Choice, Noul, Questions};

let questions = Questions::new()
    .add("billing", Noul::new("Is this about billing?"))
    .add(
        "tone",
        Choice::new("What is the tone?").options(["calm", "angry"]),
    );

let response = client
    .system_one("I was charged twice.")
    .questions(questions)
    .send()
    .await?;
# Ok::<(), typesafe_sdk::Error>(())
```

## Blocking client

Enable the `blocking` feature for applications that do not use an async
runtime:

```toml
[dependencies]
typesafe-sdk = { path = "../typesafe-sdk", features = ["blocking"] }
```

```rust,no_run
use typesafe_sdk::{Noul, blocking::Client};

let client = Client::from_env()?;
let response = client
    .system_one("I was charged twice.")
    .question("billing", Noul::new("Is this about billing?"))
    .send()?;
# Ok::<(), typesafe_sdk::Error>(())
```

## Configuration

| Setting | Environment variable | Default |
| --- | --- | --- |
| API key | `TYPESAFE_API_KEY` | required |
| Base URL | `TYPESAFE_BASE_URL` | `https://api.typesafe.ai` |
| Model | `TYPESAFE_DEFAULT_MODEL` | `jev-latest` |
| Per-attempt timeout | builder only | 10 seconds |

Explicit builder values take precedence over environment values. Values read
from the environment are trimmed. Individual calls can override the model,
retry policy, timeout, headers, and additional body fields where supported.

Use `Client::builder()` for explicit configuration:

```rust
use std::time::Duration;
use typesafe_sdk::{Client, RetryPolicy};

let client = Client::builder()
    .api_key("your-api-key")
    .model("jev-latest")
    .timeout(Duration::from_secs(20))
    .retry(RetryPolicy::default())
    .build()?;
# Ok::<(), typesafe_sdk::Error>(())
```

## Reliability and diagnostics

Requests retry transient HTTP, connection, and timeout failures by default.
Retry behavior can be replaced per client or per call with `RetryPolicy`.
Successful responses retain their HTTP status, headers, body, sanitized
endpoint, and request ID through `raw_response()`.

The crate emits [`tracing`](https://docs.rs/tracing) events with the
`typesafe_sdk` target. It does not install a global subscriber. Applications
can opt in with a filter such as `RUST_LOG=typesafe_sdk=debug`. Sensitive
headers are redacted from diagnostic output.

## Development

```console
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
cargo test --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --all-features
cargo package --allow-dirty
```

The live integration test is ignored by default because it makes billable API
requests. Run it explicitly only with a test key:

```console
TYPESAFE_API_KEY="your-test-key" cargo test --test live -- --ignored
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development workflow and pull
request checklist. Please report vulnerabilities according to
[SECURITY.md](SECURITY.md).

## License

Licensed under the [MIT License](LICENSE).
