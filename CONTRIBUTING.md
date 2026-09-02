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

Documentation can be built and reviewed locally with the commands above.
