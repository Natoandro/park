# Internal Design: Daemon Re-exec

Status: design only. This document describes a future upgrade mechanism; it is
not an implemented command or protocol contract.

Compatibility assumption: this design only targets Park daemons that support
the re-exec operation. Compatibility with daemons from releases that predate
re-exec is explicitly out of scope. No fallback protocol or legacy handoff
path should be added for those releases.

## Decision Summary

Park should keep its per-user daemon and use an **in-place re-exec** for daemon
upgrades. The existing daemon process should replace its executable image with
the newly installed `park` binary while retaining the same process ID, socket,
daemon lock, and managed-process parent relationship.

Starting a second daemon and transferring ownership is not the primary design.
It would change the daemon PID, while Linux supervisors currently use the
daemon as their parent and terminate their managed process group when that
parent dies. A same-PID `exec` preserves that relationship by construction.

The implementation should be delivered in three stages:

1. Re-exec only when no managed process or monitor is active.
2. For an explicit or configured force-re-exec, stop active managed processes,
   re-exec while idle, and restart the same records.
3. Re-exec with active processes after descriptor and monitor handoff is fully
   implemented and tested.

The first two stages are deliberately useful on their own. They avoid risking
unplanned active-process loss while establishing the upgrade path. The second
stage accepts temporary downtime for processes that the user has configured as
safe to restart.

## Current Runtime Model

The current process topology is:

```text
park CLI
  `-- detached Park daemon
        `-- park --internal-supervisor
              `-- managed command and descendants
```

The CLI starts the daemon from the installed executable and detaches it with
`setsid` (`src/client.rs`). The daemon owns the Unix listener, the advisory
lock, lifecycle locks, the SQLite connection boundary, child wait handles, and
stdout/stderr capture tasks (`src/daemon/mod.rs`, `src/daemon/monitor.rs`).

On Linux, the daemon starts one internal supervisor for each managed command.
The supervisor creates the command session and process group. It receives the
daemon PID and uses `PR_SET_PDEATHSIG` so that daemon loss kills the managed
group (`src/daemon/launch.rs`, `src/main.rs`).

Durable process records and append-only logs are already the recovery source:

- SQLite stores the exact command, lifecycle state, PID, process-group ID,
  process start time, and terminal outcome.
- Separate files store stdout and stderr.
- The daemon reconciles active records against verified Linux process identity
  data at startup and during relevant operations.

Re-exec must preserve these existing ownership, logging, and recovery
properties. It must not stop or restart managed processes unless the configured
policy or an explicit `--force` command authorizes that behavior.

## Command Taxonomy and Configuration

Commands that target retained managed processes are **managed-process
operations**. The existing `status`, `logs`, `stop`, `restart`, `start`,
`signal`, `rm`, `clean`, and `wait` commands belong to this group. They address
process records and, where applicable, require a project-scoped process key.

Commands that target the per-user daemon are **daemon-management commands**.
They should use an explicit namespace so they cannot be confused with a
process name:

```text
park daemon status [--json]
park daemon reexec [--force]
park daemon config [--json]
```

`park daemon status` reports the daemon PID, binary version, protocol and
handoff versions, generation, re-exec state, and active-record count.
`park daemon reexec` is a public operator command and is also the test-facing
entry point for the re-exec path. It honors the configured active-process
policy; `--force` selects the stop-and-restart path for that invocation.
`park daemon config` reports the effective configuration and its source.

Configuration is global to the current user and is optional. The daemon uses
built-in defaults when no file exists. The initial configuration file is
`$XDG_CONFIG_HOME/park/config.toml`, falling back to
`$HOME/.config/park/config.toml`:

TOML is chosen for a human-editable configuration file. Its parser dependency
is approved and will be recorded with the implementation dependency changes;
this design does not add that dependency yet.

```toml
[daemon.reexec]
active_processes = "defer"

