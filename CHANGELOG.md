# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-07-23

### Added

- A native `ChatCompletions` capability returning `impl Future + Send` without
  boxing on generic calls.
- An explicit object-safe `DynChatCompletions` adapter and `BoxChatFuture` for
  runtime-selected implementations.
- Typed `Error::Timeout` classification for configured whole-request deadlines.
- Deterministic async contract tests for future `Send`, concurrent calls,
  caller cancellation, deadline expiry, client reuse, and dropped connections.
- A complete async execution guide covering runtime ownership, cancellation,
  deadlines, concurrency, backpressure, retries, and migration.

### Changed

- **Breaking:** `ChatCompletions` is no longer object-safe. Replace
  `&dyn ChatCompletions` with generic dispatch or
  `&dyn DynChatCompletions`, and call `complete_boxed` on the dynamic boundary.
- Removed the direct production dependency on `async-trait`. Downstream
  implementations can use a native `async fn complete`.
- Pinned Reqwest's minimum compatible version to 0.12.28 and explicitly
  disabled its internal retry policy.
- The CLI and integration contracts now use native generic dispatch.

## [0.3.0] - 2026-07-22

### Added

- A sealed, trait-bound `ChatRequestBuilder` typestate API that makes missing
  messages and tool-choice-before-tool sequences compile-time errors.
- An exhaustive `AssistantOutput` sum type for text, tool calls,
  text and tool calls, and empty provider outcomes.
- A `ValidatedToolCall<T>` proof type that binds validated model arguments and
  application results to the same tool contract.

### Changed

- The CLI and bundled examples now build requests through typestate and handle
  assistant output through an exhaustive match.
- Public fallible APIs now document their complete error contracts. Clippy
  treats missing `# Errors` and `# Panics` sections as release-blocking warnings.
- Developer documentation now separates compile-time guarantees from runtime
  validation and provides a focused type-system guide and `0.2` migration path.

## [0.2.0] - 2026-07-22

### Added

- An opt-in `edge-completions` CLI with offline configuration checks, typed chat
  options, bounded prompt input, sanitized text-only output, and subprocess
  contract coverage.
- `Client::builder_from_env` for combining standard credential lookup with
  custom transport or AI Gateway configuration.
- A crates.io/docs.rs landing page, feature table, and complete CLI reference.

### Changed

- Added OIDC trusted-publishing automation for future releases.
- Optimized installed CLI release builds with fat LTO, one codegen unit, panic
  abort behavior, and stripped symbols.

## [0.1.0] - 2026-07-20

### Added

- Typed chat-completion requests and responses for Cloudflare's OpenAI-compatible endpoint.
- A `ChatCompletions` trait for substitutable application boundaries.
- Typed function-tool schemas, argument decoding, and result encoding.
- Validated account, token, model, sampling, timeout, and endpoint configuration.
- Structured `thiserror` failures that do not expose raw provider bodies or API tokens.
- Local provider-contract tests and runnable chat and tool-use examples.

[Unreleased]: https://github.com/TAKAMAgents/edge-completions/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/TAKAMAgents/edge-completions/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/TAKAMAgents/edge-completions/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/TAKAMAgents/edge-completions/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/TAKAMAgents/edge-completions/releases/tag/v0.1.0
