# Scripting

Park is non-interactive and keeps lifecycle results predictable for scripts and
coding-agent workflows. Commands resolve the current project directory, and
the daemon is started on demand. Use the readable subcommand forms and check
the process exit status rather than parsing human-readable messages.

For installation and the complete agent workflow, see [AI Agent
Integration](../ai-agents.md).

## JSON Output

`ps`, `status`, and `logs` support `--json`:

```bash
park ps --json
park status dev --json
park logs dev --tail 20 --json
```

The JSON result has a stable envelope with `status` and `ok`, plus optional
`message`, `data`, and `error` fields. An error contains its machine-readable
`code` and a human-readable `message`.

For inspection commands, `ps` places the matching project records in `data`
and `status` places the selected record there. A log result places its selected
`stream`, collected `content`, and observed `state` in `data`. JSON output is
written as one result, including error results, so scripts do not need to
interpret stderr text when using these flags.

Lifecycle and wait commands do not have a documented `--json` option. Their
success or failure is still represented by the process exit code.

## Exit Codes

The public exit codes are:

```text
0  success
1  generic failure
2  command-line usage error
3  missing record
4  duplicate record
5  invalid lifecycle state
```

For example, a launch using an existing name in the same project returns `4`.
A lookup for a name with no record returns `3`. A lifecycle operation that is
not valid for the record's current state returns `5`. A `wait` timeout returns
the generic failure code `1`.

## Reliable Workflows

Launch with an explicit separator, then wait for an observable condition:

```bash
park dev -- ./server
park wait dev --state running --timeout 5s
park wait dev --match 'ready' --timeout 30s
park logs dev --grep 'ready' --tail 1
```

Use `--exit` when any terminal outcome is sufficient:

```bash
park wait dev --exit --timeout 1m
```

`--state` is an exact state check, while `--exit` matches `exited`, `failed`,
or `killed`. `--match` is a literal byte-substring search across both retained
streams and observes output appended by later starts or restarts. Each wait
condition is checked immediately and then polled; provide exactly one of
`--state`, `--match`, or `--exit`.

Names and arguments should be passed as separate command-line arguments. The
`--` launch separator prevents Park operation parsing from consuming managed
command arguments, including dash-prefixed arguments. Use `--json` when
machine-readable inspection data is needed, and use exit codes to distinguish
missing, duplicate, invalid-state, usage, and generic failures.
