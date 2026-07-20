# Repository instructions

- Keep `edge-completions` a small provider SDK; do not add an autonomous tool-execution loop here.
- Preserve the `ChatCompletions` trait as the application-facing substitution boundary.
- Keep Cloudflare/OpenAI transport DTOs separate from application-specific domain models.
- Treat tool calls and tool arguments as untrusted input and deserialize them into validated types.
- Never log or expose API tokens. Secret-bearing types must use redacted `Debug` implementations.
- Add boundary tests for request/response contract changes and use only local mock servers by default.
- Run format, Clippy with warnings denied, and all tests before claiming completion.
