# Async execution model

`edge-completions` uses native Rust futures for its primary capability boundary.
The HTTP adapter is asynchronous and Tokio-backed through Reqwest. The library
does not create a runtime or spawn background tasks.

## Choose a dispatch boundary

Use `ChatCompletions` for the normal path:

```rust
use edge_completions::{
    ChatCompletion, ChatCompletions, ChatRequest, Error,
};

async fn complete<C>(
    service: &C,
    request: &ChatRequest,
) -> Result<ChatCompletion, Error>
where
    C: ChatCompletions,
{
    service.complete(request).await
}
```

The trait returns `impl Future + Send`. Static dispatch keeps the future's
concrete type and does not allocate a box.

A downstream implementation that retains non-`Send` state across an await point
is rejected:

```compile_fail
use std::rc::Rc;

use edge_completions::{
    ChatCompletion, ChatCompletions, ChatRequest, Error,
};

struct InvalidAdapter;

impl ChatCompletions for InvalidAdapter {
    async fn complete<'a>(
        &'a self,
        _request: &'a ChatRequest,
    ) -> Result<ChatCompletion, Error> {
        let state = Rc::new(());
        std::future::ready(()).await;
        drop(state);
        Err(Error::MissingChoice)
    }
}
```

Use `DynChatCompletions` only when the implementation must be selected at
runtime:

```rust
use edge_completions::{
    ChatCompletion, ChatRequest, DynChatCompletions, Error,
};

async fn complete_dynamic(
    service: &dyn DynChatCompletions,
    request: &ChatRequest,
) -> Result<ChatCompletion, Error> {
    service.complete_boxed(request).await
}
```

Dynamic dispatch erases the concrete future into `BoxChatFuture`. The one
allocation per call is explicit in the method name and return type.

## Ownership and cancellation

```text
caller-owned task
    |
    +-- complete(request)
          |
          +-- serialize the typed request
          +-- await the HTTP response
          +-- await bounded response chunks
          +-- validate the provider response
          +-- return ChatCompletion

drop the future
    |
    +-- drop the request exchange and partial response
    +-- leave no SDK-owned task running
```

Cancellation is performed by dropping the future. This happens naturally when
a caller abandons it or another branch wins a `tokio::select!`. Cancellation
does not produce an SDK error because the caller no longer awaits an output.

The SDK does not execute tools, commit application state, or retry an
application request after any await point. A cancelled request can therefore be
discarded without compensating SDK state.

## Deadlines

`RequestTimeout` configures a whole-request deadline from connection start
through response-body completion. An elapsed deadline returns `Error::Timeout`
with the configured duration. Other interrupted or malformed exchanges return
`Error::Transport`.

The provider may still complete work after the client has cancelled an HTTP
request. Do not retry a request solely because its result was cancelled when
the provider operation could have a side effect. Chat completions are treated
as side-effect-free by this SDK, but application tools remain outside its
authority.

## Concurrency and backpressure

`Client` is `Clone`, `Send`, and `Sync`. Clones share Reqwest's connection pool,
so callers may execute independent completions concurrently.

The single-call SDK does not impose a semaphore or create an unbounded queue.
The caller owns fan-out and must apply a bounded concurrency policy when
processing an unbounded input source. Response bodies are read chunk by chunk
and rejected when they exceed `ResponseSizeLimit`.

## Runtime contract

The library does not depend directly on Tokio unless the `cli` feature is
enabled, but Reqwest's asynchronous transport is Tokio-backed. Applications
must call the SDK from a compatible Tokio runtime. The optional CLI creates a
current-thread Tokio runtime at its application boundary and reports runtime
initialization failures as typed command errors.

Executor-independent transport would require a separate HTTP adapter contract.
Version 0.4 does not introduce that additional abstraction.

## Retry contract

The Reqwest client explicitly disables its internal retry policy. The SDK never
retries a chat request. Applications may add retries outside the capability
boundary after classifying provider status, timeout, idempotency, and rate-limit
policy.

## Migration from 0.3

Version 0.3 used `async-trait`, which made `ChatCompletions` object-safe by
boxing every returned future.

| Version 0.3 | Version 0.4 |
| --- | --- |
| `&dyn ChatCompletions` | Generic `C: ChatCompletions` |
| `Arc<dyn ChatCompletions>` | `Arc<dyn DynChatCompletions>` |
| `service.complete(request)` through `dyn` | `service.complete_boxed(request)` |
| `#[async_trait] impl ChatCompletions` | Native `impl ChatCompletions` with `async fn complete` |

This is an intentional pre-1.0 compatibility change. Request types, response
types, tool contracts, and `Client::chat` remain unchanged.

## Validation

The async contract suite proves that:

- native futures are `Send`, and non-`Send` implementations are rejected;
- the dynamic adapter is object-safe;
- cloned clients support concurrent calls;
- configured deadlines return `Error::Timeout`;
- caller cancellation leaves the client reusable; and
- dropped response connections return `Error::Transport`.

Run it with:

```bash
cargo test --locked --test async_contract
```
