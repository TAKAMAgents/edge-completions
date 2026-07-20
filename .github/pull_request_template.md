## What changed

Describe the observable behavior and owning boundary.

## Why

Describe the invariant, failure mode, or compatibility need.

## Validation

- [ ] Tests cover the behavior and relevant failure path.
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo test --all-targets --all-features`
- [ ] Public docs and changelog are updated when needed.
- [ ] No credentials, raw provider payloads, or unrelated changes are included.
