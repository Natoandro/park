# Park

[![Crates.io](https://img.shields.io/crates/v/park-cli.svg)](https://crates.io/crates/park-cli)
[![Crates.io downloads](https://img.shields.io/crates/d/park-cli.svg)](https://crates.io/crates/park-cli)
[![Documentation](https://docs.rs/park-cli/badge.svg)](https://docs.rs/park-cli)
[![CI](https://github.com/Natoandro/park/actions/workflows/test.yml/badge.svg)](https://github.com/Natoandro/park/actions/workflows/test.yml)
[![License](https://img.shields.io/crates/l/park-cli.svg)](https://github.com/Natoandro/park/blob/master/LICENSE)

**Keep local development processes running, visible, and under control.**

Park runs a named command independently of the terminal that launched it, then
keeps its status and output available for later inspection and control. Start a
server, close the terminal, and come back to the same process from the same
project later.

[Install Park](#installation) · [Try the quick start](#quick-start) · [Read the docs](docs/src/introduction.md)

Park is a small, ordinary CLI for developers who need a better alternative to
leaving terminals open, using `nohup`, or rebuilding a process by hand. Coding-
agent integration is available when useful, but is not part of the core setup.

## Why Park?

- **Leave the terminal behind:** local servers, workers, and watchers keep running after the launching shell closes.
- **Keep names scoped to projects:** `dev` in one project is independent from `dev` in another.
- **Return to the evidence:** status and separate stdout/stderr logs remain available after a process exits.
- **Control the whole process tree:** stop and restart commands target the managed process group where the platform supports it.
- **Automate safely:** stable JSON output, wait conditions, and lifecycle exit codes work in scripts and tooling.

Park is a development-machine tool, not a production supervisor, container runtime,
or deployment system.

The Rust package is `park-cli`; the installed executable is `park`.

> [!NOTE]
> Park is under active development. The latest published release is available
> from crates.io.

## Installation

Install the latest published release from crates.io:

```bash
cargo install park-cli
```

This installs the `park` executable from the `park-cli` package.

Park currently supports Unix systems. Linux has the strongest process-ownership
and recovery guarantees; Windows is not yet supported.

To try the latest development version from the `master` branch:

```bash
cargo install --git https://github.com/Natoandro/park.git --branch master park-cli
```

The `master` build may be unstable and can differ from the latest published
release. For a local checkout, use `cargo install --path .` instead.

## Everyday Development

Use Park anywhere you would otherwise keep a terminal tab open for a long-running
local command:

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

This is especially useful for:

- Development servers and API backends you revisit throughout the day.
- Background workers, queues, and local consumers that should survive shell changes.
- File watchers and documentation preview servers.
- Temporary local services whose output you need to inspect after failure.
- Shell scripts that need to wait for a process to become ready before continuing.

Park does not require a manifest or project configuration for this workflow. The
name is associated with the directory where you run the command, so the same
short names can be reused across projects.

## AI Agent Integration

Park provides a canonical skill for coding agents through the [`npx skills`](https://skills.sh/)
CLI. The skill teaches agents to inspect existing records, use project-scoped
names, wait for readiness, read retained logs, and avoid disrupting processes
owned by another actor.

Install it for the current project. The `npx skills` CLI detects available agents
and lets you choose when needed:

```bash
npx skills add Natoandro/park --skill park
```

Install it globally instead:

```bash
npx skills add Natoandro/park --skill park -g
```

To target a specific agent, add `-a <agent>`, for example `-a opencode`. Use the
skill for one session without installing it with:

```bash
npx skills use Natoandro/park --skill park
```

The one-off command prints a prompt; add `--agent <agent>` to start a specific
supported agent.

Discover the integration guide from the installed binary:

```bash
park help --skill
park help --skill --json
```

See [AI Agent Integration](docs/src/ai-agents.md) for other supported agents,
updates, removal, and the recommended workflow.

## Quick Start

From a project directory, park a long-running command:

```bash
park dev -- pnpm dev
park worker -- cargo run --bin worker
```

Then close the terminal and manage the processes later from the same project:

```bash
park ps
park status dev
park logs dev --tail 100
park logs dev --follow
park restart dev
park stop dev
```

The same name can be used in separate projects. `dev` in `~/code/shop` and `dev` in `~/code/api` are independent process records.

## Use Cases

Park is intended for:

- Keeping local development servers, workers, and watchers running after the launching terminal closes.
- Sharing process visibility between human developers and coding agents working in the same project. A Park-managed process launched by one actor can be discovered, inspected, and controlled by the others without sharing a terminal or starting duplicate services.
- Inspecting status and stdout/stderr later, including after a command exits, for debugging and handoff.
- Scripted and agent-driven workflows that need stable JSON output, wait conditions, retained logs, and predictable lifecycle exit codes.

## Feature Status

### Available

- [x] Configuration-free launch of exact executable argument vectors.
- [x] Canonical project-scoped process names with duplicate protection.
- [x] On-demand per-user daemon management independent of the launching terminal.
- [x] Dedicated process groups and conservative Linux process-ownership checks.
- [x] Durable process records with separate, retained stdout and stderr logs.
- [x] Status, log inspection, filtering, following, signals, graceful stop, restart, start, removal, cleanup, and wait operations.
- [x] Stable JSON output and lifecycle exit codes for scripts and coding agents.
- [x] Captured client environments, repeatable dotenv files, and per-record
  environment inspection and updates.

### Not Yet Implemented

- [ ] Automatic restart policies with backoff and retry limits.
- [ ] Filesystem-triggered restarts for development workflows.
- [ ] Additional coordination support for shared human-and-agent workflows.
- [ ] Optional project configuration with `park up` and `park down`.
- [ ] Broader platform-specific process-ownership and lifecycle guarantees.
- [ ] Log rotation, retention, and pruning.
- [ ] Graceful daemon upgrades that preserve active managed processes.

See the [roadmap](docs/implementation-plan.md#roadmap) for the complete prioritized list.

## Feedback And Contributions

Park is still early, so real-world feedback is especially valuable. [Open an
issue](https://github.com/Natoandro/park/issues) if you tried Park, found a bug,
hit installation or platform friction, or have a concrete development workflow
that Park does not support.

Pull requests are welcome. For substantial behavior or feature changes, please
open an issue first so the change can be discussed in the context of Park's
scope. Keep pull requests focused and include tests and documentation when the
public behavior changes. See [Contributing](CONTRIBUTING.md) for local checks and
development instructions.

## Development

Enable the repository's pre-commit version check once per checkout:

```bash
scripts/setup-hooks.sh
```

Release tags must match the workspace version, for example `v0.1.0` for version
`0.1.0`.

Bump all workspace package versions and refresh `Cargo.lock`:

```bash
scripts/bump-version.sh patch
scripts/bump-version.sh 0.2.0
```

See [Contributing](CONTRIBUTING.md) for local build, test, documentation, and
release instructions.

## Intended Interface

```text
park <name> [--env-file <path>]... -- <command> [arguments...]
park ps [--json]
park status <name> [--json]
park logs <name> [--tail N|--head N] [--follow] [--grep PATTERN] [--stdout|--stderr] [--json]
park stop <name> [--force]
park restart <name>
park restart <name> --recapture-env [--env-file <path>]...
park start <name>
park start <name> [--env-file <path>]... -- <command> [arguments...]
park signal <name> <SIGNAL>
park rm <name> [--keep-logs]
park clean
park wait <name> (--state STATE | --match TEXT | --exit) [--timeout DURATION]
park env <name> [--json]
park env <name> [--set KEY=VALUE]... [--unset KEY]... [--json]
park daemon status [--json]
park daemon config [--json]
park help
park help --skill [--json]
```

`park logs` is the canonical log interface. `park daemon status` and `park daemon config` inspect the per-user daemon without selecting a project. JSON output, stable exit codes, predictable lookup, and non-interactive operation are public requirements because Park is intended to work well in scripts and coding-agent workflows.

`stop` sends SIGTERM to the managed process group and escalates to SIGKILL after a two-second grace period; `--force` sends SIGKILL immediately. `signal` accepts `HUP`, `INT`, `QUIT`, `TERM`, `USR1`, `USR2`, `STOP`, `CONT`, and `KILL`, with an optional `SIG` prefix. Numeric signal values are not accepted. `restart` stops an active process before starting it again from its recorded command and environment inputs. `--recapture-env` captures the calling client's environment and enables repeatable `--env-file` arguments. `start` without a command starts a retained terminal record; `start <name> -- <command>...` creates a record when the key is unused. Restart and start append to the existing stream logs.

`rm` refuses active records or records whose managed process group is still present, and removes logs unless `--keep-logs` is supplied. `clean` removes terminal records with no remaining managed process group across the user's Park state; it never removes active records.

`wait --state` succeeds when the persisted state exactly matches the requested state. `wait --exit` matches any terminal state. `wait --match` performs a literal byte-substring search across both retained stdout and stderr, including output appended by later starts or restarts. Conditions are checked immediately and then polled; `--timeout` accepts `ms`, `s`, or `m` values, and a timeout is a generic failure (exit code `1`). A missing record remains exit code `3`.

The client captures its complete environment when a record is created. `--env-file`
can be repeated; the daemon reads those dotenv files in order for every spawn,
and `park env` displays or updates explicit per-record values. The merged
environment is not persisted. `restart` rereads the recorded files and accepts
`--recapture-env` when the caller also wants to replace the stored client
snapshot. The flag also enables repeatable `--env-file` arguments; supplied
paths replace the stored dotenv file list, while omitting them retains the
existing list. `start <name> -- <command>...` creates a new record when the key is
unused; otherwise the existing record is not silently replaced.

Without `--stdout` or `--stderr`, logs are combined deterministically as stdout followed by stderr. `--grep` performs a literal substring search on retained lines before `--head` or `--tail` is applied; regular expressions are not supported. With `--follow`, the initial retained output honors these filters and subsequent output is streamed as it is appended.

The operation subcommands also accept long-option aliases such as `park --status dev`, while the readable subcommand form remains canonical. The `--` separator marks the start of the managed command and its arguments. Process names must contain only ASCII letters, digits, `.`, `_`, `-`, and `:`. Names remain project-scoped, and Park does not reserve operation words, so names such as `status` and `--status` are valid when used in the launch form, for example `park status -- ./server`.

## Exit Codes

- `0`: success
- `1`: generic failure
- `2`: command-line usage error
- `3`: missing record
- `4`: duplicate record
- `5`: invalid lifecycle state

## Scope

Park is for development machines, not production service management. It deliberately does not replace systemd, Docker Compose, Kubernetes, or a workflow engine. It does not currently provide process isolation or sandboxing: managed commands run as host processes and share the host filesystem, network, and other OS resources. Use containers or a virtual machine when isolation is required. Its core responsibility is narrow: named, project-scoped development commands with persistent logs and straightforward lifecycle control.

Park is conceptually related to Unix process supervisors such as [Supervisor](https://github.com/Supervisor/supervisor) and development task runners such as [Whiz](https://github.com/zifeo/whiz), but serves a different purpose. Supervisor manages configured services, while Whiz runs configured task graphs. Park is configuration-free by default and manages named, project-scoped ad-hoc commands with persistent status and logs after the launching terminal exits.

## State and Logs

Park stores process metadata in a private SQLite database at `$XDG_STATE_HOME/park/park.sqlite3`, falling back to `$HOME/.local/state/park/park.sqlite3`. Standard output and standard error remain separate append-only files under the adjacent `logs` directory. The daemon socket, lock, and PID marker are ephemeral files under `$XDG_RUNTIME_DIR/park`, with a state-directory fallback when the runtime directory is unavailable.

Park's strongest process-ownership checks are implemented on Linux using `/proc` start times, process groups, and sessions. Other Unix targets retain the Unix interface but cannot safely verify process identity across daemon restarts yet.

## Configuration

Park remains configuration-free for ordinary launches. The optional global TOML
file is `$XDG_CONFIG_HOME/park/config.toml`, falling back to
`$HOME/.config/park/config.toml`; missing files use built-in defaults. The
configuration format defines daemon re-exec and managed-process restart
policies, but those config-driven CLI behaviors are still under development.
See the [configuration guide](docs/src/configuration.md) for the file format,
defaults, and validation rules.

## Design Documents

- [High-level architecture](docs/architecture.md)
- [Low-level architecture](docs/low-level-architecture.md)
- [Implementation plan](docs/implementation-plan.md)
- [Code review checklist](docs/review-checklist.md)
- [End-to-end user stories](docs/e2e-user-stories.md)
- [Docker e2e test guide](docs/e2e-docker.md)
