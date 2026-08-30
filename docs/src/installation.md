# Installation

Park is currently under active development and is not yet published to
crates.io. The MVP is Unix-only; Windows support is deferred.

The Rust package is named `park-cli`, while the installed executable is
`park`. The project uses Rust Edition 2024 and requires Rust 1.85 or newer.

## Install From GitHub

Install the latest development version directly from the repository:

```bash
cargo install --git https://github.com/Natoandro/park.git park-cli
```

This installs the `park` executable from the `park-cli` package.

## Install A Local Checkout

From the repository root, install the local package with:

```bash
cargo install --path .
```

The package uses bundled SQLite, so the MVP does not require a separately
installed system SQLite library for installation.

## After crates.io Publication

After Park is published, the intended crates.io installation command is:

```bash
cargo install park-cli
```

That publication has not happened yet, so use the GitHub or local-checkout
command for now.

## Where Park Stores Data

Installation does not place process records in the project directory. Park uses
per-user XDG locations:

- Durable metadata: `$XDG_STATE_HOME/park/park.sqlite3`
- Durable logs: the adjacent `$XDG_STATE_HOME/park/logs` directory
- Ephemeral daemon files: `$XDG_RUNTIME_DIR/park`

If `XDG_STATE_HOME` is unset, durable state falls back to
`$HOME/.local/state/park`. If `XDG_RUNTIME_DIR` is unavailable, the socket,
lock, and PID marker use a private `runtime/park` directory under the durable
state directory.

## First Command

Run Park from the project directory whose process namespace you want to use:

```bash
park dev -- pnpm dev
```

The same name can be used in a different project, but launching `dev` again in
the same canonical project is a duplicate-record error. Park records the exact
command arguments and retains stdout and stderr separately for later inspection.
