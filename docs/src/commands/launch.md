# Launch

Launch a named command from the current project directory:

```text
park <name> -- <command> [arguments...]
```

The name identifies the process within the canonicalized current project
directory. The command is started independently of the terminal that launched
it. Park records the executable and argument vector exactly, starts it in the
project directory, and retains its status and output for later commands.

## Argument Boundary

The `--` separator is required in the launch form. It separates the process
name from the managed command. Everything after it is passed as the command
and its arguments; it is not parsed as a Park operation. This includes
arguments beginning with `-` or `--`:

```bash
park dev -- cargo run --release --bin worker
park dev -- -custom-command --flag
```

The first item after `--` is the executable. A launch without a command is a
usage error. Park launches the executable directly rather than reconstructing
a shell command, so shell syntax is not interpreted by Park.

## Names

Names are opaque command-line arguments. They are not globally unique, and
Park does not reserve operation words or impose lexical name validation. A
name is available again only after its record is removed; launching the same
name in the same project returns a duplicate-record result.

An operation word becomes a name when the launch separator follows it:

```bash
park status -- ./server
```

Dash-prefixed names are also valid in launch form:

```bash
park -status -- ./server
park --status -- ./server
```

Without the separator, `status` and `--status` in the operation position are
parsed as the status operation or its long-option alias. Use the readable
subcommand form for operations; the launch separator makes the intended
boundary unambiguous.

## Project Scope

Park resolves the caller's current directory canonically before creating the
record. The same name can therefore be used in different projects, while the
same canonical project and name cannot have two records. The daemon starts on
demand, so the launching terminal does not need to remain open.
