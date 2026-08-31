# Implementation Plan

This proposed Unix-first Rust project preserves Park's public invariants: project-scoped identity, terminal-independent processes, durable separate logs, and script-friendly lifecycle control. Phase 0 records the Unix-only platform and toolchain decisions. Each phase ends with focused checks before the next phase begins.

## Phase 0: Foundation Decisions

Phase 0 establishes the platform, toolchain, and dependency policy before implementation. Do not add a dependency until its use is approved. `clap`, `serde`, and `serde_json` are conventional CLI/serialization dependencies; all other third-party crates require explicit approval before they appear in `Cargo.toml`.

- [x] Confirm the current supported platform: Unix-only.
- [x] Confirm the Rust toolchain policy: Edition 2024 and MSRV 1.85.
- [x] Approve the async/process strategy: Tokio.
- [x] Approve the persistence strategy: SQLite process metadata with append-only log files.
- [x] Approve Unix process-group, signal, and advisory-lock support through `nix`.
- [x] Record each approved crate, version policy, purpose, and rejected alternative in this document.

Recorded decisions:

- Current platform: Unix-only; Windows support is not yet implemented.
- Toolchain: Edition 2024 with MSRV 1.85.
- Async runtime and local IPC: `tokio`; synchronous threads and standard-library sockets were rejected for the current version.
- Durable metadata: SQLite backed by `rusqlite`; process output remains in separate append-only files.
- Unix process groups, signals, and advisory locking: `nix`; internal FFI and a separate `fs2` dependency were rejected.
- Errors: `thiserror`; `anyhow` and fully internal error types were rejected in favor of typed stable outcomes.
- Time: epoch timestamps with internal formatting; `time` and `chrono` were rejected for the current version.
- SQLite: bundled `rusqlite`; system SQLite linking was rejected to avoid an installation-time system dependency.

## Phase 1: Workspace and Public Contract

- [x] Create the `park-cli` Cargo package with a `park` binary target.
- [x] Set Edition 2024 and the minimum supported Rust version of 1.85.
- [x] Add only approved dependencies; commit `Cargo.lock` for reproducible application builds.
- [x] Define CLI parsing for `park <name> -- <command> [args...]` and explicit subcommands.
- [x] Reserve `run` as an optional alias without making it the primary invocation.
- [x] Define a shared command-result type for human output, JSON output, and exit mapping.
- [x] Assign and document stable lifecycle exit codes: success, generic failure, missing record, duplicate record, and invalid state.
- [x] Add `--json` to inspection commands from the first implementation rather than retrofitting it.
- [x] Add unit tests for parsing ambiguous argument boundaries and JSON schema snapshots.
- [x] Run the project formatter, linter, unit tests, and build commands once tooling exists.

Phase 1 syntax decisions:

- The readable subcommand form is canonical; each operation also accepts a `--<operation>` alias.
- The short launch form is selected by a `--` separator immediately after the ASCII process name.
- New names use only ASCII letters, digits, `.`, `_`, `-`, and `:`, with no whitespace. Operation words are not reserved, and names are passed as one command-line argument with normal shell/OS argument-boundary rules.
- `run` is an optional explicit launch alias, not a requirement.
- Lifecycle result codes are `0` for success, `1` for generic failure, `3` for missing records, `4` for duplicate records, and `5` for invalid state. CLI usage errors use `2`.

## Phase 2: Domain Model and Project Resolution

- [x] Model `ProcessKey` as canonical project path plus process name.
- [x] Model stored executable, exact argument vector, recorded working directory, timestamps, process identifiers, terminal outcome, and log locations.
- [x] Model legal states: `starting`, `running`, `stopping`, `exited`, `failed`, and `killed`.
- [x] Implement validated state transitions and reject invalid lifecycle operations explicitly.
- [x] Canonicalize the caller's current directory for all lookups and creations.
- [x] Make invocation-directory scoping the initial policy; do not add implicit Git-root behavior.
- [x] Add tests for relative paths, symlink aliases, duplicate names in one project, and identical names in distinct projects.
- [x] Verify records cannot be addressed by name alone.

