# Releasing

Publishing a crates.io version is permanent. Perform these steps from a clean,
reviewed commit and never pass a registry token on the command line.

## Preconditions

- Confirm that the latest crates.io version and GitHub release refer to the same
  commit and tag.
- Choose the next version according to the public compatibility impact.
- Update `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, and user-facing guides.
- Confirm that every public fallible API documents `# Errors` and that all
  compile-fail guarantees still fail for the intended reason.
- For an async-boundary release, inspect `cargo public-api`: the native
  `ChatCompletions` method must return `impl Future + Send`, and only
  `DynChatCompletions` may expose `BoxChatFuture`.
- Compile clean downstream fixtures for both generic native dispatch and
  explicit dynamic dispatch.
- Scan the exact package contents for credentials before publication.

## Bootstrap release

1. Confirm the crate name and repository URL resolve to this project.
2. Update `CHANGELOG.md` and verify the version in `Cargo.toml`.
3. Run the full local validation suite from `README.md` on Rust 1.86 and stable.
4. Inspect `cargo package --list` and unpacked package contents.
5. Run `cargo publish --locked --dry-run`.
6. Confirm crates.io account ownership and use Cargo's configured credential provider.
7. Run `cargo publish --locked` once.
8. Verify the crates.io page, docs.rs build, checksum, repository links, and
   installation from a clean consumer project.
9. Create and push the matching signed `vMAJOR.MINOR.PATCH` tag and GitHub
   release.

Use this manual path only when a crate does not yet exist on crates.io. Trusted
publishing cannot be configured until the first version exists.

## Trusted releases

Trusted publishing is bound to `TAKAMAgents/edge-completions`, `publish.yml`,
and the GitHub `release` environment. The workflow obtains a short-lived token
with OIDC; the repository stores no crates.io secret.

1. Run the complete validation suite documented in `README.md`, including
   `cargo publish --locked --dry-run`.
2. Commit the version and documentation update, push `main`, and wait for CI to
   pass on that exact commit.
3. Create an annotated `vMAJOR.MINOR.PATCH` tag on that commit and push only the
   tag.
4. Approve the protected `release` environment deployment when GitHub requests
   approval.
5. The workflow verifies that the tag matches `Cargo.toml`, belongs to `main`,
   and has a clean checkout before publishing with the temporary token.
6. Verify the new version and checksum on crates.io. Confirm that docs.rs builds
   the same version and that a clean consumer can compile both the default
   library and the optional CLI feature.
7. Create a GitHub release from the same tag using the matching changelog
   section. Mark it as the latest release.

Do not retry publication without first checking crates.io. A workflow can lose
its final status response after crates.io has already accepted the immutable
artifact.

## Release verification

Record these postconditions:

- crates.io reports the expected version;
- the downloaded crate checksum matches the published API response;
- docs.rs reports a successful build for the version;
- the Git tag and GitHub release point to the release commit;
- `cargo check` succeeds in a new temporary consumer project;
- native and dynamic async downstream examples compile on the declared MSRV;
- `cargo install edge-completions --version MAJOR.MINOR.PATCH --features cli`
  succeeds from crates.io.

Every third-party action is pinned to a full commit SHA.

## Rollback

Published crate files cannot be replaced or deleted. If a release is defective,
yank it only after confirming the affected version and impact, document why,
fix forward with a new version, and leave the original artifact auditable.
