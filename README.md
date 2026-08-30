# Park

Park is a project-scoped background process manager for local development. It runs a named command independently of the terminal that launched it, then keeps its status and output available for later inspection and control.

The Rust package is `park-cli`; the installed executable is `park`.

```bash
cargo install park-cli
```

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

## Intended Interface

```text
park <name> -- <command> [arguments...]
park ps [--json]
park status <name> [--json]
park logs <name> [--tail N|--head N] [--follow] [--grep PATTERN] [--stdout|--stderr] [--json]
park stop <name> [--force]
park restart <name>
park start <name>
park signal <name> <SIGNAL>
park rm <name> [--keep-logs]
park clean
park wait <name> (--state STATE | --match TEXT | --exit) [--timeout DURATION]
```

`park logs` is the canonical log interface. JSON output, stable exit codes, predictable lookup, and non-interactive operation are public requirements because Park is intended to work well in scripts and coding-agent workflows.

`stop` sends SIGTERM to the managed process group and escalates to SIGKILL after a two-second grace period; `--force` sends SIGKILL immediately. `signal` accepts `HUP`, `INT`, `QUIT`, `TERM`, `USR1`, `USR2`, `STOP`, `CONT`, and `KILL`, with an optional `SIG` prefix. Numeric signal values are not accepted. `restart` stops an active process before starting it again from its recorded command, while `start` only starts a retained terminal record. Restart and start append to the existing stream logs.

`rm` refuses active records or records whose managed process group is still present, and removes logs unless `--keep-logs` is supplied. `clean` removes terminal records with no remaining managed process group across the user's Park state; it never removes active records.

`wait --state` succeeds when the persisted state exactly matches the requested state. `wait --exit` matches any terminal state. `wait --match` performs a literal byte-substring search across both retained stdout and stderr, including output appended by later starts or restarts. Conditions are checked immediately and then polled; `--timeout` accepts `ms`, `s`, or `m` values, and a timeout is a generic failure (exit code `1`). A missing record remains exit code `3`.

Without `--stdout` or `--stderr`, logs are combined deterministically as stdout followed by stderr. `--grep` performs a literal substring search on retained lines before `--head` or `--tail` is applied; regular expressions are not supported. With `--follow`, the initial retained output honors these filters and subsequent output is streamed as it is appended.

The operation subcommands also accept long-option aliases such as `park --status dev`, while the readable subcommand form remains canonical. The `--` separator marks the start of the managed command and its arguments. Process names are opaque command-line arguments: Park does not reserve operation words or impose lexical name validation, so names such as `status` and `--status` are valid when used in the launch form, for example `park status -- ./server`.

## Exit Codes

- `0`: success
- `1`: generic failure
- `2`: command-line usage error
- `3`: missing record
- `4`: duplicate record
- `5`: invalid lifecycle state

## Scope

Park is for development machines, not production service management. It deliberately does not replace systemd, Docker Compose, Kubernetes, or a workflow engine. Its core responsibility is narrow: named, project-scoped development commands with persistent logs and straightforward lifecycle control.

## State and Logs

Park stores process metadata in a private SQLite database at `$XDG_STATE_HOME/park/park.sqlite3`, falling back to `$HOME/.local/state/park/park.sqlite3`. Standard output and standard error remain separate append-only files under the adjacent `logs` directory. The daemon socket, lock, and PID marker are ephemeral files under `$XDG_RUNTIME_DIR/park`, with a state-directory fallback when the runtime directory is unavailable.

The MVP's strongest process-ownership checks are implemented on Linux using `/proc` start times, process groups, and sessions. Other Unix targets retain the Unix interface but cannot safely verify process identity across daemon restarts yet.

## Design Documents

- [High-level architecture](docs/architecture.md)
- [Low-level architecture](docs/low-level-architecture.md)
- [Implementation plan](docs/implementation-plan.md)
- [Code review checklist](docs/review-checklist.md)
