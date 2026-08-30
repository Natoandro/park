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
  |-- lifecycle and signal controller
  |-- stdout/stderr log writer
  `-- state and log storage
```

### CLI

The CLI parses commands, resolves the current project directory, connects to the per-user daemon, and renders either stable human output or JSON. It does not own managed child processes and should not need a foreground terminal to remain alive.

### Daemon

The daemon is started transparently on first use and owns the complete lifecycle of parked commands. It spawns commands in their recorded working directory, captures their output, monitors termination, and persists state transitions. It should be effectively idle when no requests or child events are pending.

### Registry and Storage

The registry persists process records and points to separate stdout and stderr logs. State follows XDG conventions: durable records and logs belong in the user state directory; the IPC socket and other ephemeral runtime coordination belong in `$XDG_RUNTIME_DIR` when available. Exited records remain inspectable until explicitly removed or cleaned.

## Process Lifecycle

```text
start request -> starting -> running -> stopping -> exited | failed | killed
```

Starting records the exact executable arguments and working directory before returning success. A name collision in the same project is an error unless the user explicitly chooses replacement behavior. Restart uses the recorded command, not a shell reconstruction.

Stopping is graceful by default: signal the managed process group, wait for a configured timeout, then escalate to forceful termination when necessary. Group signaling avoids orphaned children from wrappers such as `npm`, `pnpm`, and `cargo watch`.

## Public Behavior

- The short start form is `park <name> -- <command> [arguments...]`; `run` may be an alias but is not required for normal use.
- `ps`, `status`, and lifecycle commands resolve only within the current project's canonical path.
- Logs stay available after a command exits. Standard output and standard error are retained independently and can also be presented together in deterministic stdout-then-stderr order.
- `--json` is a first-class output mode for process inspection and should use documented, stable fields.
- Commands must be non-interactive unless explicitly requested. Stable exit semantics distinguish normal failure, missing records, duplicate records, and invalid transitions.

## Non-Goals

- Managing production services or requiring root privileges.
- Restarting user processes automatically after an operating-system reboot in the initial version.
- Requiring a manifest for routine use.
- Becoming a container runtime, deployment system, task graph, or general workflow engine.

An optional project configuration file can later describe repeatable named processes for `park up` and `park down`, but it must not displace the configuration-free workflow.
