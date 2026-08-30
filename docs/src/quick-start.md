# Quick Start

Park manages named, project-scoped development commands after the launching
terminal is gone. Run these commands from the project directory you want Park
to associate with the process.

## Start Commands

The primary launch form is `park <name> -- <command> [arguments...]`:

```bash
park dev -- pnpm dev
park worker -- cargo run --bin worker
```

The `--` separator marks the beginning of the managed command and its
arguments. Park launches the recorded executable argument vector without
implicitly invoking a shell.

Names are scoped to the canonical project directory. For example, `dev` in
`~/code/shop` and `dev` in `~/code/api` are independent records. A second
launch with the same name in the same project is rejected rather than silently
replacing the first record.

## Inspect Processes

List records for the current project or inspect one named record:

```bash
park ps
park status dev
park ps --json
park status dev --json
```

`ps` and `status` use the current project's canonical path. Records remain
available after their commands exit, allowing status and output to be inspected
later.

## Read Logs

Park retains stdout and stderr as separate append-only streams:

```bash
park logs dev
park logs dev --stdout
park logs dev --stderr
park logs dev --tail 100
park logs dev --head 20
park logs dev --grep ready
park logs dev --follow
```

The combined view is deterministic stdout followed by stderr. It does not claim
to preserve cross-stream event timing. `--grep` performs a literal substring
search on retained lines before `--head` or `--tail`; regular expressions are
not supported. With `--follow`, the retained initial output uses the requested
filters and later appended output is streamed until the process terminates.

Logs remain available after exit and also include output from later `start` or
`restart` operations, which append to the existing stream logs.

## Control The Lifecycle

Use the lifecycle commands to stop, restart, start, signal, remove, or clean
records:

```bash
park stop dev
park stop dev --force
park restart dev
park start dev
park signal dev TERM
park rm dev
park rm dev --keep-logs
park clean
```

`stop` sends `SIGTERM` to the managed process group, waits two seconds, and
escalates to `SIGKILL` if needed. `--force` sends `SIGKILL` immediately. This
process-group behavior is intended to avoid orphaned children from wrappers
such as `npm`, `pnpm`, and `cargo watch` where supported.

`restart` stops an active process when necessary and starts it again from the
recorded command. `start` is limited to retained terminal records. `rm` refuses
active records and removes their logs unless `--keep-logs` is supplied. `clean`
removes eligible terminal records and their logs across the user's Park state;
it never removes active records.

## Wait For A Condition

`wait` observes a record without taking a lifecycle lock:

```bash
park wait dev --state running
park wait dev --exit
park wait dev --match ready --timeout 30s
```

`--state` requires an exact persisted state. `--exit` matches any terminal
state. `--match` searches both retained stdout and stderr for a literal byte
substring, including historical output and output appended by later starts or
restarts. A timeout is a generic failure; a missing record remains a missing
record result.

## Script-Friendly Output

Park is non-interactive and supports stable JSON output on inspection commands:

```bash
park ps --json
park status dev --json
park logs dev --json
```

The exit codes distinguish common lifecycle outcomes:

- `0`: success
- `1`: generic failure
- `2`: command-line usage error
- `3`: missing record
- `4`: duplicate record
- `5`: invalid lifecycle state