Phase 2 implementation decisions:

- `ProjectPath` is constructed only from canonicalized existing directories. The current working directory is resolved directly; Git-root discovery is not performed.
- `ProcessKey` owns the canonical `ProjectPath` and process name. Process names use ASCII letters, digits, `.`, `_`, `-`, and `:`. Registry access accepts only a complete `ProcessKey`, never a name alone.
- New records begin in `starting`; valid transitions cover successful startup, graceful stopping, natural failure, and forceful termination. Terminal states cannot transition further.
- The in-memory registry rejects duplicate canonical keys while allowing identical names under distinct project paths. Durable storage was implemented in Phase 3.

## Phase 3: State Layout and Persistence

- [x] Resolve durable state under `XDG_STATE_HOME` or its XDG fallback.
- [x] Resolve the IPC directory under `XDG_RUNTIME_DIR`, with a documented safe fallback when it is unavailable.
- [x] Define stable on-disk directory and file naming that does not expose raw project paths as unsafe filenames.
- [x] Implement SQLite record writes and crash-safe transaction semantics.
- [x] Create separate stdout and stderr files before spawning a process.
- [x] Persist process creation before reporting successful parking.
- [x] Retain terminal records and logs until `rm` or `clean` acts on them.
- [x] Implement startup reconciliation for records that claim to be active but whose process is gone.
- [x] Test SQLite recovery behavior, stale records, absent XDG variables, and process-name/path encoding.

Phase 3 implementation decisions:

- Durable state uses `$XDG_STATE_HOME/park`, falling back to `$HOME/.local/state/park`. Runtime state uses `$XDG_RUNTIME_DIR/park`, falling back to a private `runtime/park` directory under the durable state directory.
- Process metadata is stored in `$XDG_STATE_HOME/park/park.sqlite3` (or the documented fallback), while logs remain under its `logs` directory. SQLite identity columns use lossless Unix BLOB values for canonical project paths and ASCII process names.
- SQLite schema version 1 stores scalar process metadata in normalized columns,
  with ordered raw command arguments in a child table. It creates a unique
  `(project_path, name)` index, uses SQLite transactions for atomic record
  updates, and keeps the database private to the user.
- Every record load validates lifecycle fields, working-directory/key consistency, SQLite identity columns, and derived log paths before it is listed, reconciled, or removed.
- Log files are created independently with exclusive creation before a record is persisted. Terminal records and logs remain until explicit removal.
- Reconciliation accepts an injected liveness check; platform-specific PID and process-group ownership checks are not yet implemented for all supported Unix platforms.

## Phase 4: Daemon Ownership and Local IPC

- [x] Define a versioned local request/response protocol with structured success and error payloads.
- [x] Implement one per-user daemon endpoint and ownership lock.
- [x] Start the daemon on demand when the CLI cannot connect.
- [x] Handle concurrent first clients without starting competing daemons.
- [x] Detect and safely remove stale sockets and stale daemon markers.
- [x] Keep the daemon detached from the invoking terminal.
- [x] Implement IPC handlers for `ps`, `status`, and structured error responses first.
- [x] Keep JSON rendering in the CLI based on response data, never by parsing human text.
- [x] Test daemon startup, reconnection, stale runtime state, and concurrent client behavior.

Phase 4 implementation decisions:

- IPC uses newline-delimited JSON over a per-user Unix socket with protocol version `1`, request IDs, and the existing structured command-result schema.
- The daemon owns an advisory `flock` on `daemon.lock`; the kernel releases it if the daemon exits, so stale PID and socket markers never grant ownership.
- Clients retry the socket while one daemon owner starts. The daemon is launched through the installed `park` executable, detached with `setsid`, and competing starters exit after failing the ownership lock.
- A lock holder removes stale socket and PID marker files before binding the endpoint. It writes the current PID only after binding succeeds.
- The first handlers are project-scoped `ps` and exact-key `status`; the CLI renders JSON directly from response data.
- The daemon canonicalizes every project path received through IPC before dispatching a launch, list, or status operation. The client rejects a response whose version or request ID does not match its request and starts a daemon only after missing/refused-socket errors.

