# Architecture

`edge-completions` is a provider-boundary library, not an agent runtime.

```text
application -----------+
                       |
optional typed CLI ----+-> ChatCompletions trait
                              -> Client transport adapter
                                  -> private Cloudflare/OpenAI wire contract
                                      -> typed ChatCompletion and ToolCall
```

## Invariants

- Credentials become `AccountId` and redacted `ApiToken` values at ingress.
- An API base uses HTTPS unless it is an explicit loopback test endpoint.
- Every request has at least one message; every tool-enabled request has a tool.
- Tool definitions bind one name to one argument type and one output type.
- A model-proposed tool call must match that name before decoding or result encoding.
- Provider bodies are bounded and never returned as raw public values.
- The CLI accepts bounded prompt text and emits only validated assistant text.
- The SDK proposes no policy and executes no tool side effects.

The `ChatCompletions` trait is the application-facing substitution boundary. The
concrete `Client` owns HTTP authentication, endpoint construction, deadlines,
body limits, status mapping, and private wire deserialization.

The feature-gated CLI is an application adapter over that same trait boundary.
It owns command parsing, standard-input bounds, exit status, and text-only
terminal output; it does not add a second provider transport or raw-data path.

## Compatibility

Before 1.0, minor releases may refine the public type model. Deprecation is
preferred when practical. Once 1.0 is reached, semantic-versioning compatibility
is evaluated against the exported items in `src/lib.rs`.