[managed_processes.restart]
policy = "never"
max_attempts = 3
initial_delay = "250ms"
max_delay = "30s"
multiplier = 2.0
```

The default `active_processes` value is `defer`. The supported alternative is
`restart`, which stops active records with normal lifecycle semantics, re-execs
while idle, and restarts the records that were active before the operation.
The setting applies to automatic version-mismatch handshakes and
`park daemon reexec`. A command-line `--force` override selects `restart` for
one explicit re-exec without modifying configuration.

The daemon loads configuration at startup and reloads it before each automatic
version-mismatch decision and explicit `daemon reexec` request. A malformed or
unreadable configuration is a structured configuration failure; the daemon
does not silently substitute defaults for a file the user provided.

The configuration also owns defaults for managed-process restart behavior. The
initial restart policy values are `never`, `on-failure`, and `always`, with
`never` as the default. Automatic restart is separate from an explicit
`park restart`, a re-exec restart plan, or an intentional `park stop`; an
intentional stop must suppress automatic restart. Backoff is exponential,
bounded by `max_delay`, and limited by `max_attempts`.
`max_attempts` counts automatic relaunches for one desired process run; it does
not limit explicit lifecycle commands.

Restart attempts, desired restart state, and the current restart generation
must be persisted so a daemon re-exec or crash cannot reset the retry budget or
create duplicate restarts. Per-record overrides are not part of the initial
configuration surface; the global defaults apply to all managed processes.

Park must not infer that a command is a development server or watcher from its
executable name. Enabling `restart` is an explicit user-level statement that
all active managed records may be briefly stopped and restarted. Per-record
restart policy can be added later if the global policy is too broad.

## Why Same-PID Exec

An ordinary process handoff would look like this:

```text
old daemon starts new daemon -> new daemon becomes ready -> old daemon exits
```

That is unsafe with the current supervisor relationship. When the old daemon
exits, each supervisor observes parent death and kills its process group before
the new daemon can take over.

An in-place exec has this shape instead:

```text
old daemon pauses work -> execve(new park image) -> new daemon resumes work
```

The process ID, session, parent relationship from the supervisors' point of
view, and inherited file descriptors remain in place. Rust memory, including
the Tokio runtime, does not remain in place, so the new image must reconstruct
its runtime state from inherited descriptors and durable records.

## Upgrade Trigger

The external installation method remains responsible for replacing the
executable. Park does not need to know whether the user used Cargo, a package
manager, or a release archive.

The daemon needs a way to learn that its executable has been replaced. The
chosen trigger is an internal `reexec` IPC operation initiated by the upgraded
CLI. Each CLI connection begins with a version handshake. When the daemon
reports a different binary version, the CLI sends the candidate executable
path and version in the re-exec request, waits for the daemon to become ready,
and retries the original request.

This keeps the upgrade flow automatic without adding a public `park upgrade`
command: the user upgrades through the original installation method, then the
next normal Park invocation upgrades the daemon. The handshake and re-exec
operation are part of the re-exec-capable protocol generation; no fallback is
required for pre-re-exec daemons.

Installers should replace binaries atomically. An installer that writes into a
running executable in place is not a supported upgrade primitive. If the path
is missing, no longer executable, or cannot be inspected, the daemon should
continue running the old image and report a diagnostic through internal
logging, not terminate managed processes.

The handshake outcomes are:

- Equal client and daemon compatibility identities: continue normally.
- Lower client identity: return a structured incompatibility error; never
  downgrade the daemon.
- Higher client identity with a matching candidate at the daemon's own startup
  path: apply the configured active-process policy, re-exec, and retry the
  original request.
- Higher client identity with an old, missing, non-executable, malformed, or
  otherwise incompatible candidate: return a structured upgrade-required
  failure and keep the old daemon serving.
- Candidate identity higher than the client identity: return a structured
  client-too-old error; do not re-exec into an image the client cannot use.

The compatibility identity is the Park package version together with the IPC
protocol version and handoff format version. Development builds with the same
identity are treated as compatible; a separate source revision identity is not
part of the initial design.

## Re-exec State Machine

The daemon should have an internal state in addition to process-record
lifecycle states:

```text
serving -> quiescing -> handing_off -> serving
                         `-> failed, serving old image
```

### Serving

Normal requests, launches, lifecycle mutations, monitor updates, and log
streaming continue normally. Executable replacement detection may set a
pending-reexec flag, but must not interrupt an in-flight operation.

### Quiescing

The daemon must establish a barrier before changing the executable image:

1. Stop accepting new IPC connections.
2. Reject or defer new lifecycle work with a retryable daemon-restarting
   result if necessary.
