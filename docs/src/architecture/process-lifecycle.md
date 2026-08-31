# Process Lifecycle

The daemon owns the lifecycle of each retained process record. The normal
state flow is:

```text
starting -> running -> stopping -> exited | failed | killed
```

The record is created before spawning, and a terminal outcome is persisted once
with either an exit code or a termination signal. Terminal records remain
available for status and log inspection.

## Launch and Monitoring

For a launch request, the daemon:

1. Canonicalizes the project path and reserves the complete project/name key.
2. Rejects an existing record for that key, including a retained terminal
   record.
3. Creates separate stdout and stderr log destinations.
4. Persists the `starting` record with the exact executable, arguments, and
   working directory.
5. Creates a dedicated process group or session where supported and spawns the
   command without an implicit shell.
6. Drains stdout and stderr independently while waiting for termination.
7. Persists `running` only after spawn succeeds, then records the terminal
   result exactly once.

On Linux, Park starts a supervisor directly from the stored argument vector.
The supervisor owns a dedicated session/process group, starts the target
command without a shell, and kills its group if the daemon dies. Other Unix
platforms currently spawn the managed command directly.

The launch reservation covers the check, record creation, and spawn transaction;
concurrent launches for the same complete key receive a duplicate result. If
capture or wait handling fails, the daemon terminates the managed group when
necessary, records `failed`, and retries durable terminal persistence with
capped backoff.

## Lifecycle Operations

`stop` is graceful by default. It transitions a running record to `stopping`,
sends `SIGTERM` to the managed process group, waits two seconds, and sends
`SIGKILL` only if the group is still alive. `stop --force` skips the grace
period. Group signaling also reaches descendants created by wrappers such as
`npm`, `pnpm`, and `cargo watch`, reducing the risk of orphaned children.

`signal` validates a supported named signal and targets the same verified group.
The supported names are `HUP`, `INT`, `QUIT`, `TERM`, `USR1`, `USR2`, `STOP`,
`CONT`, and `KILL`, with an optional `SIG` prefix. Numeric signal values are
not accepted.

`restart` stops an active process when necessary, then launches the preserved
executable, argument vector, and working directory. `start` is limited to a
retained terminal record. Both operations reset the current lifecycle fields
and append new output to the existing stream logs.

`rm` is distinct from `stop`: it refuses an active record or a record whose
managed group is still present, then removes metadata and, unless
`--keep-logs` is set, its logs. `clean` applies the same conservative process
group check to terminal records across the user's Park state. It never removes
an active record.

Lifecycle mutations for one key are serialized. This prevents concurrent stop,
restart, signal, start, remove, and clean operations from racing. Monitor and
reconciliation writes also compare the record snapshot they observed, so a
stale terminal update cannot overwrite a later start or restart.

## Reconciliation

The daemon reconciles non-terminal records when it starts and when inspection
requires a current status. A machine reboot is treated as reconciliation, not
as an automatic restart request: the record and logs are preserved, while the
process is exposed as no longer running.

On Linux, a PID alone is not proof that Park still owns a process. Ownership
checks require the recorded PID start time, process-group ID, and session to
match `/proc` identity data. If the original group leader has exited, the
recorded group/session combination can be used conservatively to find
descendants. A bare, reused PID or process-group ID is never sufficient.

Records whose identity cannot be verified are reconciled as no longer running,
rather than risking a signal to an unrelated process. Logs are retained during
that transition.

## Platform Limits

Park is Unix-first. Linux provides the strongest restart and reconciliation
safety because `/proc` exposes the process identity data required for ownership
checks. Other Unix targets retain the Unix process and IPC interface, but do
not yet claim equivalent safety across daemon restarts; platform-specific
identity checks are still needed.

Park does not yet support Windows, production service supervision, reboot-time
automatic restart, or process isolation. Managed commands remain ordinary host
processes. These limits are deliberate: lifecycle
operations should not imply ownership or safety guarantees the platform cannot
verify.