## Phase 5: Spawn, Capture, and Monitoring

- [x] Validate duplicate process keys before launch; reject by default.
- [x] Implement explicit replacement only after the lifecycle semantics are complete.
- [x] Spawn from the stored executable and argument vector without invoking a shell implicitly.
- [x] Spawn in the stored working directory with a detached session/process group on supported Unix systems.
- [x] Drain stdout and stderr independently so high output cannot block the child process.
- [x] Append captured bytes to their respective durable log files.
- [x] Persist `running` only after spawn succeeds.
- [x] Retain a `failed` record with a useful diagnostic when spawning fails.
- [x] Monitor child termination and persist its exit code or termination signal exactly once.
- [x] Test commands that exit successfully, fail, emit interleaved stdout/stderr, emit large output, and spawn children.

Phase 5 implementation decisions:

- Launch requests carry the canonical project path, ASCII name, and exact OS argument vector over IPC. The daemon rejects any existing record for the complete key, including retained terminal records.
- The daemon creates both log files and persists a `starting` record before spawning. On Linux it starts a Park supervisor, sets the supervisor as the leader of a dedicated session/process group, and records the supervisor's `/proc` start time. The supervisor starts the exact target argument vector without a shell and kills its group when the daemon dies.
- Independent Tokio capture tasks append raw stdout and stderr bytes to their respective files. The child is waited independently so output draining cannot block termination monitoring.
- A successful spawn records the PID, process-group ID, Linux process start time where available, and `running` state before returning success. Startup reconciliation requires the recorded Linux identity to match before treating a record as live. Spawn errors retain the pre-created record as `failed` with the diagnostic.
- Natural exits become `exited` with an exit code; signal termination becomes `killed` with the signal number. The monitor checks for an existing terminal record before saving the terminal transition.
- The daemon reserves each complete key through the launch transaction, so concurrent requests consistently receive a duplicate result. Key-derived logs without a matching record are stale pre-spawn artifacts and are recreated safely.
- Capture and wait failures terminate the managed group, persist `failed` with the diagnostic, and retry terminal persistence with capped backoff until it succeeds.

## Phase 6: Inspection and Logs

- [x] Implement `park ps` for the current project with deterministic ordering.
- [x] Implement `park status <name>` using persisted state and reconciled liveness.
- [x] Implement matching JSON representations with documented stable field names.
- [x] Implement combined `park logs <name>` plus independent `--stdout` and `--stderr` views.
- [x] Specify and implement deterministic ordering for combined streams, such as daemon-assigned sequence numbers.
- [x] Implement `--tail`, `--head`, and literal search after deciding the search contract.
- [x] Implement `--follow` as a streaming IPC request that does not block log draining.
- [x] End a follow session cleanly when the process exits, including its terminal result.
- [x] Test retained logs after exit, empty logs, follow termination, filtered output, and slow readers.

Phase 6 implementation decisions:

- `ps` and `status` reconcile active records against verified process identity before reading them. `ps` sorts names by their ASCII bytes.
- Log JSON data uses stable `stream`, `content`, and `state` fields. Content is returned as UTF-8 with replacement for invalid bytes; the durable files retain the original bytes.
- Combined output is stdout followed by stderr. This is deterministic but does not claim to reconstruct cross-stream event timing.
- `--grep` is a literal substring filter applied line-by-line before `--head` or `--tail`. Regex search is not yet implemented; adding it would require another dependency.
- Log responses use bounded newline-delimited JSON frames. Follow emits the initial snapshot and appended content, then a terminal frame containing the observed state. Initial head/tail/filter options apply to the retained snapshot; subsequent follow content is not head/tail limited.