3. Allow already accepted short-lived requests to finish.
4. Do not begin a new launch, restart, start, stop, signal, remove, or clean
   mutation.
5. Decide how long-lived `logs --follow` and `wait` requests are handled. The
   initial implementation should close them with a retryable restart result;
   preserving client connections across exec is not required for the first
   active-process milestone.
6. Ensure no monitor is midway through a state transition or log write that
   has not been represented durably.

The quiescing barrier must not hold a lifecycle lock while waiting for an IPC
client that may be slow or disconnected. Existing IPC write deadlines remain
in force.

### Handing Off

Before calling `execve`, the daemon must:

1. Persist all lifecycle state that the new image will need.
2. Freeze the set of active records and assign each handoff entry a record key,
   supervisor PID, process-group identity, and stream descriptor mapping.
3. Make required descriptors inheritable across exec by clearing
   `FD_CLOEXEC` only for the approved handoff set.
4. Build a bounded, versioned handoff description.
5. Set an internal handoff marker so the new image knows not to unlink or
   rebind the inherited endpoint.
6. Execute the newly installed `park` binary using the daemon's original
   executable path.

The normal daemon startup path must remain unchanged. Only the explicit
handoff startup path may consume inherited descriptors. Arbitrary inherited
descriptors must be closed or ignored.

### New Image Startup

The re-exec image should perform these steps before resuming service:

1. Parse and validate the handoff version and descriptor table.
2. Verify that the inherited listener and lock descriptors are valid.
3. Reconstruct the Unix listener without unlinking the socket.
4. Reconstruct the advisory lock from the inherited lock descriptor. The new
   image must not attempt to acquire a second lock.
5. Open the existing SQLite database and validate its supported schema before
   accepting requests.
6. Reconstruct active monitor tasks from the handoff entries.
7. Revalidate each supervisor's PID, start time, process group, and session.
8. Reconcile any record whose identity cannot be verified.
9. Mark the daemon as serving and accept new requests.

If any validation fails, the new image must not signal an unrelated process or
delete the live socket. Failure behavior is addressed below.

## Descriptor Handoff

### Listener

The Unix listener must remain bound throughout re-exec. Its descriptor needs to
survive exec, and the new image must construct its Tokio listener from that
descriptor rather than calling `UnixListener::bind` again.

The socket path and PID marker must not be removed during handoff. The marker
may be rewritten only after the new image has successfully initialized, and it
must contain the unchanged daemon PID.

### Advisory Lock

The daemon lock is the authority for endpoint ownership. Its open file
descriptor must survive exec so the kernel-held lock is continuous. The new
image must adopt the descriptor instead of opening and locking the file again.

The old `DaemonLock` destructor must not run during a successful exec. On an
exec failure, control returns to the old image, which must either resume
serving with the original descriptors or terminate only after a safe failure
policy has been applied.

### stdout and stderr Pipes

For each active managed command, the daemon currently owns one read end for
stdout and one read end for stderr. Those read descriptors must be included in
the handoff if active-process re-exec is supported. The handoff description
must map each descriptor to the complete process key and stream.

The new image must resume independent capture for both streams. It must not
close a pipe before a replacement capture task owns it, or the managed command
may receive `SIGPIPE` or lose output.

The handoff format must be bounded and must reject duplicate descriptors,
unknown record keys, invalid stream names, and descriptors outside the
approved range.

### Child Wait State

The current Tokio `Child` value cannot be serialized across exec. The new
monitor must regain termination observation for the known supervisor PID.

Park will reattach to each direct supervisor child with Linux
`waitpid(WNOHANG)` polling. The supervisor remains the daemon's direct child
across exec, so the new monitor can observe and reap it without a new
dependency or a serialized Tokio `Child` value.

The reattached monitor must preserve the exact exit code or terminating signal.
Polling only `/proc` is insufficient because it cannot recover that outcome
after the child has been reaped. The first idle re-exec phase avoids this
problem until the active monitor implementation is ready.

## Supervisor Safety

Same-PID exec preserves the normal parent-death relationship when it succeeds,
but upgrade failure must not accidentally become process termination.

