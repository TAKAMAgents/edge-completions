# Security policy

## Reporting a vulnerability

Please use GitHub's private vulnerability reporting for this repository. Do not
open a public issue with exploit details, credentials, request bodies, response
bodies, account identifiers, or other sensitive data.

Include the affected version, the boundary involved, safe reproduction steps,
impact, and any suggested remediation. A maintainer will acknowledge the report
and coordinate disclosure after a fix is available.

## Supported versions

Before 1.0, only the latest released minor version receives security fixes.

## Security boundaries

This crate sends requests and validates model proposals. It does not authorize
or execute tools. Applications must authenticate users, authorize each tool
action, validate typed arguments, apply timeouts and idempotency where relevant,
and verify external side effects independently.

Compile-time guarantees cover local program structure:

- `ChatRequestBuilder` prevents empty requests and invalid tool-choice ordering.
- `AssistantOutput` requires callers to consider every supported output shape.
- `ValidatedToolCall<T>` prevents pairing a validated call with another tool's
  output type.

These guarantees do not make provider data trustworthy. HTTP responses, tool
names, tool arguments, authorization decisions, and side effects still require
runtime validation at the owning application boundary.

API tokens use redacted `Debug` output. Public errors do not retain raw provider
bodies. Success and error bodies are size-bounded. Custom API base URLs require
HTTPS, except for explicit loopback addresses used by local contract tests.
Applications should use a least-privilege Cloudflare token and must not log
requests, prompts, account identifiers, or tool arguments without an explicit
data-handling policy.

## Verification for security-sensitive changes

Run the local contract tests, dependency policy, advisory scan, and secret scan
before release. Tests must not use production credentials or captured provider
payloads. A successful local test does not prove that a live Cloudflare account
is authorized, funded, or configured correctly.
