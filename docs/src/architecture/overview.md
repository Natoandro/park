# Architecture Overview

Park is a project-scoped background process manager for local development. It
runs an exact command independently of the terminal that launched it, then
keeps the command's status and output available for later inspection and
control.

## Components and Boundaries

Park has two processes with distinct responsibilities:

```text
park CLI
    |  local Unix-socket IPC
    v
per-user Park daemon
    |-- process registry and persistence
    |-- process launcher and monitor
    |-- environment resolution and dotenv loading
    |-- lifecycle and signal controller
    |-- stdout/stderr capture
    `-- state and log storage
```

The CLI is the user-facing boundary. It parses commands, resolves the current
project directory, sends structured requests to the daemon, and renders stable
human or JSON output. It does not own the managed child process, so the
launching terminal can close without ending the command.

The daemon is started on demand and owns the managed process lifecycle. It
launches commands in their recorded working directories, captures their output,
monitors termination, and persists state transitions. When there are no
requests or child-process events, it should be effectively idle.

The registry stores process metadata and identifies the corresponding log
files. Metadata and logs are durable user-local state; the socket, lock, and
PID marker used to coordinate the daemon are runtime state. The storage
boundary is described in [Persistence](persistence.md), while process
ownership and state transitions are described in
[Process Lifecycle](process-lifecycle.md).

## Project-Scoped Identity

A managed process is identified by the pair:

```text
(canonical project directory, process name)
```

Names are not globally unique. For example, `dev` in `~/code/shop` and `dev`
in `~/code/api` are different records. Within one canonical project directory,
the same name cannot be launched again while its record is retained; a
duplicate is reported rather than silently replacing the existing record.

Before every lookup or creation, Park canonicalizes the caller's current
directory. The daemon repeats that resolution for project paths received over
IPC because serialized paths are untrusted. This prevents relative paths,
`.` components, and symlink aliases from creating separate namespaces. The
baseline is the invocation directory; Git-root discovery is not implicit.

## Daemon and IPC

The CLI communicates with one daemon per user through a local Unix socket. The
protocol is versioned, newline-delimited JSON. Requests and responses contain
request IDs, an operation, a target process key where applicable, and a
structured result. Requests also carry the client compatibility identity. The
internal `reexec` operation carries a candidate executable path and version;
safe handoff is not implemented yet. The client verifies the protocol version
and request ID before rendering a response.

If the socket is missing or refused, a CLI invocation may start a detached
daemon candidate and retry the connection. An advisory lock determines the
owner, so concurrent first clients do not create competing daemons. Only the
lock holder removes stale socket and PID-marker files and binds the endpoint.
Permission and protocol errors are returned instead of being treated as an
invitation to start another daemon.

IPC requests and responses have finite deadlines. Long-lived log-follow and
wait operations stream bounded frames, and a disconnected or slow client must
not prevent the daemon from draining child output or completing lifecycle
monitoring.

## Public Shape

The primary launch form is configuration-free and ad hoc:

```text
park <name> [--env-file <path>]... -- <command> [arguments...]
park restart <name> --recapture-env [--env-file <path>]...
park start <name> [--env-file <path>]... -- <command> [arguments...]
park env <name> [--set KEY=VALUE]... [--unset KEY]...
```

Daemon-management commands use an explicit namespace: `park daemon status`,
`park daemon reexec`, and `park daemon config`. Their public grammar is in
place. Status and config inspection are available; re-exec execution remains a
later milestone.

The command and argument vector are preserved exactly. Later `restart` and
`start` operations use those recorded arguments and the recorded working
directory rather than reconstructing a shell command. Inspection and lifecycle
operations are resolved within the current canonical project by default, and
retained records remain available after exit. A planned `ps --scope` extension
will allow explicit subtree or global inspection without broadening the target
scope of `status` or lifecycle operations.

The launch client captures its complete environment and sends it to the daemon.
The daemon, not the client, reads any requested dotenv files and resolves the
effective environment immediately before each spawn. The captured snapshot,
dotenv paths, and explicit `park env` edits are durable inputs; the merged
environment is deliberately not persisted.

Human output is non-interactive and script-friendly. JSON output and stable
lifecycle exit codes are first-class behavior rather than formatting layered on
top of human text.

## Non-Goals

Park manages ordinary development processes on a user's machine. It is not a
production service manager, deployment system, container runtime, task graph,
or general workflow engine. It does not require a manifest for routine use and
does not automatically restart processes after an operating-system reboot.

Managed commands are host processes and are not sandboxed or isolated from the
host filesystem, network, or other OS resources. Containers or virtual machines
are the appropriate tools when isolation is required.

Optional project configuration and orchestration are not yet implemented, and
must not displace the configuration-free launch workflow.
