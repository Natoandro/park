# Implementation Plan

This proposed Unix-first Rust MVP preserves Park's public invariants: project-scoped identity, terminal-independent processes, durable separate logs, and script-friendly lifecycle control. Phase 0 records the Unix-only platform and toolchain decisions. Each phase ends with focused checks before the next phase begins.

## Phase 0: Foundation Decisions

Phase 0 establishes the platform, toolchain, and dependency policy before implementation. Do not add a dependency until its use is approved. `clap`, `serde`, and `serde_json` are conventional CLI/serialization dependencies; all other third-party crates require explicit approval before they appear in `Cargo.toml`.

- [x] Confirm the initial supported platform: Unix-only MVP.
- [x] Confirm the Rust toolchain policy: Edition 2024 and MSRV 1.85.
- [x] Approve the async/process strategy: Tokio.
- [x] Approve the persistence strategy: atomically replaced JSON metadata.
- [x] Approve Unix process-group, signal, and advisory-lock support through `nix`.
- [x] Record each approved crate, version policy, purpose, and rejected alternative in this document.

Recorded decisions:

- Initial platform: Unix-only MVP; Windows support is deferred.
- Toolchain: Edition 2024 with MSRV 1.85.
- Async runtime and local IPC: `tokio`; synchronous threads and standard-library sockets were rejected for the MVP.
- Durable metadata: atomically replaced JSON backed by `serde_json`; `rusqlite` was rejected for the MVP.
- Unix process groups, signals, and advisory locking: `nix`; internal FFI and a separate `fs2` dependency were rejected.
- Errors: `thiserror`; `anyhow` and fully internal error types were rejected in favor of typed stable outcomes.
- Time: epoch timestamps with internal formatting; `time` and `chrono` were rejected for the MVP.

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
- The short launch form is selected by a `--` separator immediately after the opaque name.
- Names have no reserved-word or lexical validation. They are passed as one command-line argument, with normal shell/OS argument-boundary rules.
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
- `ProcessKey` owns the canonical `ProjectPath` and opaque process name. Registry access accepts only a complete `ProcessKey`, never a name alone.
- New records begin in `starting`; valid transitions cover successful startup, graceful stopping, natural failure, and forceful termination. Terminal states cannot transition further.
- The in-memory registry rejects duplicate canonical keys while allowing identical names under distinct project paths. Durable storage is deferred to Phase 3.

## Phase 3: State Layout and Persistence

- [x] Resolve durable state under `XDG_STATE_HOME` or its XDG fallback.
- [x] Resolve the IPC directory under `XDG_RUNTIME_DIR`, with a documented safe fallback when it is unavailable.
- [x] Define stable on-disk directory and file naming that does not expose raw project paths as unsafe filenames.
- [x] Implement atomic record writes and crash-safe replacement semantics.
- [x] Create separate stdout and stderr files before spawning a process.
- [x] Persist process creation before reporting successful parking.
- [x] Retain terminal records and logs until `rm` or `clean` acts on them.
- [x] Implement startup reconciliation for records that claim to be active but whose process is gone.
- [x] Test interrupted writes, stale records, absent XDG variables, and process-name/path encoding.

Phase 3 implementation decisions:

- Durable state uses `$XDG_STATE_HOME/park`, falling back to `$HOME/.local/state/park`. Runtime state uses `$XDG_RUNTIME_DIR/park`, falling back to a private `runtime/park` directory under the durable state directory.
- Records and logs are stored under separate `records` and `logs` directories. Filenames use a deterministic digest of the canonical project path and opaque name rather than exposing either value as a path component.
- New records use an atomically linked completed temporary file; updates use a synced temporary file followed by atomic replacement. The records directory is synced after link/rename, and exclusive temporary creation retries collision-resistant names so stale files from a prior process do not block updates. Temporary files are ignored during record discovery.
- Every record load validates lifecycle fields, working-directory/key consistency, the expected record filename, and derived log paths before it is listed, reconciled, or removed.
- Log files are created independently with exclusive creation before a record is persisted. Terminal records and logs remain until explicit removal.
- Reconciliation accepts an injected liveness check so platform-specific PID and process-group ownership checks can be added with the daemon in later phases.

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

