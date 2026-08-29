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
park logs <name> [--tail N] [--head N] [--follow] [--grep PATTERN] [--stdout|--stderr]
park stop <name> [--force]
park restart <name>
park start <name>
park signal <name> <SIGNAL>
park rm <name> [--keep-logs]
park clean
park wait <name> (--state STATE | --match TEXT | --exit) [--timeout DURATION]
```

`park logs` is the canonical log interface. JSON output, stable exit codes, predictable lookup, and non-interactive operation are public requirements because Park is intended to work well in scripts and coding-agent workflows.

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

## Design Documents

- [High-level architecture](docs/architecture.md)
- [Low-level architecture](docs/low-level-architecture.md)
- [Implementation plan](docs/implementation-plan.md)
