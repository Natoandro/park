# Low-Level Architecture

## Data Model

Persist one record per canonical `(project_path, name)` key. A record requires:

- `name`, canonical `project_path`, and recorded `working_directory`
- exact executable plus argument vector, never a lossy display-string reconstruction
- `pid` and process-group identifier where supported
- `created_at`, `started_at`, and `exited_at`
- state, exit code, and termination signal
- paths to stdout and stderr logs

Use a separate human-readable command display only as derived presentation data. Process records must remain after exit so `status` and `logs` can report historical outcomes.

## Project Resolution

Resolve the caller's working directory to a canonical path before every lookup or creation. This prevents aliases such as symlinked paths, `.` components, and relative paths from creating separate namespaces. Git-root detection may be added only as an explicit, predictable policy; the invocation directory remains the baseline contract.

## IPC and Daemon Startup

The CLI connects through a per-user local Unix socket in `$XDG_RUNTIME_DIR/park`, or a private `runtime/park` directory under durable state when the runtime variable is unavailable. The daemon owns an advisory lock in the same directory and writes a PID marker after binding the socket. On a missing socket, the CLI launches a detached daemon candidate and retries the connection; only the lock holder removes stale socket and marker files and binds the endpoint.

Use versioned, newline-delimited JSON request/response messages with an operation, request ID, target process key, and structured result. Every response needs a machine-readable status so the CLI can render JSON without parsing human text. Streaming log follow is a long-lived IPC operation that forwards appended records and ends with the observed exit result when the process terminates.

## Spawn and Monitoring

1. Validate the canonical key is absent; explicit replacement is deferred until lifecycle semantics are complete.
2. Create durable record and log destinations before spawning.
3. On Linux, spawn a Park supervisor directly from the executable and argument vector in the recorded working directory. The supervisor starts the managed command without a shell and kills its process group when the daemon dies. Other Unix platforms currently spawn the managed command directly.
4. Create a new process group/session on supported Unix platforms.
5. Pipe stdout and stderr to independent asynchronous writers.
6. Mark `running` only after spawn succeeds; monitor the child and persist terminal state exactly once.

If spawn fails, retain a `failed` record with the diagnostic rather than leaving a partial running record. On daemon startup, reconcile non-terminal records against live PIDs/process groups and mark dead processes as terminal without discarding their logs.

## Lifecycle Semantics

`stop` transitions a running record to `stopping`, sends SIGTERM to its process group, waits for the configured grace period, and sends SIGKILL only if still alive. `--force` skips directly to forceful termination. `signal` validates the requested supported signal and targets the same group. Terminal transitions record either the exit code or signal, and no lifecycle action may silently overwrite a record.

`restart` stops the current group then spawns from the preserved executable, arguments, and working directory. It must serialize operations on an individual record so concurrent stop, restart, and remove requests cannot race. `rm` is distinct from `stop`: it removes metadata and, unless `--keep-logs` is set, log files only after the process is no longer active.

## Logging

Write stdout and stderr separately with append-only records. The combined `logs` view must preserve a deterministic ordering policy; a shared sequence number or timestamp assigned by the daemon is preferable to assuming independent file order. `--stdout` and `--stderr` read their respective streams. `--tail`, `--head`, and `--grep` operate on retained output; `--follow` observes new output and reports a clean terminal status.

Log rotation and retention are future configuration features. Their implementation must preserve the ability to inspect historical output associated with a retained record or explicitly state which history was pruned.

## Failure Boundaries

- On Linux, never trust a PID alone as proof that a record still owns a process. Reconciliation requires a matching `/proc` start time, process group, and session. Records without a verified identity are reconciled as no longer running.
- Persist state changes atomically enough that a daemon crash cannot make an active process appear removable or a terminal process appear running indefinitely.
- Treat a machine reboot as a reconciliation event, not an automatic restart request. Preserve the record and logs, then expose the process as no longer running.
- Keep log readers and slow IPC clients from blocking child-output draining; otherwise a verbose command can deadlock on full pipes.
