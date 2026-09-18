# Contributing

Thank you for helping improve the TypeSafe AI Rust SDK. Contributions of bug
reports, documentation, tests, and code are welcome.

By participating, you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
Security vulnerabilities should be reported privately as described in
[SECURITY.md](SECURITY.md), not in a public issue.

## Before you start

- Search existing issues and pull requests to avoid duplicate work.
- Open an issue before making a large API or behavior change so the design can
  be discussed first.
- Never include API keys, access tokens, customer data, or captured production
  responses in an issue, test fixture, commit, or pull request.

## Development setup

Install Rust with [rustup](https://rustup.rs). The minimum supported Rust
version is 1.98.1. The repository's toolchain file selects this version
automatically.

Clone your fork, enter the repository, and run the test suite:

```console
cargo test --all-features --locked
```

Tests that do not contact the TypeSafe API must use a local mock server. The
ignored live test is optional and should only be run with a dedicated test key:

```console
TYPESAFE_API_KEY="your-test-key" cargo test --test live -- --ignored
```

Live tests may consume API quota. Do not use a production credential.

## Making changes

- Keep public APIs idiomatic, typed, and backward compatible whenever possible.
- Add tests for behavior changes and bug fixes.
- Add rustdoc for new public items and update the README when usage changes.
- Preserve unknown response fields in the retained raw response where relevant.
- Do not log credentials, authorization headers, or unredacted sensitive data.
- Update `CHANGELOG.md` under `Unreleased` for user-visible changes.
- Keep changes focused; unrelated cleanup belongs in a separate pull request.

Format and validate the complete package before opening a pull request:

```console
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
cargo test --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --all-features
cargo package --allow-dirty
```

When changing code that supports the blocking feature, test both the default
and all-feature configurations. Changes must continue to compile on Rust
1.98.1.

## Pull requests

A pull request should:

- Explain the problem and the chosen solution.
- Link the relevant issue when one exists.
- Include tests that would fail without the change.
- Call out public API changes and compatibility considerations.
- Pass all continuous-integration checks.
- Contain no generated build output or secrets.

Maintainers may ask for changes before merging. Reviews focus on correctness,
API clarity, compatibility, security, documentation, and test coverage.

## Licensing

By submitting a contribution, you agree that it may be distributed under the
terms of the repository's [MIT License](LICENSE).