- Launch requests carry the canonical project path, opaque name, and exact OS argument vector over IPC. The daemon rejects any existing record for the complete key, including retained terminal records.
- The daemon creates both log files and persists a `starting` record before spawning. On Linux it starts a Park supervisor, sets the supervisor as the leader of a dedicated session/process group, and records the supervisor's `/proc` start time. The supervisor starts the exact target argument vector without a shell and kills its group when the daemon dies.
- Independent Tokio capture tasks append raw stdout and stderr bytes to their respective files. The child is waited independently so output draining cannot block termination monitoring.
- A successful spawn records the PID, process-group ID, Linux process start time where available, and `running` state before returning success. Startup reconciliation requires the recorded Linux identity to match before treating a record as live. Spawn errors retain the pre-created record as `failed` with the diagnostic.
- Natural exits become `exited` with an exit code; signal termination becomes `killed` with the signal number. The monitor checks for an existing terminal record before saving the terminal transition.
- The daemon reserves each complete key through the launch transaction, so concurrent requests consistently receive a duplicate result. Key-derived logs without a matching record are stale pre-spawn artifacts and are recreated safely.
- Capture and wait failures terminate the managed group, persist `failed` with the diagnostic, and retry terminal persistence with capped backoff until it succeeds.

## Phase 6: Inspection and Logs

- [ ] Implement `park ps` for the current project with deterministic ordering.
- [ ] Implement `park status <name>` using persisted state and reconciled liveness.
- [ ] Implement matching JSON representations with documented stable field names.
- [ ] Implement combined `park logs <name>` plus independent `--stdout` and `--stderr` views.
- [ ] Specify and implement deterministic ordering for combined streams, such as daemon-assigned sequence numbers.
- [ ] Implement `--tail`, `--head`, and literal/regex search only after deciding the search contract.
- [ ] Implement `--follow` as a streaming IPC request that does not block log draining.
- [ ] End a follow session cleanly when the process exits, including its terminal result.
- [ ] Test retained logs after exit, empty logs, follow termination, filtered output, and slow readers.

## Phase 7: Lifecycle Control

- [ ] Implement `stop` as SIGTERM to the managed process group, timeout, then SIGKILL escalation.
- [ ] Implement `stop --force` as immediate forceful group termination.
- [ ] Implement `signal` with a defined set of supported named signals and optional numeric parsing only if approved.
- [ ] Serialize lifecycle operations per process key to prevent stop/restart/remove races.
- [ ] Implement `restart` from the recorded executable, arguments, and working directory.
- [ ] Implement `start` only for retained terminal records.
- [ ] Implement `rm` as a separate operation that refuses active records and respects `--keep-logs`.
- [ ] Implement `clean` with an explicit, conservative eligibility policy for terminal records.
- [ ] Test graceful child-tree shutdown, forced shutdown, repeated operations, restart after exit, and removal behavior.

## Phase 8: Agent Coordination and Hardening

- [ ] Implement `wait --state`, `wait --match`, and `wait --exit` with timeout handling.
- [ ] Define whether `--match` searches historical logs, new logs, or both, then test that contract.
- [ ] Reconcile records after daemon crash, abrupt client disconnect, and machine reboot.
- [ ] Guard process ownership checks against PID reuse using available start-time/process-group data.
- [ ] Ensure slow IPC readers cannot stall child-output writers or daemon lifecycle monitoring.
- [ ] Add integration tests that invoke the installed binary against isolated XDG state/runtime directories.
- [ ] Add documentation for state locations, cleanup, exit codes, JSON schemas, and known platform limits.
- [ ] Add CI only after the test, format, lint, and toolchain commands are established.

## Deferred Work

- [ ] Optional project configuration and `park up` / `park down`.
- [ ] Log rotation, retention limits, and pruning policy.
- [ ] Git-root project resolution as an explicit selectable policy.
- [ ] Cross-platform process-group and IPC implementations.
- [ ] Process restart after operating-system reboot.

## Approved Dependencies

Approved for the MVP. Use the latest release compatible with the MSRV unless a phase records a narrower version requirement.

- [x] `clap`: conventional CLI argument parsing and subcommand boundaries.
- [x] `serde`: conventional structured data serialization for persisted records and IPC payloads.
- [x] `serde_json`: conventional JSON persistence and first-class JSON CLI output.
- [x] `tokio`: asynchronous local IPC, child-process monitoring, timers, and independent output draining.
- [x] `nix`: Unix process groups, signals, and kernel-managed advisory daemon locking.
- [x] `thiserror`: typed internal errors with stable machine-readable classification and exit-code mapping.
- [ ] `rusqlite`: rejected for the MVP; SQLite persistence may be reconsidered if registry querying or history requires it.
- [ ] `fs2`: rejected for the MVP; advisory locking is provided through `nix`.
- [ ] `anyhow`: rejected for the MVP; typed errors are required at public command boundaries.
- [ ] `time` / `chrono`: rejected for the MVP; timestamps are persisted as epochs with internal formatting.