The supervisor will use a short-lived, authenticated handoff-grace lease. The
daemon writes the lease before exec, binding it to the daemon generation and
verified supervisor identities. If the supervisor receives its parent-death
signal during this window, it waits for the replacement daemon to prove that it
has adopted the same generation. A genuine daemon crash still terminates the
managed group after the bounded lease expires.

The lease must be private to the user, bound to the daemon generation, and
impossible to satisfy with only a reused PID. A successful same-PID exec clears
the lease after the new daemon has initialized. This preserves the existing
parent-death safety behavior while preventing a failed upgrade from being
treated as an ordinary daemon crash.

## Failure and Recovery Rules

### Before exec

Any failure while preparing the handoff is recoverable. The daemon should
discard the handoff description, restore `serving`, and continue with the old
image. It must leave the socket, lock, records, logs, and managed groups
untouched.

### `execve` failure

`execve` replaces no state if it returns an error. The old daemon remains the
owner. It should clear quiescing state and continue serving, then expose a
diagnostic through a later status or debug path. It must not remove the socket
or lock as if it had exited.

### New image validation failure

The new image must not run lifecycle actions during failed startup. It should
retain the inherited lock and endpoint while reporting a clear internal error.
If it cannot continue safely, the supervisor failure policy must prevent an
upgrade attempt from killing active groups. This is why active handoff cannot
ship before the supervisor policy is implemented.

### Lost or stale handoff state

Handoff metadata is runtime state, not durable process history. It must contain
a generation and expiry, use private permissions, and be ignored after a
failed or completed attempt. Startup must never trust a stale handoff marker to
claim a socket or a process group.

### Database or schema incompatibility

The new image must validate SQLite schema compatibility before taking over
active monitoring. Schema migration must be transactional and must preserve
the current record invariants. An image that cannot read the existing schema
must not partially migrate it during the handoff.

The IPC protocol version and persisted schema version are separate. A daemon
re-exec must not silently change either compatibility boundary.

## Idle-Only First Implementation

The first implementation should re-exec only when all of the following are
true:

- No process record is in `starting`, `running`, or `stopping`.
- No monitor task owns a child or capture pipe.
- No lifecycle mutation is in progress.
- No long-lived IPC stream is active.
- No accepted request remains unfinished.

In this phase, only the listener and daemon lock need to survive exec. The new
image can reconstruct all other state from SQLite and start the normal daemon
loop. This provides safe upgrades after managed processes finish, without
requiring a second daemon or a top-level upgrade command.

## Restart-active Re-exec

The configured restart-active path is the initial solution for users who accept
temporary downtime. It deliberately reaches the idle re-exec path instead of
trying to transfer live pipes or child wait handles.

When `active_processes` is `restart`, or when
`park daemon reexec --force` is used, the daemon must:

1. Enter `quiescing` and stop accepting new lifecycle work.
2. Snapshot every active record, its launch generation, and its exact restart
   command into a private restart plan before stopping anything.
3. Stop each active process group with the existing graceful-stop and
   escalation semantics.
4. Abort if any group cannot reach a safe terminal state. Restart every record
   already stopped by the plan, discard the plan, and keep the old daemon
   serving.
5. Re-exec only after every planned record is terminal and all monitor and
   capture tasks have completed.
6. Have the new image consume the restart plan and invoke the normal retained
   record start path for each planned record.
7. Persist individual restart failures while continuing to attempt the
   remaining records, then remove the completed plan atomically.

With the default `active_processes = defer` policy, an automatic version
mismatch or an unforced `park daemon reexec` returns a structured deferred
result and does not run the original request against the older daemon. The
user can wait for an idle daemon, change the global policy, or use `--force`.

## Development Todo

Tasks are ordered by dependency. A later milestone must not be marked complete
until the preceding milestone's tests pass. The implementation direction is
fixed by the design above; these are execution tasks, not open design choices.

### Milestone 0: Contract and Primitives

- [ ] [REXEC-M0-01] Add the client version handshake to the daemon connection
  path.
- [ ] [REXEC-M0-02] Add the internal `reexec` IPC operation with candidate
  executable path and version fields.
- [ ] [REXEC-M0-03] Add public daemon-management parsing for `park daemon
  status`, `park daemon reexec`, and `park daemon config`.
- [ ] [REXEC-M0-04] Add built-in configuration defaults and load the optional
  `$XDG_CONFIG_HOME/park/config.toml` file with its documented fallback.
