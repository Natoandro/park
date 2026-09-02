# Park

[![Crates.io](https://img.shields.io/crates/v/park-cli.svg)](https://crates.io/crates/park-cli)
[![Crates.io downloads](https://img.shields.io/crates/d/park-cli.svg)](https://crates.io/crates/park-cli)
[![Documentation](https://docs.rs/park-cli/badge.svg)](https://docs.rs/park-cli)
[![CI](https://github.com/Natoandro/park/actions/workflows/test.yml/badge.svg)](https://github.com/Natoandro/park/actions/workflows/test.yml)
[![License](https://img.shields.io/crates/l/park-cli.svg)](https://github.com/Natoandro/park/blob/master/LICENSE)

**Keep local development processes running, visible, and under control.**

Park is a project-scoped background process manager for local development. It
runs a named command independently of the terminal that launched it, then
keeps the command's status and output available for later inspection and
control. Start a server, close the terminal, and return to the same process from
the same project later.

```bash
cargo install park-cli
```

[Quick start](quick-start.md) · [Installation](installation.md) · [Commands](commands/index.md)

Park is a small CLI for developers who need a better alternative to leaving
terminals open, using `nohup`, or rebuilding a process by hand. Coding-agent
integration is available when useful, but is not part of the core setup.

## Why Park?

- **Terminal-independent:** local servers, workers, and watchers keep running after the launching shell closes.
- **Project-scoped:** the name `dev` in one project is independent from `dev` in another.
- **Inspectable:** status and separate stdout/stderr logs remain available after exit.
- **Safe to control:** lifecycle operations target the managed process group where supported.
- **Scriptable:** JSON output, wait conditions, and stable exit codes support automation.

Park is for development machines. It is not a production service manager,
container runtime, deployment system, task graph, or general workflow engine.

## A Normal Developer Workflow

```bash
# From the project directory
park api -- ./bin/api --port 3000
park worker -- cargo run --bin worker

# Close the terminal, then return later
park ps
park status api
park logs api --tail 100
park restart api
park stop api
```

Use Park for development servers, local workers, file watchers, preview servers,
temporary services, and scripts that need to wait for readiness. No project
manifest is required for this workflow.

## With Coding Agents

Park also gives developers and coding agents a shared process vocabulary. An
agent can inspect an existing record before launching a duplicate, wait for a
readiness message, read retained logs, and avoid stopping a process owned by
someone else. The [AI Agent Integration](ai-agents.md) guide covers the optional
skill installation; the `park` CLI remains the operational interface.

## At A Glance

| Need | Park provides |
| --- | --- |
| Keep a local command alive after closing a shell | Detached, on-demand per-user daemon |
| Find the process later | Project-scoped names and durable records |
| Diagnose a failed command | Retained, separate stdout and stderr logs |
| Wait before running the next step | State, exit, and literal log-match conditions |
| Integrate with scripts | Stable JSON output and lifecycle exit codes |

The remainder of this page explains the technical contract. Start with the
[quick start](quick-start.md) if you want to try Park first.

## Feedback And Contributions

Park is still early, so real-world feedback is especially valuable. [Open an
issue on GitHub](https://github.com/Natoandro/park/issues) if you tried Park,
found a bug, hit installation or platform friction, or have a concrete workflow
that Park does not support.

[Pull requests](https://github.com/Natoandro/park/pulls) are welcome. For
substantial behavior or feature changes, open an issue first so the change can
be discussed in the context of Park's scope. See the [contribution
guide](https://github.com/Natoandro/park/blob/master/CONTRIBUTING.md) for local
checks and development instructions.

## The Core Workflow

Launch a command from a project directory with a name and the `--` separator:

```bash
park dev -- pnpm dev
```

Park records the exact executable and argument vector and the command's working
directory. The command can then be inspected or controlled without keeping the
launching terminal open:

```bash
park ps
park status dev
park logs dev --tail 100
park stop dev
```

The daemon starts on demand and is independent of the terminal. Process records
and logs remain after a command exits, so a later `status` or `logs` operation
can show the historical outcome. `restart` uses the recorded command rather
than reconstructing a shell command, and `start` can start a retained terminal
record.

## Project-Scoped Names

A process is identified by the canonical project directory and its name. Names
are not global across the user's projects:

```text
~/code/shop + dev  -> one record
~/code/api  + dev  -> another record
```

Process names must contain only ASCII letters, digits, `.`, `_`, `-`, and
`:`, with no whitespace.

Park canonicalizes the invocation directory for lookups and creation. This
means relative paths, `.` components, and symlink aliases do not silently create
separate project namespaces. A duplicate name in the same canonical project is
rejected rather than silently replacing the existing record.

## Output And Automation

Standard output and standard error are captured separately and retained in
separate append-only logs. `park logs` is the canonical log interface:

```bash
park logs dev --stdout
park logs dev --stderr
park logs dev --grep ready --tail 20
park logs dev --follow
```

Without a stream option, the combined view presents stdout followed by stderr
deterministically. This ordering is stable but does not reconstruct the timing
between the two streams. `--grep` is a literal substring filter, not a regular
expression search.

Park is non-interactive by default and provides stable exit codes and first-
class JSON output for scripts and coding-agent workflows:

```bash
park ps --json
park status dev --json
park logs dev --json
```

The lifecycle exit codes are:

- `0`: success
- `1`: generic failure
- `2`: command-line usage error
- `3`: missing record
- `4`: duplicate record
- `5`: invalid lifecycle state

## State And Platform Scope

Park stores process metadata in SQLite at
`$XDG_STATE_HOME/park/park.sqlite3`, falling back to
`$HOME/.local/state/park/park.sqlite3`. The adjacent `logs` directory contains
the stdout and stderr files. The daemon socket, lock, and PID marker are
ephemeral files under `$XDG_RUNTIME_DIR/park`; when the runtime directory is
unavailable, Park uses a runtime directory under its durable state directory.

Park is Unix-first. Linux provides the strongest process-ownership checks,
using `/proc` process start times together with process groups and sessions.
Other Unix targets retain the Unix interface but do not yet claim equivalent
process-identity verification across daemon restarts. Windows support is not yet
implemented.
