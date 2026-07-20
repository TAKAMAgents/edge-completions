# Contributing

Thank you for helping improve `edge-completions`.

## Development

Install the stable Rust toolchain. The minimum supported Rust version is 1.86.
Then run the same checks used by CI:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo deny check
cargo audit
```

Tests must use a local contract server by default. Do not place Cloudflare
credentials or captured provider payloads in code, fixtures, logs, issues, or
pull requests.

## Design constraints

- Keep `ChatCompletions` as the application-facing trait boundary.
- Keep provider transport data private and expose only typed domain objects.
- Treat model-produced tool names and arguments as untrusted input.
- Use `thiserror` for library errors; do not panic on fallible production paths.
- Avoid adding untyped extension maps or `serde_json::Value` to the public API.
- Keep tool execution and authorization in the calling application.

## Pull requests

Keep changes focused, update public documentation and `CHANGELOG.md`, add tests
for behavior changes, and explain any compatibility impact. Maintainers may ask
for a changeset to be split when unrelated concerns are combined.

By contributing, you agree that your contribution is licensed under the
project's MIT OR Apache-2.0 license.
