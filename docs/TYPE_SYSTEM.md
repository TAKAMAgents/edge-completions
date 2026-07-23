# Type-system guide

This guide explains which `edge-completions` guarantees the Rust compiler can
enforce and which checks must remain at runtime.

## Fast path

Build a request through typestate:

```rust
use edge_completions::{ChatMessage, ChatRequest};

let request = ChatRequest::kimi_k3_builder()
    .message(ChatMessage::user("Explain typestate in one sentence."))
    .build();

assert_eq!(request.model().as_str(), "moonshotai/kimi-k3");
```

Classify provider output with an exhaustive match:

```rust
use edge_completions::{AssistantMessage, AssistantOutput};

fn output_kind(message: &AssistantMessage) -> &'static str {
    match message.output() {
        AssistantOutput::Text(_) => "text",
        AssistantOutput::ToolCalls(_) => "tool-calls",
        AssistantOutput::TextAndToolCalls { .. } => "text-and-tool-calls",
        AssistantOutput::Empty => "empty",
    }
}
```

## Request states

`ChatRequestBuilder<M, T>` tracks two independent facts:

- `M` records whether the request contains at least one message.
- `T` records whether the request contains at least one tool definition.

```text
(NeedsMessage, WithoutTools) --message--> (HasMessages, WithoutTools)
              |                                  |
            tool                               tool
              |                                  |
              v                                  v
(NeedsMessage, WithTools) ----message--> (HasMessages, WithTools)
```

Only a `HasMessages` builder has `build`. Only a `WithTools` builder has
`tool_choice`. The state traits are sealed, so downstream crates cannot invent
a witness for a state that the SDK has not established.

These calls therefore fail during compilation:

```compile_fail
use edge_completions::ChatRequest;

let _request = ChatRequest::kimi_k3_builder().build();
```

```compile_fail
use edge_completions::{ChatRequest, ToolChoice};

let _request = ChatRequest::kimi_k3_builder()
    .tool_choice(ToolChoice::Required);
```

## Typed tool proof

A model-generated tool call is untrusted. `ToolCall::validate::<T>()` checks the
function name and decodes its arguments into `T::Arguments`. Success returns
`ValidatedToolCall<T>`.

That value is a proof carried by the type system:

- `arguments()` returns only `T::Arguments`.
- `result()` accepts only `T::Output`.
- The tool-call identifier remains attached to the result message.

The application still decides whether the tool is authorized and whether to
execute it. The SDK never executes a proposed tool.

## Exhaustive assistant output

An assistant message can contain text, tool calls, both, or neither. Reading
`content()` and `tool_calls()` independently makes it easy to ignore a valid
combination. `AssistantOutput` represents the complete set as one enum:

```text
AssistantOutput = Text
                | ToolCalls
                | TextAndToolCalls
                | Empty
```

Rust requires an exhaustive `match`. If a future release adds another output
variant, callers without a wildcard branch receive a compile error and must
choose how to handle the new case.

## Why category terms apply

The plain state-machine explanation above is sufficient for using the SDK. The
same design also has a small categorical interpretation:

- Builder states are objects.
- Consuming methods such as `message` and `tool` are morphisms between states.
- Method chaining is morphism composition.
- The identity function is the identity morphism for each state.
- Rust function composition is associative.
- `AssistantOutput` is a coproduct, or tagged choice, with one product branch
  containing both text and tool calls.

This interpretation is useful because it focuses review on legal composition.
It does not introduce general-purpose algebra traits or claim that external
data is statically proven.

## Compile-time and runtime boundary

| Concern | Enforcement | Reason |
| --- | --- | --- |
| At least one request message | Compile time | The `build` method exists only for `HasMessages` |
| Tool choice follows a tool | Compile time | `tool_choice` exists only for `WithTools` |
| Tool output matches the validated contract | Compile time | `ValidatedToolCall<T>::result` requires `T::Output` |
| Assistant alternatives are considered | Compile time | `AssistantOutput` is exhaustively matched |
| A native completion future can move between executor threads | Compile time | `ChatCompletions` returns `impl Future + Send` |
| Account ID, model ID, limits, and URLs are valid | Runtime at construction | Values may come from files, arguments, or environment variables |
| HTTP exchange succeeds | Runtime | Network state is external to the program |
| Provider response matches the contract | Runtime | Provider data is untrusted |
| Tool name and arguments match `T` | Runtime | Model output is untrusted |
| A tool action is authorized and succeeds | Calling application | Authorization and side effects are outside this SDK |

Moving an external-data check into a type does not remove the runtime check. It
records the successful check so later code cannot accidentally bypass it.

`DynChatCompletions` deliberately erases the native future type into
`BoxChatFuture`. Type erasure preserves the typed completion and error output,
but it trades static dispatch for an explicit allocation. See the
[async execution guide](ASYNC.md) for that boundary.

## Compatibility API

The runtime-checked `ChatRequest::new`, `ChatRequest::kimi_k3`, `with_tool`,
`with_tools`, and `ToolCall::arguments_for` APIs remain available for version
0.2 callers. Version 0.4 changes only the async capability dispatch described in
the [migration guide](ASYNC.md#migration-from-03). New integrations should
prefer typestate construction and `ValidatedToolCall<T>` when the additional
compiler guarantees are useful.

## Validation

Compile all examples, including the intentionally invalid examples:

```bash
cargo test --locked --doc --all-features
```

Run the law and invariant tests:

```bash
cargo test --locked --all-targets --all-features
```

The test suite verifies that the typestate and compatibility builders serialize
the same request, independent configuration transitions commute, every
assistant-output variant is classified, validated tool results retain the
matching tool contract, and native completion futures satisfy their `Send`
contract.
