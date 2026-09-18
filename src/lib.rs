//! Async-first Rust SDK for the TypeSafe AI API.
//!
//! The SDK calls System One with typed questions and lists models available to
//! the authenticated account. Set `TYPESAFE_API_KEY` and call [`Client::from_env`],
//! or provide a key with [`Client::new`].
//!
//! ```no_run
//! use typesafe_sdk::{Choice, Client, Noul};
//!
//! # async fn run() -> Result<(), typesafe_sdk::Error> {
//! let client = Client::from_env()?;
//! let response = client
//!     .system_one("I was charged twice. Please fix this ASAP.")
//!     .question("billing", Noul::new("Is this about billing?"))
//!     .question(
//!         "tone",
//!         Choice::new("What is the customer's tone?")
//!             .options(["calm", "angry"]),
//!     )
//!     .send()
//!     .await?;
//! println!("{}", response.choice("tone").unwrap().choice);
//! # Ok(())
//! # }
//! ```

mod client;
mod config;
pub mod constants;
mod error;
mod json;
mod question;
mod response;
mod retry;
mod transport;

#[cfg(feature = "blocking")]
pub mod blocking;

pub use client::{Client, ClientBuilder, ModelsOptions, SystemOneRequestBuilder};
pub use error::{ApiError, ApiErrorKind, Error, ErrorBody};
pub use json::{IntoState, JsonContent};
pub use question::{Choice, Noul, NoulCriteria, Question, Questions, Score};
pub use response::{
    Answer, ChoiceAnswer, ListModelsResponse, ModelMetadata, NoulAnswer, RawResponse, ScoreAnswer,
    SystemOneResponse, Usage,
};
pub use retry::{RetryPolicy, RetryPredicate, RetryStatuses};

/// Convenience alias for the asynchronous [`Client`].
pub type TypeSafeClient = Client;

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
