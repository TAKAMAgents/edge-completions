# Architecture

`edge-completions` is a provider-boundary library, not an agent runtime.

This page describes the component boundaries, data flow, compile-time request
model, runtime validation, and compatibility policy. Start with the
[README](../README.md) for installation and first use.

```text
application -----------+
                       |
optional typed CLI ----+-> native ChatCompletions port
                              |
                              +-> Client transport adapter
                              |     -> private Cloudflare/OpenAI wire contract
                              |         -> typed ChatCompletion and ToolCall
                              |
runtime type erasure --------> DynChatCompletions
                                    -> BoxChatFuture
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
- Native capability futures are `Send` and are not boxed.
- Dynamic dispatch is explicit and allocates exactly at the erasure boundary.
- Dropping a request future leaves no SDK-owned task running.
- The HTTP adapter performs no automatic retry.

The `ChatCompletions` trait is the application-facing substitution boundary. The
trait uses return-position `impl Future + Send`, so generic callers retain the
concrete future type. `DynChatCompletions` is the separate object-safe adapter
for callers that need `Arc<dyn ...>` or another runtime-selected
implementation.

The concrete `Client` owns HTTP authentication, endpoint construction,
deadlines, body limits, status mapping, and private wire deserialization.

The feature-gated CLI is an application adapter over that same trait boundary.
It owns command parsing, standard-input bounds, exit status, and text-only
terminal output; it does not add a second provider transport or untyped output
path.

## Async execution

```text
caller-owned Tokio task
    |
    +-- ChatCompletions::complete
          |
          +-- construct the HTTP request
          +-- await headers
          +-- await bounded body chunks
          +-- decode the private provider DTO
          +-- return a typed completion

drop future
    |
    +-- drop the HTTP exchange and partial body
    +-- no SDK task, queue, or tool action survives
```

The library does not create a runtime, spawn work, or hide a concurrency queue.
Reqwest is Tokio-backed, and the caller owns the runtime and task hierarchy.
The optional CLI creates a current-thread runtime only at the binary boundary.

`RequestTimeout` configures one deadline covering connection establishment
through response-body completion. Expiry maps to `Error::Timeout`. Caller
cancellation is different: dropping the future abandons the result and returns
no error.

The client explicitly disables Reqwest's protocol retry policy. Application
code owns retry classification, idempotency, and rate-limit handling. Callers
also own fan-out and must bound concurrency when processing an unbounded input
source.

See the [async execution guide](ASYNC.md) for runnable native and dynamic
dispatch examples.

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

Configuration, timeout, transport, provider status, body limits, response
decoding, and tool validation have separate typed error variants. Provider
bodies and API tokens are never returned through public errors. The caller can
match the owning failure boundary without parsing an error string.

## Compatibility

Before 1.0, minor releases may refine the public type model. Deprecation is
preferred when practical. Once 1.0 is reached, semantic-versioning compatibility
is evaluated against the exported items in `src/lib.rs`.

Version 0.4 intentionally changes `ChatCompletions` from an `async-trait`
object-safe method to a native statically dispatched future. Callers that need
type erasure migrate to `DynChatCompletions`. Request, response, tool, and
concrete `Client::chat` contracts remain compatible with version 0.3.
