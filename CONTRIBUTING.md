# Contributing

Park is a Unix-first Rust project. The package is `park-cli`, the installed
binary is `park`, and CI targets the current stable Rust toolchain.

## Local Setup

Clone the repository and install the pre-commit checks:

```bash
scripts/setup-hooks.sh
```

Commits run the version check, formatting check, and workspace compilation check.
Full tests, Clippy, and Docker E2E remain part of CI.

Build the binary from the repository root:

```bash
cargo build --locked --bin park
```

Run the test suite and checks used by CI:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

The end-to-end scenarios can be run in Docker with:

```bash
scripts/e2e.sh
```

## Documentation

The documentation website is an [mdBook](https://rust-lang.github.io/mdBook/)
whose source is under `docs/src`. Install the local mdBook executable once:

```bash
cargo install mdbook
```

Build the book from the repository root:

```bash
mdbook build docs
```

The generated site is written to `docs/book/` and is ignored by Git. Serve it
locally with live reload:

```bash
mdbook serve docs --open
```

The server is available at `http://localhost:3000` by default.

Update `docs/src/SUMMARY.md` when adding or moving pages. Keep the README as the
short project landing page and keep public CLI behavior aligned with the
documentation pages.

## Releases

Release tags must match the workspace version, for example `v0.1.0` for version
`0.1.0`. Update versions and `Cargo.lock` with one of:

```bash
scripts/bump-version.sh patch
scripts/bump-version.sh 0.2.0
```

The bump script also updates the current package version in
`docs/src/development.md`, and the version check rejects documentation that is
out of sync with the workspace.

After committing the version bump, preview and create an annotated release tag
from the workspace version:

```bash
scripts/tag-version.sh --dry-run
scripts/tag-version.sh --push
scripts/tag-version.sh --wait-ci --push
```

The tag script checks that all workspace package versions match, requires a
clean worktree on `master` synchronized with `origin/master`, and verifies that
the GitHub Actions `Test` workflow passed for that exact commit. It also refuses
to overwrite an existing tag. The script requires an authenticated `gh` CLI.
By default it creates the tag locally; `--push` also pushes it to `origin`.
`--dry-run` performs no Git mutations and can be combined with either mode, but
still performs every preflight check. By default, an unavailable or incomplete
Test workflow causes the script to fail. `--wait-ci` polls the Test workflow for
the exact `master` commit until it completes successfully before creating the
tag; a failed or cancelled run still stops the script.

Pushing a `v*` tag starts the `release.yml` workflow. It runs the reusable
`Test` workflow first. Only after all checks succeed do the binary release,
crates.io publication, and documentation deployment jobs start in parallel:

- `release.yml` builds and publishes the Linux `x86_64-unknown-linux-gnu`
  binary as a GitHub release asset with checksums.
- The release workflow publishes the workspace packages to crates.io.
- The release workflow builds and deploys the mdBook to GitHub Pages.

Documentation can be built and reviewed locally with the commands above.
