# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-07-20

### Added

- Typed chat-completion requests and responses for Cloudflare's OpenAI-compatible endpoint.
- A `ChatCompletions` trait for substitutable application boundaries.
- Typed function-tool schemas, argument decoding, and result encoding.
- Validated account, token, model, sampling, timeout, and endpoint configuration.
- Structured `thiserror` failures that do not expose raw provider bodies or API tokens.
- Local provider-contract tests and runnable chat and tool-use examples.

[Unreleased]: https://github.com/TAKAMAgents/edge-completions/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/TAKAMAgents/edge-completions/releases/tag/v0.1.0
