# edge-completions

[![CI](https://github.com/TAKAMAgents/edge-completions/actions/workflows/ci.yml/badge.svg)](https://github.com/TAKAMAgents/edge-completions/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/edge-completions.svg)](https://crates.io/crates/edge-completions)
[![docs.rs](https://docs.rs/edge-completions/badge.svg)](https://docs.rs/edge-completions)
[![license](https://img.shields.io/crates/l/edge-completions.svg)](LICENSE-MIT)

An ergonomic, typed Rust client for OpenAI-compatible chat completions through
Cloudflare. The first supported convenience model is `moonshotai/kimi-k3`.

This is an independent open-source project. It is not affiliated with,
endorsed by, or sponsored by Cloudflare, Inc. Cloudflare is a trademark of
Cloudflare, Inc.

## Why this crate

- A small, object-safe `ChatCompletions` trait for application boundaries.
- Validated newtypes for account IDs, API tokens, models, URLs, timeouts, and limits.
- Typed tool schemas, arguments, and results without public raw-JSON escape hatches.
- `thiserror` errors for configuration, transport, provider, response, and tool failures.
- Redacted token debug output and bounded response-body decoding.
- No autonomous tool loop: your application retains authorization and execution control.

## Install

After the first crates.io release:

```bash
cargo add edge-completions
```

The examples also need Tokio, Schemars, and Serde:

```bash
cargo add tokio --features macros,rt-multi-thread
cargo add schemars
cargo add serde --features derive
```

The minimum supported Rust version is 1.86.

## Configuration

Create a scoped Cloudflare API token and set:

```bash
export CLOUDFLARE_ACCOUNT_ID="your_cloudflare_account_id"
export CLOUDFLARE_API_TOKEN="your_least_privilege_api_token"
```

`Client::from_env()` also accepts the original
`KIMI3_ON_CLOUDFLARE_API_KEY` variable as a compatibility fallback. New
applications should use `CLOUDFLARE_API_TOKEN`.

You can find the account ID in the Cloudflare dashboard after selecting your
account, or follow Cloudflare's
[account and zone ID guide](https://developers.cloudflare.com/fundamentals/account/find-account-and-zone-ids/).
Never commit either value.

## Simple chat

```rust,no_run
use edge_completions::{ChatMessage, ChatRequest, Client, Error};

async fn answer() -> Result<(), Error> {
    let client = Client::from_env()?;
    let request = ChatRequest::kimi_k3(vec![ChatMessage::user(
        "Explain typed API boundaries in one sentence.",
    )])?;

    let completion = client.chat(&request).await?;
    let text = completion
        .first_choice()?
        .message()
        .content()
        .ok_or(Error::MissingContent)?;

    println!("{text}");
    Ok(())
}
```

Run the complete example:

```bash
source ~/.zshrc
cargo run --example simple_chat
```

## Typed tool calls

A tool contract ties together its generated JSON Schema, validated input type,
and serializable output type:

```rust
use edge_completions::ToolDefinition;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

struct GetWeather;

#[derive(Deserialize, JsonSchema)]
struct WeatherArguments {
    city: String,
}

#[derive(Serialize)]
struct WeatherReport {
    temperature_celsius: i16,
}

impl ToolDefinition for GetWeather {
    type Arguments = WeatherArguments;
    type Output = WeatherReport;

    const NAME: &'static str = "get_weather";
    const DESCRIPTION: &'static str = "Get the weather for a city";
}
```

Decode a provider proposal only through the matching contract:

```rust,ignore
let call = completion.first_choice()?.message().first_tool_call()?;
let arguments = call.arguments_for::<GetWeather>()?;

// The application authorizes and executes the action here.
let report = get_weather(arguments);
let result_message = ChatMessage::tool_result::<GetWeather>(call, &report)?;
```

Run the complete two-turn example:

```bash
source ~/.zshrc
cargo run --example weather_tool
```

The example returns deterministic sample weather; it does not call a weather
service or claim to provide a live forecast.

## Trait boundary

Application services can depend on behavior instead of the concrete HTTP client:

```rust,ignore
async fn answer(
    ai: &dyn edge_completions::ChatCompletions,
    request: &edge_completions::ChatRequest,
) -> Result<edge_completions::ChatCompletion, edge_completions::Error> {
    ai.complete(request).await
}
```

## Error and security model

- Every library failure is typed with `thiserror`; production code contains no
  `unwrap`, `expect`, or panic path.
- API tokens use redacted `Debug` and are never included in errors.
- Success and failure bodies are size-bounded. Unknown error bodies are not exposed.
- Only HTTPS endpoints are accepted, except loopback HTTP for local tests.
- Tool names and arguments are model-controlled and remain untrusted until
  `arguments_for::<T>()` validates the name and deserializes into `T::Arguments`.
- Tool execution is intentionally outside this crate.

A `402 Payment Required` with Cloudflare code `2021` means the account or AI
Gateway lacks usable Workers AI balance/provider billing. Add the required
balance or configure BYOK before retrying the live examples.

## Validation

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo deny check
cargo audit
cargo publish --dry-run
```

Tests cover exact request serialization, public trait use, typed tool-call
round trips, HTTP authentication, response validation, provider errors,
response limits, TLS policy, and secret redaction against local servers. Tests
do not make live provider calls.

## Scope and limitations

- Supported now: non-streaming chat completions, typed function tools, Kimi K3
  convenience construction, optional AI Gateway ID, timeout and body-size policy.
- Not supported yet: streaming, multimodal content, embeddings, Responses API,
  provider-specific raw extension maps, or autonomous tool execution.
- Provider contract drift remains possible. Contract changes should land with a
  versioned test before expanding the public API.

See [architecture](docs/ARCHITECTURE.md), [contributing](CONTRIBUTING.md),
[security policy](SECURITY.md), and [release process](docs/RELEASING.md).

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
