# Repository instructions

- Keep `edge-completions` a small provider SDK; do not add an autonomous tool-execution loop here.
- Preserve the `ChatCompletions` trait as the application-facing substitution boundary.
- Keep Cloudflare/OpenAI transport DTOs separate from application-specific domain models.
- Treat tool calls and tool arguments as untrusted input and deserialize them into validated types.
- Preserve the sealed request typestate, exhaustive `AssistantOutput`, and `ValidatedToolCall<T>` proof boundary.
- Never log or expose API tokens. Secret-bearing types must use redacted `Debug` implementations.
- Add boundary tests for request/response contract changes and use only local mock servers by default.
- Keep public Rustdoc explicit about errors and the boundary between compile-time guarantees and runtime validation.
- Write public documentation in direct international English with consistent terminology and runnable examples.
- Run formatting, Clippy with warnings denied, all tests, strict Rustdoc, and package validation before claiming completion.
