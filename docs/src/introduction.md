# Introduction

Park is a project-scoped background process manager for local development. It
runs a named command independently of the terminal that launched it, then
keeps the command's status and output available for later inspection and
control.

Park is designed for development machines and ad-hoc commands. It is not a
production service manager, container runtime, deployment system, task graph,
or general workflow engine. Managed commands run as ordinary host processes and
share the host filesystem, network, and other operating-system resources. Use a
container or virtual machine when isolation is required.

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

New process names must contain only ASCII letters, digits, `.`, `_`, `-`, and
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