## Phase 7: Lifecycle Control

- [x] Implement `stop` as SIGTERM to the managed process group, timeout, then SIGKILL escalation.
- [x] Implement `stop --force` as immediate forceful group termination.
- [x] Implement `signal` with a defined set of supported named signals and optional numeric parsing only if approved.
- [x] Serialize lifecycle operations per process key to prevent stop/restart/remove races.
- [x] Implement `restart` from the recorded executable, arguments, and working directory.
- [x] Implement `start` only for retained terminal records.
- [x] Implement `rm` as a separate operation that refuses active records and respects `--keep-logs`.
- [x] Implement `clean` with an explicit, conservative eligibility policy for terminal records.
- [x] Test graceful child-tree shutdown, forced shutdown, repeated operations, restart after exit, and removal behavior.

Phase 7 implementation decisions:

- `stop` uses a two-second grace period. The default sends SIGTERM to the verified process group; `--force` skips directly to SIGKILL. Both operations wait for the monitor to persist a terminal result.
- Supported named signals are HUP, INT, QUIT, TERM, USR1, USR2, STOP, CONT, and KILL, with an optional `SIG` prefix. Numeric signal parsing is not yet implemented because it was not approved for this phase.
- A per-key daemon lock serializes launch, stop, signal, restart, start, remove, and clean mutations. Monitor writes carry the spawned PID and ignore stale terminal updates after a later start attempt.
- Restart stops a running record before reusing its recorded command. Start is limited to terminal records. Both reset the current lifecycle fields and append output to the existing stdout and stderr logs.
- `rm` refuses active records or a still-present recorded process group and deletes metadata plus logs unless `--keep-logs` is set. `clean` removes terminal records with no remaining process group across the user's Park state and never removes active records.

## Phase 8: Agent Coordination and Hardening

- [x] Implement `wait --state`, `wait --match`, and `wait --exit` with timeout handling.
- [x] Define whether `--match` searches historical logs, new logs, or both, then test that contract.
- [x] Reconcile records after daemon crash, abrupt client disconnect, and machine reboot.
- [x] Guard process ownership checks against PID reuse using available start-time/process-group data.
- [x] Ensure slow IPC readers cannot stall child-output writers or daemon lifecycle monitoring.
- [x] Add integration tests that invoke the installed binary against isolated XDG state/runtime directories.
- [x] Add documentation for state locations, cleanup, exit codes, JSON schemas, and known platform limits.
- [ ] Add CI only after the test, format, lint, and toolchain commands are established.

Phase 8 implementation decisions:

- `wait` requires exactly one condition. State names are the six persisted lowercase state names. `--match` searches both complete log files, so historical output and output appended after restart/start are included. An empty pattern matches immediately.
- Wait timeouts use `ms`, `s`, or `m` suffixes and return the existing generic failure status on expiry. Wait uses streaming heartbeat frames, which lets the daemon observe client disconnects without blocking lifecycle operations.
- Monitor and reconciliation updates use compare-and-swap against the record snapshot they observed, preventing stale terminal updates from overwriting a newer start or restart. SQLite connections wait briefly on transient locks.
- Linux ownership checks validate PID start time, process group, and session. Descendant-only groups are recognized by matching the recorded group/session, while a reused bare group ID is not trusted. Non-Linux Unix ownership verification remains a documented limitation.
- IPC request reads and response writes have finite deadlines. Capture tasks remain independent of IPC clients.

## Roadmap

The following milestones are ordered by priority. Each feature must preserve the
configuration-free launch form and the existing project/name identity.

- [ ] **1. Automatic restart policies**
   Add opt-in `never`, `on-failure`, and `always` policies with intentional-stop
   suppression, bounded backoff, retry limits, persisted desired state, and clear
   restart generations in status and logs.
