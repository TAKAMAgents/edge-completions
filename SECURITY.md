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
