# Lifecycle

Lifecycle and inspection commands target records in the canonicalized current
project directory. The readable subcommand forms are:

```text
park ps [--json]
park status <name> [--json]
park stop <name> [--force]
park restart <name>
park start <name>
park signal <name> <SIGNAL>
park rm <name> [--keep-logs]
park clean
park wait <name> (--state STATE | --match TEXT | --exit) [--timeout DURATION]
```

The operation commands also accept long-option aliases such as
`park --status dev`. The subcommand form is the canonical, readable form.

## Inspect

`ps` lists the retained process records for the current project. `status`
selects one record by name. Records remain available after the managed command
exits, so both commands can inspect historical outcomes as well as active
processes.

The lifecycle states are `starting`, `running`, `stopping`, `exited`, `failed`,
and `killed`. The last three are terminal states.

## Stop and Signal

`stop` applies only to a running record. By default it sends `SIGTERM` to the
managed process group, waits up to two seconds, and escalates to `SIGKILL` if
the group is still active. `--force` sends `SIGKILL` immediately.

`signal` sends a supported signal to the managed process group. The accepted
names are `HUP`, `INT`, `QUIT`, `TERM`, `USR1`, `USR2`, `STOP`, `CONT`, and
`KILL`; each may optionally be written with a `SIG` prefix. Numeric signal
values are not accepted. Signals apply while a record is `starting`,
`running`, or `stopping`; a terminal record cannot be signaled.

Park targets the process group rather than only the recorded process, so child
processes are included where the platform supports process groups.

## Restart and Start

`restart` stops an active process when necessary, then starts it again from the
recorded executable, arguments, and working directory. It can also restart a
terminal record. `start` only starts a retained terminal record.

Both operations reset the current lifecycle fields and append new output to
the existing stdout and stderr logs. Neither operation creates a second record
for the name.

## Remove and Clean

`rm` removes a terminal record. It refuses an active record and also refuses a
record whose managed process group is still present. Logs are removed with the
record unless `--keep-logs` is supplied.

`clean` removes eligible terminal records across the user's Park state. A
record is eligible only when its managed process group is no longer present.
Active records are never removed. Cleanup also removes the logs for records it
removes.

## Wait

`wait` requires exactly one condition:

- `--state STATE` succeeds when the persisted state exactly equals `STATE`.
- `--exit` succeeds for any terminal state.
- `--match TEXT` searches both retained stdout and stderr for a literal byte
  substring.

The match search includes output retained from earlier runs and output appended
by later `start` or `restart` operations. Conditions are checked immediately
and then polled. An optional `--timeout DURATION` accepts non-negative values
ending in `ms`, `s`, or `m`, such as `250ms`, `2s`, or `1m`. A timeout is a
generic failure.
