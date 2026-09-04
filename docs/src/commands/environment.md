# Environment

`park env` inspects and updates the environment policy stored with a process
record:

```text
park env <name> [--json]
park env <name> [--set KEY=VALUE]... [--unset KEY]...
```

Without mutation options, the command resolves and displays the effective
environment that the daemon would use for the next start. The result includes
the currently evaluated dotenv values, so it can change when an environment
file changes. Environment values are not included in ordinary `status` or `ps`
output.

`--set` adds or replaces a per-record environment override. `--unset` adds a
per-record removal for the named variable. Explicit overrides and removals take
precedence over captured values and dotenv files. These changes affect future
`start` and `restart` operations; a running process keeps the environment it
received when it was spawned.

The no-option form and mutation form both support `--json`. JSON output uses a
stable result envelope and returns the effective variables in deterministic key
order. Environment values can contain `=`; the first `=` separates a key from a
value.

## Sources And Precedence

Each record has three environment inputs:

1. The complete environment captured by the client that creates the record.
2. Zero or more dotenv file paths supplied with `--env-file` at creation time
   or during an explicit recapturing restart.
3. Explicit per-record overrides and removals made with `park env`.

The daemon evaluates these inputs for every process spawn. It starts with the
dotenv files in their recorded order, applies the captured client environment
over them, and then applies explicit overrides and removals. The merged result
is never persisted. The capture, file paths, and explicit edits are persisted
instead.

Dotenv files are read by the daemon, not by the client. Relative paths are
resolved from the canonical project directory and the daemon must be able to
read them. Files are parsed as data, never executed as shell scripts. The
initial grammar supports blank lines, comments, `KEY=value`, quoted values, and
an optional `export ` prefix; shell command substitution and arbitrary shell
syntax are not supported.

Environment files are reread for every `start` and `restart`. A read or parse
failure prevents the new process from being spawned and is reported as a
launch/lifecycle failure.

## Privacy

The captured environment and explicit values may contain credentials, tokens,
or other secrets. They are stored in the user-private Park database and are
shown only by the intentional `park env` command. Park does not promise
encryption at rest; users should avoid capturing sensitive values when that
risk is unacceptable and should treat access to the Park state directory as
access to the managed processes' environment.
