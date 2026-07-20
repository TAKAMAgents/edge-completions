# Releasing

Publishing a crates.io version is permanent. Perform these steps from a clean,
reviewed commit and never pass a registry token on the command line.

## First release

1. Confirm the crate name and repository URL resolve to this project.
2. Update `CHANGELOG.md` and verify the version in `Cargo.toml`.
3. Run the full local validation suite from `README.md` on Rust 1.86 and stable.
4. Inspect `cargo package --list` and unpacked package contents.
5. Run `cargo publish --dry-run`.
6. Confirm crates.io account ownership and use Cargo's configured credential provider.
7. Run `cargo publish` once.
8. Verify the crates.io page, docs.rs build, checksum, repository links, and
   installation from a clean consumer project.
9. Create and push the matching signed `v0.1.0` tag and GitHub release.

The first release is manual because crates.io trusted publishing cannot be
configured until the crate exists.

## Later releases

Trusted publishing is bound to `TAKAMAgents/edge-completions`, `publish.yml`,
and the GitHub `release` environment. The workflow obtains a short-lived token
with OIDC; the repository stores no crates.io secret.

1. Merge a reviewed version and changelog update after CI passes.
2. Create an annotated `vMAJOR.MINOR.PATCH` tag on that exact `main` commit.
3. Push the tag and approve the protected `release` environment deployment.
4. The workflow verifies that the tag matches `Cargo.toml`, belongs to `main`,
   and has a clean checkout before publishing with the temporary token.
5. Verify crates.io, docs.rs, checksum, and a clean consumer installation.

Every third-party action is pinned to a full commit SHA.

## Rollback

Published crate files cannot be replaced or deleted. If a release is defective,
yank it, document why, fix forward with a new version, and leave the original
artifact auditable.