- [ ] [REXEC-M0-05] Implement `daemon.reexec.active_processes` with `defer` as
  the default and `restart` as the opt-in value.
- [ ] [REXEC-M0-06] Implement the managed-process restart policy and bounded
  backoff configuration with `never` as the default.
- [ ] [REXEC-M0-07] Implement `park daemon status` and `park daemon config`
  output, including JSON output for scripts.
- [ ] [REXEC-M0-08] Add the retryable daemon-restarting response used while
  requests are quiesced.
- [ ] [REXEC-M0-09] Add a versioned private handoff manifest under the runtime
  directory with bounded size, private permissions, generation, and expiry.
- [ ] [REXEC-M0-10] Add the inherited descriptor table with fixed descriptor
  roles and strict `FD_CLOEXEC` handling.
- [ ] [REXEC-M0-11] Add daemon generations and per-record launch generations
  to reject stale monitor updates.
- [ ] [REXEC-M0-12] Expose the package, IPC protocol, and handoff compatibility
  identity through the version probe.
- [ ] [REXEC-M0-13] Validate the candidate executable path and require an
  atomically replaced executable for re-exec.

### Milestone 1: Safe Idle Re-exec

- [ ] [REXEC-M1-01] Add daemon runtime phases for `serving`, `quiescing`, and
  `handing_off`.
- [ ] [REXEC-M1-02] Implement the quiescing barrier for a daemon with no active
  process, monitor, lifecycle mutation, or long-lived IPC stream.
- [ ] [REXEC-M1-03] Implement listener descriptor inheritance and reconstruction without
  unlinking or rebinding the Unix socket.
- [ ] [REXEC-M1-04] Implement daemon-lock descriptor inheritance without releasing and
  reacquiring the kernel lock.
- [ ] [REXEC-M1-05] Implement the handoff startup path separately from ordinary daemon
  startup, with strict descriptor validation.
- [ ] [REXEC-M1-06] Implement same-PID `execve` using the validated candidate
  executable path.
- [ ] [REXEC-M1-07] Preserve the daemon PID marker and endpoint ownership across successful
  re-exec.
- [ ] [REXEC-M1-08] On preparation or `execve` failure, return to the old serving image
  without changing records, logs, socket, lock, or process groups.
- [ ] [REXEC-M1-09] Add idle re-exec unit and integration tests before attempting active
  restart-active handling.
- [ ] [REXEC-M1-10] Implement client mismatch handling: request re-exec, wait
  for readiness, and retry the original operation.

### Milestone 2: Restart-active Re-exec

- [ ] [REXEC-M2-01] Snapshot every active record and persist a restart plan before
  stopping any managed process.
- [ ] [REXEC-M2-02] Stop active process groups using the existing graceful stop
  and escalation semantics.
- [ ] [REXEC-M2-03] Abort and roll back the restart plan when any active process
  cannot reach a terminal state safely.
- [ ] [REXEC-M2-04] Re-exec only after all planned records are terminal and no
  monitor or capture task remains active.
- [ ] [REXEC-M2-05] Consume the restart plan in the new image and restart the
  records that were active before re-exec.
- [ ] [REXEC-M2-06] Persist individual restart failures without preventing the
  remaining planned records from being attempted.
- [ ] [REXEC-M2-07] Implement `park daemon reexec --force` as a one-shot
  `restart` policy override.
- [ ] [REXEC-M2-08] Add integration tests for configured restart-active and
  explicit forced re-exec, including partial stop and restart failures.

### Milestone 3: Preserved-process Handoff

- [ ] [REXEC-M3-01] Implement the supervisor handoff-grace lease.
- [ ] [REXEC-M3-02] Bind the lease to a private runtime location, daemon generation, and
  verified process identity.
- [ ] [REXEC-M3-03] Ensure an intentional re-exec does not trigger `PDEATHSIG` group cleanup.
- [ ] [REXEC-M3-04] Ensure a genuine daemon crash still terminates the managed group after
  the bounded grace period.
- [ ] [REXEC-M3-05] Reattach to each direct supervisor child with Linux
  `waitpid(WNOHANG)` polling and preserve its exact exit status.
