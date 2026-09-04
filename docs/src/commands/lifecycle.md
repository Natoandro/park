# Lifecycle

Lifecycle and inspection commands target records in the canonicalized current
project directory. The readable subcommand forms are:

```text
park ps [--json]
park status <name> [--json]
park stop <name> [--force]
park restart <name>
park restart <name> --recapture-env [--env-file <path>]...
park start <name>
park start <name> [--env-file <path>]... -- <command> [arguments...]
park signal <name> <SIGNAL>
park rm <name> [--keep-logs]
park clean
park wait <name> (--state STATE | --match TEXT | --exit) [--timeout DURATION]
park env <name> [--json]
park env <name> [--set KEY=VALUE]... [--unset KEY]... [--json]
```

The operation commands also accept long-option aliases such as
`park --status dev`. The subcommand form is the canonical, readable form.

## Inspect

`ps` lists the retained process records for the current project. `status`
selects one record by name. Records remain available after the managed command
exits, so both commands can inspect historical outcomes as well as active
processes.

Without `--json`, `ps` prints a human-readable process table and `status` prints
human-readable record details. With `--json`, both commands emit the stable JSON
result envelope described in [Scripting](scripting.md).

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
terminal record. By default it uses the record's captured client environment,
but rereads its dotenv files. `--recapture-env` replaces the stored client
snapshot with the environment of the calling client before the restart. The
flag enables repeatable `--env-file` arguments. When supplied, those paths
replace the record's stored dotenv file list; when omitted, the existing list is
retained.

The replacement capture is a candidate until environment resolution and spawn
succeed. A failed preflight or spawn does not replace the prior stored capture.

`start <name>` starts a retained terminal record using its recorded command and
environment inputs. `start <name> -- <command> [arguments...]` is also an
explicit creation form: when the complete project/name key does not exist, it
creates and starts a new record, capturing the calling client's environment.
The optional `--env-file` arguments apply to this creation form. If the key is
already retained, the request returns the normal duplicate-record result rather
than replacing it.

Both operations reset the current lifecycle fields and append new output to
the existing stdout and stderr logs. Neither operation creates a second record
for the name.

## Environment

Use `park env <name>` to inspect the effective environment for the next spawn.
Use `--set KEY=VALUE` to add or replace an explicit per-record value and
`--unset KEY` to remove a variable even when it is present in a captured
snapshot or dotenv file. Environment updates do not mutate an already running
process; they apply to the next `start` or `restart`.

The merged environment is reevaluated for each spawn. `park env` therefore
reflects current dotenv contents rather than a persisted merged snapshot. See
[Environment](environment.md) for source precedence and dotenv behavior.

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