- [ ] **2. Filesystem-triggered restarts**
   Add an optional file-watch trigger for development processes, with debounce,
   ignored paths, restart-loop protection, and cross-platform behavior. Treat
   watching as a restart policy rather than turning Park into a general task
   runner.
- [ ] **3. Agent-aware coordination**
   Improve shared human-and-agent workflows with optional actor metadata,
   ownership hints, collision-resistant naming helpers, and clearer safeguards
   around processes managed by other actors. Shared visibility must remain the
   default within a user's Park state.
- [ ] **4. Optional project orchestration**
   Add an opt-in project configuration and `park up` / `park down` for repeatable
   process sets. Keep ad-hoc launches independent of configuration, and avoid
   growing into a general DAG or workflow engine.
- [ ] **5. Broader platform safety**
   Add platform-specific process-group, identity, and IPC implementations so
   lifecycle and stale-process guarantees are explicit on macOS, BSD, and later
   Windows rather than inferred from Linux behavior.
- [ ] **6. Reboot recovery policies**
   Define an explicit, opt-in policy for what should happen after an operating
   system reboot. The default should remain reconciliation without automatic
   restart.
- [ ] **7. Log lifecycle management**
   Add size- and age-based rotation, retention limits, pruning, compression, and
   explicit reporting of which historical output was removed.
- [ ] **8. Faster and richer log queries**
   Evaluate SQLite-backed indexes or log chunks while retaining raw append-only
   files as the recovery baseline. Improve search and cross-stream inspection
   without changing the lossless stdout/stderr contract.
- [ ] **9. Structured log metadata**
   Add opt-in structured envelopes with severity, attributes, resource fields,
   and trace identifiers. Preserve raw command output and distinguish event,
   observed, and ingested timestamps.
- [ ] **10. External log export**
   Add optional compressed export only after defining authentication, retry,
   backpressure, privacy, retention, and local-recovery behavior.
- [ ] **11. Selectable project resolution**
   Consider Git-root resolution as an explicit policy, never as an implicit
   change to invocation-directory scoping.
- [ ] **12. Graceful daemon upgrades**
   Add an explicit handoff path that lets a new Park binary replace a running
   daemon without interrupting managed process groups. Preserve socket and lock
   ownership and supervisor relationships during the handoff, and reject
   incompatible upgrades safely.
- [ ] **13. Store process names as text**
   Migrate the SQLite process-name column from `BLOB` to `TEXT` now that process
   names use a restricted ASCII character set. Keep the current BLOB schema until
   the migration and compatibility behavior are defined.

Process isolation and sandboxing are not supported by Park's core design. Park
should continue to manage ordinary host processes; users requiring filesystem,
network, or resource isolation should use containers or virtual machines.

## Approved Dependencies

Approved for the current version. Use the latest release compatible with the MSRV unless a phase records a narrower version requirement.

- [x] `clap`: conventional CLI argument parsing and subcommand boundaries.
- [x] `serde`: conventional structured data serialization for persisted records and IPC payloads.
- [x] `serde_json`: structured IPC and first-class JSON CLI output.
- [x] `tokio`: asynchronous local IPC, child-process monitoring, timers, and independent output draining.
- [x] `nix`: Unix process groups, signals, and kernel-managed advisory daemon locking.
- [x] `thiserror`: typed internal errors with stable machine-readable classification and exit-code mapping.
- [x] `rusqlite`: SQLite process metadata and transactional lifecycle persistence; the bundled feature avoids a system SQLite dependency. A JSON-file registry was rejected because concurrent lifecycle mutations and registry queries are core behavior in the current version.
- [ ] `fs2`: rejected for the current version; advisory locking is provided through `nix`.
- [ ] `anyhow`: rejected for the current version; typed errors are required at public command boundaries.
- [ ] `time` / `chrono`: rejected for the current version; timestamps are persisted as epochs with internal formatting.