- [ ] [REXEC-M3-06] Transfer every active stdout and stderr read descriptor with
  an explicit record-key and stream mapping.
- [ ] [REXEC-M3-07] Reconstruct independent capture tasks in the new image
  without closing a live pipe during the transition.
- [ ] [REXEC-M3-08] Reattach termination monitoring to every active supervisor
  and validate PID, start time, process group, and session before accepting it.
- [ ] [REXEC-M3-09] Add launch-generation checks to terminal persistence and
  monitor retry paths.
- [ ] [REXEC-M3-10] Implement retryable reconnect behavior for active
  `logs --follow` and `wait` clients during quiescing.
- [ ] [REXEC-M3-11] Add integration tests for active output capture, exact
  terminal outcomes, signals, and concurrent lifecycle requests during handoff.

### Milestone 4: Hardening and Release Readiness

- [ ] [REXEC-M4-01] Reject truncated, oversized, duplicated, stale, and version-mismatched
  handoff metadata.
- [ ] [REXEC-M4-02] Confirm inherited descriptors cannot cause access to unrelated sockets,
  files, records, or process groups.
- [ ] [REXEC-M4-03] Confirm SQLite schema validation and any migration are transactional
  before active monitoring resumes.
- [ ] [REXEC-M4-04] Test concurrent CLI requests, large interleaved output, terminal exits,
  signals, and daemon failure during each handoff phase.
- [ ] [REXEC-M4-05] Add Linux end-to-end coverage for idle, restart-active, and
  preserved-process re-exec.
- [ ] [REXEC-M4-06] Update `docs/architecture.md` and
  `docs/low-level-architecture.md` once the implementation behavior is fixed.
- [ ] [REXEC-M4-07] Update installation and daemon-management documentation to
  describe external binary upgrades and the re-exec command flow.

## Preserved-Process Milestone

Active-process re-exec is complete only when the following are true:

- Listener and lock ownership are continuous.
- Every active stdout and stderr pipe is transferred without lost or reordered
  bytes attributable to the handoff.
- Every active supervisor remains owned by the same daemon PID.
- New monitoring records the exact terminal exit code or signal.
- Stale monitor writes cannot overwrite a later lifecycle generation.
- Existing process groups survive a successful upgrade.
- Failed upgrade initialization cannot kill active groups.
- Clients connected before and after handoff receive bounded, explicit behavior
  during the brief quiescing window.

A per-record generation or launch token must be included in the handoff and
used with existing compare-and-swap persistence. PID equality alone is not
enough to reject stale monitor updates.

## Verification Plan

The implementation needs focused tests in addition to normal unit and E2E
coverage:

1. Replace the executable while the daemon is idle and verify that the daemon
   PID, socket, and lock ownership remain valid.
2. Re-exec after a terminal record exists and verify that records and logs are
   unchanged.
3. Attempt re-exec with a missing, non-executable, or incompatible candidate
   and verify that the old daemon continues serving.
4. Verify that concurrent clients cannot launch or mutate records during the
   quiescing barrier.
5. With active handoff enabled, generate large interleaved stdout and stderr,
   re-exec, and verify that capture continues without a pipe deadlock.
6. Exit or signal the managed command during handoff and verify one exact
   terminal transition.
7. Run `logs --follow` and `wait` across a handoff and verify the documented
   retry or continuation behavior.
8. Kill the daemon unexpectedly and verify that the existing supervisor safety
   behavior remains unchanged.
9. Exercise stale, duplicated, truncated, and version-mismatched handoff
   metadata.
10. Verify SQLite schema compatibility and rollback behavior for a failed
    startup validation.

The tests must run with isolated XDG state and runtime directories and must
inspect process identity using the same conservative rules as normal lifecycle
operations.

## Non-goals

This design does not introduce:

- A package-manager detector or installation-provenance database.
- A public top-level `park upgrade` command. Daemon re-exec is exposed under
  `park daemon reexec`.
- Automatic process restart after reboot.
- A second daemon competing for the existing endpoint.
- A guarantee that a daemon binary is upgraded by merely replacing its file
  unless the running daemon has re-exec support.

Until the preserved-process milestone is complete, the default policy defers
re-exec while managed processes are active. The configured restart-active
policy or `park daemon reexec --force` instead stops and restarts those records
before the new image resumes service.
