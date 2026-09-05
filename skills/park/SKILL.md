---
name: park
description: Use Park to manage project-scoped development processes safely.
---

# Park

Use Park when a development process needs to continue after the current shell or
agent session ends. Park is configuration-free by default and scopes every
record to the canonical project directory and process name.

## Before Starting

Run commands from the project directory that owns the process. Inspect existing
records before launching a named process:

```bash
park ps --json
```

Choose a stable, descriptive name such as `dev`, `api`, `worker`, or `frontend`.
Names are project-scoped, but a duplicate name in the same project is rejected.
Do not stop, remove, or replace an existing record just to make a name available.
Inspect its status and command first; reuse it only when it is the intended
process, otherwise choose another name or ask the user.

## Start And Verify

Use the `--` separator and pass the executable and arguments separately:

```bash
park dev -- pnpm dev
park worker -- cargo run --bin worker
park api --env-file .env --env-file .env.local -- ./bin/api --port 3000
```

Park records the exact argument vector. It does not implicitly invoke a shell.
Use an explicit shell command such as `sh -lc '...'` only when shell behavior is
needed.

At creation, Park captures the complete client environment. `--env-file` is
repeatable; the daemon reads those files, not the client. Dotenv values are
layered in argument order, captured client values take precedence over dotenv
values, and explicit values managed with `park env` take final precedence. The
merged environment is not persisted, so dotenv changes apply to later spawns.

After starting a process, wait for a useful condition instead of assuming that
the launch means the service is ready:

```bash
park wait dev --state running --timeout 5s
park wait dev --match 'ready' --timeout 30s
```

Use `--state running` for process state and `--match` for literal readiness text
written to either stdout or stderr. If readiness is not observable, inspect the
status and logs before reporting success.

## Inspect And Control

Use JSON for inspection and exit codes for lifecycle results:

```bash
park status dev --json
park logs dev --stdout --tail 100
park logs dev --stderr --tail 100
```

Stdout and stderr are retained separately, including after the process exits.
Use `park logs dev --follow` when live output is needed.

Use `park restart` only when restarting the intended existing record is part of
the task. Use `park stop` only for a process started by this task or when the
user explicitly requests stopping it. Prefer graceful stop over `--force`.

```bash
park restart dev
park stop dev
```

Normal restart reuses the stored client environment capture and rereads the
record's dotenv files. Use `--recapture-env` only when the current client
environment should replace the stored capture. That flag also enables repeated
`--env-file` arguments; supplied files replace the record's stored dotenv-file
list, while omitting them retains the existing list:

```bash
park restart dev --recapture-env
park restart dev --recapture-env --env-file .env --env-file .env.local
```

Use `park start <name> -- <command> [arguments...]` to create a new record when
the project/name key is unused. An existing record is never silently replaced.
Use `park env <name>` to inspect the effective environment, or update future
spawns with `--set KEY=VALUE` and `--unset KEY`. Environment updates do not
change an already running process. Environment captures can contain secrets, so
only inspect them when needed and treat the Park state directory as sensitive.
Use `--json` when an agent needs to inspect variables without parsing human
output. Dotenv files support data-only assignments, optional `export`, comments,
and quoted values; they are never evaluated as shell code.

Records remain available after exit. Use `park rm <name>` only for an inactive
record when its history is no longer needed. Do not use `park clean` as a broad
default cleanup operation because it can remove terminal records beyond the
current task.

## Failure Handling

Do not parse human-readable output when a machine-readable result is available.
The public exit codes are:

```text
0  success
1  generic failure
2  command-line usage error
3  missing record
4  duplicate record
5  invalid lifecycle state
```

If a launch returns the duplicate-record code, inspect the existing record
instead of silently stopping or replacing it. If a wait times out, inspect
`status` and both log streams and report the observed state and useful output.
