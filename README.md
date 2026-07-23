# edge-completions

[![CI](https://github.com/TAKAMAgents/edge-completions/actions/workflows/ci.yml/badge.svg)](https://github.com/TAKAMAgents/edge-completions/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/edge-completions.svg)](https://crates.io/crates/edge-completions)
[![docs.rs](https://docs.rs/edge-completions/badge.svg)](https://docs.rs/edge-completions)
[![license](https://img.shields.io/crates/l/edge-completions.svg)](https://github.com/TAKAMAgents/edge-completions/blob/main/LICENSE-MIT)

An ergonomic, typed Rust SDK and optional command-line client for
OpenAI-compatible chat completions through Cloudflare. The first supported
convenience model is
[`moonshotai/kimi-k3`](https://developers.cloudflare.com/ai/models/moonshotai/kimi-k3/).

This is an independent open-source project. It is not affiliated with,
endorsed by, or sponsored by Cloudflare, Inc. Cloudflare is a trademark of
Cloudflare, Inc.

## Why this crate

- A native `ChatCompletions` trait with `Send` futures and no boxing on the
  generic path.
- An explicit `DynChatCompletions` adapter when runtime type erasure is needed.
- A sealed typestate request builder that rejects illegal construction
  sequences at compile time.
- Validated newtypes for account IDs, API tokens, models, URLs, timeouts, and limits.
- Typed tool schemas, arguments, and results without a public untyped JSON
  escape hatch.
- `thiserror` errors for configuration, transport, provider, response, and tool
  failures.
- Drop-based request cancellation, typed whole-request timeouts, and no hidden
  retry or background-task policy.
- Redacted token debug output and bounded response-body decoding.
- An opt-in `edge-completions` command that prints typed assistant text and
  never prints provider envelopes.
- No autonomous tool loop: your application retains authorization and
  execution control.

## Install

Add the library:

```bash
cargo add edge-completions
```

Install the optional command-line client:

```bash
cargo install edge-completions --features cli
```

The `cli` feature is intentionally disabled for library consumers, so SDK-only
builds do not compile command-line dependencies. The minimum supported Rust
version is 1.86.

Reqwest's asynchronous transport is Tokio-backed, so applications using the
SDK need a Tokio runtime. Typed tool definitions use Schemars and Serde:

```bash
cargo add tokio --features macros,rt-multi-thread
cargo add schemars
cargo add serde --features derive
```

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
If Wrangler is installed, `wrangler whoami` also reports the active account ID.
Never commit either value.

Cloudflare routes third-party models such as Kimi K3 through the account's
default AI Gateway when no gateway header is present. Set `GatewayId` only when
the request must use a named gateway. Follow Cloudflare's
[AI Gateway setup guide](https://developers.cloudflare.com/ai-gateway/get-started/)
when selecting least-privilege token permissions.

## Command line

Validate configuration without making an API request:

```bash
source ~/.zshrc
edge-completions check
```

Expected output:

```text
configuration is valid
```

Send a prompt and print only the assistant's text:

```bash
edge-completions chat \
  --temperature 0.2 \
  --max-tokens 200 \
  "Explain typed API boundaries in one sentence."
```

Prompts can also come from standard input:

```bash
printf '%s\n' 'Explain capability traits concisely.' | edge-completions chat
```

Use `edge-completions help chat` for all chat options. The command returns typed,
sanitized errors on standard error. It does not expose raw request or response
envelopes and does not execute model-proposed tools. See the complete
[CLI reference](https://github.com/TAKAMAgents/edge-completions/blob/main/docs/CLI.md),
including exit codes and the base-URL security contract.

## Library quickstart

```rust,no_run
use edge_completions::{AssistantOutput, ChatMessage, ChatRequest, Client, Error};

async fn answer() -> Result<(), Error> {
    let client = Client::from_env()?;
    let request = ChatRequest::kimi_k3_builder()
        .message(ChatMessage::user(
            "Explain typed API boundaries in one sentence.",
        ))
        .build();

    let completion = client.chat(&request).await?;
    let text = match completion.first_choice()?.message().output() {
        AssistantOutput::Text(text) => text,
        AssistantOutput::TextAndToolCalls { text, .. } => text,
        AssistantOutput::ToolCalls(_) | AssistantOutput::Empty => {
            return Err(Error::MissingContent);
        }
    };

    println!("{text}");
    Ok(())
}
```

Run the complete example:

```bash
source ~/.zshrc
cargo run --example simple_chat
```

Expected result: the example prints the model identifier followed by validated
assistant text. It never prints the provider response envelope.

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
let validated_call = call.validate::<GetWeather>()?;

// The application authorizes and executes the action here.
let report = get_weather(validated_call.arguments());
let result_message = validated_call.result(&report)?;
```

Run the complete two-turn example:

```bash
source ~/.zshrc
cargo run --example weather_tool
```

The example returns deterministic sample weather; it does not call a weather
service or claim to provide a live forecast.

Output follows this shape. The final sentence depends on the model:

```text
Tool requested: get_weather
Validated city: San Francisco
Final answer: ...
```

## Compile-time composition

The typestate builder represents valid request states as types. Adding a
message or tool consumes one state and returns the next, while trait bounds
control which operations exist:

```rust
# use edge_completions::{ChatMessage, ChatRequest, FunctionTool, ToolChoice, ToolDefinition};
# use schemars::JsonSchema;
# use serde::{Deserialize, Serialize};
# struct GetWeather;
# #[derive(Deserialize, JsonSchema)] struct WeatherArguments { city: String }
# #[derive(Serialize)] struct WeatherReport { temperature_celsius: i16 }
# impl ToolDefinition for GetWeather {
#     type Arguments = WeatherArguments;
#     type Output = WeatherReport;
#     const NAME: &'static str = "get_weather";
#     const DESCRIPTION: &'static str = "Get the weather for a city";
# }
# fn request() -> Result<edge_completions::ChatRequest, edge_completions::ToolError> {
let request = ChatRequest::kimi_k3_builder()
    .message(ChatMessage::user("Weather in Paris?"))
    .tool(FunctionTool::for_tool::<GetWeather>()?)
    .tool_choice(ToolChoice::Required)
    .build();
# Ok(request)
# }
```

Calling `build` before `message`, or `tool_choice` before `tool`, does not
compile. `AssistantOutput` models response alternatives as an exhaustive sum
type, so callers must handle text, tool calls, text and tool calls, and empty
output.

| Guarantee | Enforced by | Failure point |
| --- | --- | --- |
| A request has at least one message | `ChatRequestBuilder` typestate | Compilation |
| Tool choice follows at least one tool | `WithTools` trait bound | Compilation |
| A tool result matches its tool contract | `ValidatedToolCall<T>` | Compilation |
| Every supported assistant outcome is handled | Exhaustive `AssistantOutput` match | Compilation |
| Provider data matches the declared contract | Typed deserialization and validation | Runtime |

The same design has a small categorical interpretation: request states are
objects, legal transitions are composable morphisms, and assistant output is a
coproduct with a product branch. External network and model data still require
typed runtime validation.

See the [type-system guide](https://docs.rs/edge-completions/latest/edge_completions/type_system/)
for the complete state graph, compile-fail examples, and the boundary between
static guarantees and runtime checks.

## Native async boundary

Use generic dispatch for the normal application boundary:

```rust,no_run
use edge_completions::{
    ChatCompletion, ChatCompletions, ChatRequest, Error,
};

async fn answer<C>(
    ai: &C,
    request: &ChatRequest,
) -> Result<ChatCompletion, Error>
where
    C: ChatCompletions,
{
    ai.complete(request).await
}
```

`ChatCompletions::complete` returns `impl Future + Send`. The static path keeps
the concrete future and performs no heap allocation for trait dispatch.

Use explicit type erasure only when the concrete implementation is selected at
runtime:

```rust,no_run
use edge_completions::{
    ChatCompletion, ChatRequest, DynChatCompletions, Error,
};

async fn answer_dynamic(
    ai: &dyn DynChatCompletions,
    request: &ChatRequest,
) -> Result<ChatCompletion, Error> {
    ai.complete_boxed(request).await
}
```

`DynChatCompletions` returns `BoxChatFuture`, making its one allocation per call
visible in the API. See the
[async execution guide](https://docs.rs/edge-completions/latest/edge_completions/async_model/)
for cancellation, deadlines, concurrency, runtime ownership, and retry policy.

Dropping either future cancels the in-flight exchange. The SDK spawns no task,
retains no partial response, and performs no retry. `RequestTimeout` covers the
connection and complete response body; expiry returns `Error::Timeout`.

## Migrating from 0.3

Version 0.4 replaces the `async-trait` capability method with a native Rust
future. This is an intentional pre-1.0 compatibility change.

| 0.3 API | 0.4 replacement |
| --- | --- |
| `&dyn ChatCompletions` | Generic `C: ChatCompletions` |
| `Arc<dyn ChatCompletions>` | `Arc<dyn DynChatCompletions>` |
| `ai.complete(request)` through `dyn` | `ai.complete_boxed(request)` |
| `#[async_trait] impl ChatCompletions` | Native implementation with `async fn complete` |

`Client::chat`, request and response types, typestate construction, and typed
tool contracts are unchanged.

## Migrating from 0.2

Version 0.3 introduced these additive type-system APIs, and version 0.4 retains
them. Existing `ChatRequest::new`, `ChatRequest::kimi_k3`, `with_tool`,
`with_tools`, and `ToolCall::arguments_for` calls remain available. New code
should prefer these replacements:

| 0.2 API | Preferred 0.3 API | Benefit |
| --- | --- | --- |
| `ChatRequest::kimi_k3(messages)` | `ChatRequest::kimi_k3_builder().message(...).build()` | Non-empty messages are proven at compile time |
| `request.with_tools(tools, choice)` | `.tool(...).tool_choice(choice)` | Tool choice cannot precede a tool |
| `call.arguments_for::<T>()` | `call.validate::<T>()` | The validation proof remains available for result encoding |
| `message.content()` plus `tool_calls()` | `message.output()` | All supported output combinations are handled together |

No automatic migration is required. Adopt the new API when compile-time
guarantees are useful at the call site.

Keep the environment-based credential lookup while customizing the transport:

```rust,no_run
use edge_completions::{Client, GatewayId};

fn client() -> Result<Client, edge_completions::Error> {
    Client::builder_from_env()?
        .gateway_id(GatewayId::new("production-gateway")?)
        .build()
}
```

## Features

| Feature | Default | Purpose |
| --- | --- | --- |
| `cli` | No | Builds the installable `edge-completions` command. |

## Error and security model

- Every library failure is typed with `thiserror`; production code contains no
  `unwrap`, `expect`, or panic path.
- Configured request deadlines return `Error::Timeout`; caller cancellation
  drops the future and therefore returns no SDK result.
- Local request cardinality and tool-choice sequencing are enforced by typestate;
  provider responses and model-produced tool calls are checked at runtime.
- `ValidatedToolCall<T>` is a proof-carrying value that binds decoded arguments
  and encoded output to the same `ToolDefinition` at compile time.
- API tokens use redacted `Debug` and are never included in errors.
- Success and failure bodies are size-bounded. Unknown error bodies are not
  exposed.
- Only HTTPS endpoints are accepted, except loopback HTTP for local tests.
- Tool names and arguments are model-controlled and remain untrusted until
  `validate::<T>()` produces a `ValidatedToolCall<T>` witness. The compatibility
  helper `arguments_for::<T>()` performs the same validation and returns the
  arguments.
- Tool execution is intentionally outside this crate.
- The HTTP adapter explicitly disables automatic retries. Applications own any
  retry classification and bounded concurrency policy.

A `402 Payment Required` with Cloudflare code `2021` means the account or AI
Gateway lacks usable Workers AI balance/provider billing. Add the required
balance or configure BYOK before retrying the live examples.

## Validation

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo test --locked --doc --all-features
RUSTDOCFLAGS="-D warnings -D missing_docs" cargo doc --locked --no-deps --all-features
cargo deny check
cargo audit
cargo publish --locked --dry-run
```

Tests cover exact request serialization, native and dynamic trait use, future
`Send` guarantees, caller cancellation, typed timeouts, concurrent calls,
dropped connections, typed tool-call round trips, HTTP authentication, response
validation, provider errors, response limits, TLS policy, and secret redaction
against local servers. Tests do not make live provider calls.

## Scope and limitations

- Supported now: non-streaming chat completions, typed function tools, Kimi K3
  convenience construction, optional AI Gateway ID, timeout and body-size policy.
- Not supported yet: streaming, multimodal content, embeddings, Responses API,
  provider-specific raw extension maps, or autonomous tool execution.
- Provider contract drift remains possible. Contract changes should land with a
  versioned test before expanding the public API.

See the [architecture](https://github.com/TAKAMAgents/edge-completions/blob/main/docs/ARCHITECTURE.md),
[async execution guide](https://docs.rs/edge-completions/latest/edge_completions/async_model/),
[type-system guide](https://docs.rs/edge-completions/latest/edge_completions/type_system/),
[contributing guide](https://github.com/TAKAMAgents/edge-completions/blob/main/CONTRIBUTING.md),
[CLI reference](https://github.com/TAKAMAgents/edge-completions/blob/main/docs/CLI.md),
[security policy](https://github.com/TAKAMAgents/edge-completions/blob/main/SECURITY.md),
and [release process](https://github.com/TAKAMAgents/edge-completions/blob/main/docs/RELEASING.md).

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
