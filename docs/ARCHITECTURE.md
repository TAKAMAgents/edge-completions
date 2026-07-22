# Architecture

`edge-completions` is a provider-boundary library, not an agent runtime.

This page describes the component boundaries, data flow, compile-time request
model, runtime validation, and compatibility policy. Start with the
[README](../README.md) for installation and first use.

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
- A model-proposed tool call must match that name before decoding or result
  encoding.
- Provider bodies are bounded and never returned as raw public values.
- The CLI accepts bounded prompt text and emits only validated assistant text.
- The SDK proposes no policy and executes no tool side effects.

The `ChatCompletions` trait is the application-facing substitution boundary. The
concrete `Client` owns HTTP authentication, endpoint construction, deadlines,
body limits, status mapping, and private wire deserialization.

The feature-gated CLI is an application adapter over that same trait boundary.
It owns command parsing, standard-input bounds, exit status, and text-only
terminal output; it does not add a second provider transport or untyped output
path.

## Compile-time request model

The request builder exposes a small sealed state machine:

```text
ChatRequestBuilder<NeedsMessage, WithoutTools>
    --message--> ChatRequestBuilder<HasMessages, WithoutTools>
    --tool-----> ChatRequestBuilder<NeedsMessage, WithTools>

ChatRequestBuilder<NeedsMessage, WithTools>
    --message--> ChatRequestBuilder<HasMessages, WithTools>

ChatRequestBuilder<HasMessages, WithoutTools>
    --tool-----> ChatRequestBuilder<HasMessages, WithTools>

ChatRequestBuilder<HasMessages, *>
    --build----> ChatRequest
```

`message` and `tool` are consuming transitions. `build` exists only for the
`HasMessages` state, and `tool_choice` exists only for `WithTools`. The sealed
`MessageState` and `ToolState` trait bounds prevent downstream code from
inventing witnesses for states the SDK has not established.

The compiler rejects calls that are not available for the current state. See
the [type-system guide](TYPE_SYSTEM.md) for examples that you can copy and
paste, and for the complete boundary between compile-time and runtime checks.

### Categorical interpretation

The state machine forms the useful fragment of a category: valid states are
objects, transition functions are morphisms, each state's identity function is
the identity morphism, and Rust function composition supplies associativity.
Independent configuration endomorphisms, such as temperature and token limit,
are tested to commute.

Assistant output is modeled as the coproduct
`Text + ToolCalls + (Text × ToolCalls) + Empty`. Pattern matching therefore
forces exhaustive handling of every supported alternative. These proofs stop
at the trust boundary: deserialization, provider status, tool names, and tool
arguments remain fallible because they are external data.

After the runtime name and argument check succeeds,
`ValidatedToolCall<'a, T>` carries that proof forward. Its `result` method
accepts only `T::Output`, preventing a validated call for one tool from being
paired with another tool's output type.

## Error flow

Configuration, transport, provider status, body limits, response decoding, and
tool validation have separate typed error variants. Provider bodies and API
tokens are never returned through public errors. The caller can match the
owning failure boundary without parsing an error string.

## Compatibility

Before 1.0, minor releases may refine the public type model. Deprecation is
preferred when practical. Once 1.0 is reached, semantic-versioning compatibility
is evaluated against the exported items in `src/lib.rs`.
