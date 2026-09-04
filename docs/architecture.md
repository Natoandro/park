# High-Level Architecture

## Goal

Park manages ad-hoc, long-running development commands after their launching terminal is gone. The primary identity is a canonicalized `(project_directory, process_name)` pair, not a globally unique name.

## Components

```text
park CLI
  |  local IPC
  v
per-user Park daemon
  |-- process registry
  |-- process launcher and monitor
  |-- environment resolution and dotenv loading
  |-- lifecycle and signal controller
  |-- stdout/stderr log writer
  `-- state and log storage
```

### CLI

The CLI parses commands, resolves the current project directory, connects to the per-user daemon, and renders either stable human output or JSON. It does not own managed child processes and should not need a foreground terminal to remain alive.

### Daemon

The daemon is started transparently on first use and owns the complete lifecycle of parked commands. It spawns commands in their recorded working directory, captures their output, monitors termination, and persists state transitions. It should be effectively idle when no requests or child events are pending.

### Registry and Storage

The registry persists process records in a private SQLite database and points to separate append-only stdout and stderr logs. State follows XDG conventions: the database and logs belong in the user state directory; the IPC socket and other ephemeral runtime coordination belong in `$XDG_RUNTIME_DIR` when available. Exited records remain inspectable until explicitly removed or cleaned.

## Process Lifecycle

```text
start request -> starting -> running -> stopping -> exited | failed | killed
```

Starting records the exact executable arguments, working directory, and
environment inputs before returning success. A name collision in the same
project is an error unless the user explicitly chooses replacement behavior.
Restart uses the recorded command and environment inputs, not a shell
reconstruction or the daemon's ambient environment.

Stopping is graceful by default: send SIGTERM to the managed process group, wait two seconds, then escalate to SIGKILL when necessary. `--force` skips the grace period. Group signaling avoids orphaned children from wrappers such as `npm`, `pnpm`, and `cargo watch`.

## Public Behavior

- The short start form is `park <name> [--env-file <path>]... -- <command> [arguments...]`; `run` may be an alias but is not required for normal use.
- `start <name> -- <command>...` creates a new record when the complete key is unused; `start <name>` relaunches a retained terminal record.
- `ps`, `status`, and lifecycle commands resolve only within the current project's canonical path.
- Logs stay available after a command exits. Standard output and standard error are retained independently and can also be presented together in deterministic stdout-then-stderr order.
- `--json` is a first-class output mode for process inspection and should use documented, stable fields.
- `restart` reuses the recorded command and environment inputs, with `--recapture-env` as the explicit opt-in for a new client snapshot. `park env` inspects or updates explicit per-record environment values. `rm`/`clean` never remove an active process or its remaining process group.
- Commands must be non-interactive unless explicitly requested. Stable exit semantics distinguish normal failure, missing records, duplicate records, and invalid transitions.
- `wait --state`, `wait --exit`, and literal `wait --match` are observation operations. They poll without taking a lifecycle lock, honor an optional duration timeout, and cancel when a streaming client disconnects.
- IPC reads and writes are bounded by deadlines so a partial request or slow reader cannot stall daemon work or child-output capture.

## Non-Goals

- Managing production services or requiring root privileges.
- Restarting user processes automatically after an operating-system reboot in the current version.
- Requiring a manifest for routine use.
- Becoming a container runtime, deployment system, task graph, or general workflow engine.

## Platform Limit

Park is Unix-first, but verified process ownership across daemon restarts currently requires Linux `/proc` identity data. Non-Linux Unix builds do not claim equivalent restart/reconciliation safety until platform-specific identity checks are added.

Optional project configuration for repeatable named processes and `park up` / `park down` is not yet implemented, and must not displace the configuration-free workflow.
