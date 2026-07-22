# Command-line reference

The optional `edge-completions` command provides a narrow command-line boundary
over the same typed SDK. It sends non-streaming chat requests and writes only
validated assistant text to standard output. It never prints credentials or raw
provider request and response envelopes.

## Fast path

1. Export `CLOUDFLARE_ACCOUNT_ID` and either `CLOUDFLARE_API_TOKEN` or the
   compatibility variable `KIMI3_ON_CLOUDFLARE_API_KEY`.
2. Run `edge-completions check`.
3. Send a prompt with `edge-completions chat "Your prompt"`.

## Install

```bash
cargo install edge-completions --features cli
```

From a source checkout:

```bash
cargo run --features cli -- help
```

## Configure

Set a Cloudflare account ID and least-privilege API token:

```bash
export CLOUDFLARE_ACCOUNT_ID="your_cloudflare_account_id"
export CLOUDFLARE_API_TOKEN="your_least_privilege_api_token"
```

`KIMI3_ON_CLOUDFLARE_API_KEY` remains a compatibility fallback for the token.
New setups should use `CLOUDFLARE_API_TOKEN`.

Do not place credentials in command arguments. Arguments may be retained in
shell history or visible to local process-inspection tools.

## Commands

### `check`

```bash
edge-completions check
```

`check` validates that credentials, the optional API base URL, and the optional
AI Gateway ID can build a client. It performs no network request, so it cannot
prove that a token is authorized or that the account has usable Workers AI
balance.

Expected output:

```text
configuration is valid
```

### `chat`

```bash
edge-completions chat [OPTIONS] [PROMPT]
```

The default model is `moonshotai/kimi-k3`.

```bash
edge-completions chat \
  --system "Answer for a Rust developer." \
  --model moonshotai/kimi-k3 \
  --temperature 0.2 \
  --max-tokens 200 \
  "Explain trait boundaries in one sentence."
```

The command prints assistant text only. The exact text depends on the model.
For example:

```text
Trait boundaries let application code depend on a capability instead of one transport implementation.
```

When `PROMPT` is absent, `chat` reads at most 64 KiB from standard input:

```bash
printf '%s\n' 'Explain bounded response decoding.' | edge-completions chat
```

An interactive terminal without a prompt fails immediately rather than waiting
indefinitely. Empty, oversized, non-finite-temperature, out-of-range-temperature,
and zero-token-limit inputs fail before an API request.

## Global options

`--gateway-id <GATEWAY_ID>` adds Cloudflare's AI Gateway routing header.

`--base-url <URL>` changes the Cloudflare API base. HTTPS is required, except
for an HTTP loopback address used by local contract tests. The bearer token is
sent to this endpoint, so use this option only with a host you trust.

Global options can appear before or after the subcommand:

```bash
edge-completions --gateway-id production-gateway chat "Summarize this design."
```

Print the installed version:

```bash
edge-completions --version
```

## Output and exit contract

- Standard output contains only `configuration is valid`, help/version text,
  or final assistant text.
- Standard error contains argument, configuration, transport, or sanitized
  provider errors.
- Exit status `0` means success, help, or version output.
- Exit status `1` means configuration, input-invariant, transport, provider, or
  output failure.
- Exit status `2` means command-line syntax or argument parsing failed.

The command does not emit JSON and does not offer a raw-output mode. A response
that contains no final assistant text is a typed error. Tool execution remains
an explicit responsibility of application code using the library API.

## Common errors

### Missing environment variable

What it means: the account ID or both supported API token variables are absent
or are not valid Unicode.

How to fix:

```bash
export CLOUDFLARE_ACCOUNT_ID="your_cloudflare_account_id"
export CLOUDFLARE_API_TOKEN="your_least_privilege_api_token"
edge-completions check
```

### Provider returned HTTP 402 with code 2021

What it means: the selected Cloudflare account or AI Gateway does not have
usable Workers AI balance or provider billing.

How to fix: add the required balance or configure the appropriate provider
billing in Cloudflare, then rerun the same command. This is provider account
state, not a local parsing or compilation failure.

### Provider returned a final completion without text content

What it means: the CLI received a tool-only or empty assistant result. The CLI
prints text only and does not execute tools.

How to fix: use the library API and match `AssistantOutput` when the application
needs typed tool calls.

### Provider API base URL must use HTTPS

What it means: `--base-url` selected a non-loopback HTTP endpoint. Sending a
bearer token to that endpoint would be unsafe.

How to fix: use an HTTPS base URL. Plain HTTP is accepted only for loopback
contract tests.
