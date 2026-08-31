# Persistence and IPC State

Park separates durable process history from ephemeral daemon coordination. The
durable layer lets status and logs remain inspectable after a command exits or
the daemon restarts. The runtime layer is only used to find and coordinate the
current daemon.

## XDG Layout

Durable state is stored under the XDG state directory:

```text
$XDG_STATE_HOME/park/
|-- park.sqlite3
|-- logs/
`-- runtime/                 # fallback only
```

When `XDG_STATE_HOME` is unavailable, Park uses
`$HOME/.local/state/park`. The SQLite database is private to the user, and the
log files live below the adjacent `logs` directory.

The daemon's Unix socket, advisory lock, and PID marker are placed in
`$XDG_RUNTIME_DIR/park` when `XDG_RUNTIME_DIR` is available. Otherwise, Park
uses the private `runtime/park` directory under durable state. Runtime files
are coordination artifacts, not the source of process history: stale socket or
PID-marker files do not establish daemon ownership because the kernel-managed
advisory lock does.

## Records and Identity

SQLite stores one record for each canonical `(project_path, name)` key. A
record includes:

- The ASCII process name and lossless canonical project path.
- The recorded working directory.
- The exact executable and argument vector.
- Process and process-group identifiers where supported.
- Creation, start, and exit timestamps.
- Lifecycle state, exit code, and termination signal.
- Derived paths to the stdout and stderr logs.

The name is not a global identifier. Every registry operation requires the
complete project-and-name key, and the database enforces uniqueness for that
pair. Canonicalization happens before local lookups and is repeated by the
daemon for project paths carried over IPC. Record reads validate lifecycle
fields, identity columns, working-directory/key consistency, and derived log
locations before a record can be listed, reconciled, or removed.

The exact argument vector is authoritative. A human-readable command string is
only derived presentation data and is never used to reconstruct a restart.

The current SQLite schema is version 1, recorded with SQLite's
`PRAGMA user_version`. The normalized tables are:

- `process_records` stores the process key, executable, lifecycle identifiers,
  timestamps, state, exit information, and failure reason as individual fields.
- `process_arguments` stores one raw argument BLOB per zero-based position,
  keyed by the process record digest.

The working directory and stdout/stderr log paths are derived from the canonical
process key and are not duplicated in SQLite.

## Logs

Standard output and standard error are stored in separate append-only files.
Both destinations are created independently and exclusively before the command
is spawned. Capture tasks append raw bytes to their respective streams while
the daemon monitors the child independently, so a verbose command cannot block
because a log reader is slow.

The separate files preserve the two streams and remain available with the
record after exit. The combined `logs` view is deterministic `stdout` followed
by `stderr`; it does not claim to reproduce cross-stream event timing because
the files do not contain a shared event sequence. Invalid UTF-8 is retained in
the files and is represented with replacement when returned as JSON text.

Restarts and starts append to the existing stream logs. Literal filtering is
performed on retained lines before `head` or `tail`; follow operations send a
bounded initial snapshot and then stream appended content without allowing the
client to block capture.

Log rotation, retention, pruning, compression, structured log metadata,
SQLite-backed log indexes or chunks, and external export are not part of the
current storage design. If history management is implemented, it must state
what was pruned and keep retained records internally consistent.

## Transactions and Recovery

Launch creates the log destinations and persists a `starting` record before
spawning. A successful spawn updates the record to `running`; a spawn failure
keeps the record as `failed` with its diagnostic instead of leaving a partial
running record. SQLite transactions and journaling protect metadata mutations
from exposing partially applied lifecycle changes.

Terminal records and their logs remain until an explicit `rm` or eligible
`clean` operation. Removal is conservative: active records and records whose
managed process group may still exist are not removed. `--keep-logs` can retain
the files when metadata is removed.

If an interrupted pre-spawn attempt leaves key-derived logs without a record,
the next launch for that key may remove only those stale destinations before
recreating them. A daemon restart reconciles records that are marked active;
dead or unverifiable processes become terminal without discarding their logs.

## Local IPC

The CLI and daemon exchange versioned newline-delimited JSON over the per-user
Unix socket. Requests carry an operation, request ID, and target key where
needed. Responses carry machine-readable success or error status so JSON CLI
output is rendered from response data rather than parsed human text.

The CLI starts a daemon only for a missing or refused socket. The daemon owns
an advisory lock in the runtime directory, removes stale endpoint markers only
after acquiring that lock, and writes its PID marker after binding the socket.
Bounded request-read and response-write deadlines ensure that partial requests,
slow readers, and disconnected streaming clients do not stall unrelated
process management or output capture.
