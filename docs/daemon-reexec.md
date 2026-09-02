# Internal Design: Daemon Re-exec

Status: design only. This document describes a future upgrade mechanism; it is
not an implemented command or protocol contract.

## Decision Summary

Park should keep its per-user daemon and use an **in-place re-exec** for daemon
upgrades. The existing daemon process should replace its executable image with
the newly installed `park` binary while retaining the same process ID, socket,
daemon lock, and managed-process parent relationship.

Starting a second daemon and transferring ownership is not the primary design.
It would change the daemon PID, while Linux supervisors currently use the
daemon as their parent and terminate their managed process group when that
parent dies. A same-PID `exec` preserves that relationship by construction.

The implementation should be delivered in two stages:

1. Re-exec only when no managed process or monitor is active.
2. Re-exec with active processes after descriptor and monitor handoff is fully
   implemented and tested.

The first stage is deliberately useful on its own. It avoids risking active
development servers while establishing the upgrade path.

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
properties. It must not turn an upgrade into an implicit stop or restart.

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
preferred long-term trigger is daemon-side detection rather than a new client
request. This is important because a newly installed CLI cannot ask an older
daemon to understand a new `reexec` IPC operation.

On Linux, the daemon can periodically compare the identity of the running
executable with the executable at `current_exe()` or its startup path. An
atomic installer replacement normally changes the file identity. Metadata
changes such as size and modification time can provide a cheap preliminary
check; the daemon must not hash the executable on every loop.

Installers should replace binaries atomically. An installer that writes into a
running executable in place is not a supported upgrade primitive. If the path
is missing, no longer executable, or cannot be inspected, the daemon should
continue running the old image and report a diagnostic through internal
logging, not terminate managed processes.

An explicit internal re-exec request may be added later for testing and
operator control. It must be version-negotiated before a new client sends it.

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

The implementation must choose one platform-specific mechanism before active
handoff is enabled:

- Reattach to the direct child using an OS wait primitive and preserve exact
  exit status.
- Preserve a wait-related descriptor such as a Linux pidfd and use it in the
  new monitor.
- Move terminal-status persistence into a supervisor protocol that survives
  daemon replacement.

Polling only `/proc` is insufficient because it cannot recover the exact exit
code or terminating signal after the child has been reaped. The first idle
re-exec phase avoids this problem entirely.

## Supervisor Safety

Same-PID exec preserves the normal parent-death relationship when it succeeds,
but upgrade failure must not accidentally become process termination.

The current supervisor exits by killing its process group when it receives its
parent-death signal. Before active handoff is declared safe, one of these
failure policies must be implemented and tested:

1. **Strict same-PID startup:** require the new image to initialize without
   exiting, and accept that a post-exec startup crash kills active groups. This
   is not acceptable as the final upgrade guarantee.
2. **Supervisor handoff grace:** change supervisors to recognize a short-lived
   authenticated handoff lease and wait for the replacement daemon before
   killing the group. A genuine daemon crash still kills the group after the
   lease expires.
3. **Stable guardian:** move parent-death ownership to a small guardian whose
   lifetime is independent of the daemon image. The guardian transfers daemon
   ownership explicitly and kills the group only when both the daemon and
   guardian determine that ownership is lost.

The recommended direction is the handoff grace lease because it keeps the
existing supervisor shape while distinguishing intentional re-exec from an
unexpected daemon death. The lease must be private to the user, bound to the
daemon generation, and impossible to satisfy with only a reused PID.

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
requiring a second daemon or a public upgrade command.

An idle timeout may later allow the daemon to exit cleanly instead of
re-execing. The next CLI invocation would start the newly installed binary.
That is a simpler fallback, but it does not upgrade a daemon while processes
remain active.

## Active-Process Milestone

Active-process re-exec is complete only when the following are true:

- Listener and lock ownership are continuous.
- Every active stdout and stderr pipe is transferred without lost or reordered
  bytes attributable to the handoff.
- Every active supervisor remains owned by the same daemon PID.
- New monitoring records the exact terminal exit code or signal.
- Stale monitor writes cannot overwrite a later lifecycle generation.
- Existing process groups survive a successful upgrade.
- Failed upgrade initialization cannot kill active groups.
- Old and new clients receive bounded, explicit behavior during the brief
  quiescing window.

A per-record generation or launch token should be included in the handoff and
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
- A public `park upgrade` command.
- Automatic process restart after reboot.
- A second daemon competing for the existing endpoint.
- A guarantee that a daemon binary is upgraded by merely replacing its file
  unless the running daemon has re-exec support.

Until the active handoff milestone is complete, replacing the installed binary
may leave an already-running daemon on the previous image. Its managed
processes remain safe and functional; new daemon behavior takes effect after a
safe idle re-exec or daemon restart.
