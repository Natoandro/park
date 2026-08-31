# Installation

Park is under active development and published through crates.io. Park currently
supports Unix; Windows is not yet supported.

The Rust package is named `park-cli`, while the installed executable is
`park`. The project uses Rust Edition 2024 and requires Rust 1.85 or newer.

## Install From crates.io

Install the latest published release:

```bash
cargo install park-cli
```

This installs the `park` executable from the `park-cli` package.

## Try The Latest Development Version

To try the latest version from the `master` branch, install directly from the
GitHub repository:

```bash
cargo install --git https://github.com/Natoandro/park.git --branch master park-cli
```

The `master` build may be unstable and can differ from the latest published
release.

## Install A Local Checkout

From the repository root, install the local package with:

```bash
cargo install --path .
```

The package uses bundled SQLite, so Park does not require a separately
installed system SQLite library for installation.

## Install The Agent Skill

Park's agent skill is installed separately from the `park` executable with the
[`npx skills`](https://skills.sh/) CLI. From a project where the skill should be
available, use the default interactive installation command:

```bash
npx skills add Natoandro/park --skill park
```

The CLI detects available agents and lets you choose when needed. Use `-g` for a
global installation:

```bash
npx skills add Natoandro/park --skill park -g
```

To target a specific agent, add `-a <agent>`, such as `-a opencode`. Use the
skill once without installing it with:

```bash
npx skills use Natoandro/park --skill park
```

This prints a prompt; add `--agent <agent>` to start a specific supported agent.
See [AI Agent
Integration](ai-agents.md) for the recommended agent workflow and skill
maintenance commands.

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
