# Contributing

Thank you for helping improve `edge-completions`.

## Development

Install the stable Rust toolchain. The minimum supported Rust version is 1.86.
Then run the same checks used by CI:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --locked --no-deps --all-features
cargo test --locked --doc --all-features
cargo deny check
cargo audit
```

Tests must use a local contract server by default. Do not place Cloudflare
credentials or captured provider payloads in code, fixtures, logs, issues, or
pull requests.

## Design constraints

- Keep native `ChatCompletions` as the generic application boundary. Its
  returned future must remain `Send`.
- Keep runtime type erasure in `DynChatCompletions`; do not reintroduce boxing
  on every generic call.
- Do not spawn SDK tasks or add an internal queue. The caller owns task
  supervision, cancellation, fan-out, and bounded concurrency.
- Preserve drop-based cancellation, typed timeout classification, and the
  explicit no-retry transport policy.
- Keep provider transport data private and expose only typed domain objects.
- Treat model-produced tool names and arguments as untrusted input.
- Preserve the request typestate and proof-carrying tool boundary. Add a
  compile-fail doctest when a new illegal sequence should be rejected by Rust.
- Model response alternatives with semantic enums and exhaustive matches.
- Use `thiserror` for library errors; do not panic on fallible production paths.
- Avoid adding untyped extension maps or `serde_json::Value` to the public API.
- Keep tool execution and authorization in the calling application.
- Use category-theory terms only when they clarify a concrete type, transition,
  composition law, or testable invariant.

## Documentation

- Write direct international English with short sentences and consistent terms.
- Put developer tasks and runnable examples before theory.
- Add `# Errors` to every fallible public API and `# Panics` when a public API
  can panic. The crate treats missing sections as Clippy warnings.
- Explain which guarantees occur at compile time and which checks remain at
  runtime because they depend on external data.
- Keep examples free of credentials, account identifiers, captured provider
  bodies, and machine-specific paths.
- Verify Rust examples with doctests and confirm CLI examples against the real
  command help and contract tests.
- Document whether an async operation owns tasks, how cancellation works, which
  deadline applies, and where backpressure is enforced.

## Pull requests

Keep changes focused. Update public documentation and `CHANGELOG.md`, add tests
for behavior changes, and explain compatibility impact. Maintainers may ask for
a changeset to be split when it combines unrelated concerns.

Changes to an async trait must include downstream compile coverage for both the
native generic path and the explicit dynamic adapter. Cancellation and timeout
changes require deterministic local failure tests; do not use production
credentials.

By contributing, you agree that your contribution is licensed under the
project's MIT OR Apache-2.0 license.
