# Launch

Launch a named command from the current project directory:

```text
park <name> [--env-file <path>]... -- <command> [arguments...]
```

The name identifies the process within the canonicalized current project
directory. The command is started independently of the terminal that launched
it. Park records the executable and argument vector exactly, starts it in the
project directory, and retains its status and output for later commands.

## Environment

At launch, the client captures its complete environment and sends that snapshot
to the daemon with the launch request. The daemon uses the snapshot rather than
its own environment when spawning the managed command. This includes variables
such as `PATH`, so executable lookup and the child environment use the same
launch inputs.

`--env-file` is repeatable. It sends a dotenv path to the daemon; the client
does not read the file. The daemon resolves relative paths from the canonical
project directory, reads the files in argument order, and applies later file
values over earlier file values. The captured client environment wins over
dotenv values with the same key, and per-record overrides made with `park env`
are applied last.

The captured snapshot and dotenv paths are stored with the record. The merged
environment is not stored. Every later `start` or `restart` rereads the dotenv
files, so changes to those files affect the next process. See
[Environment](environment.md) for inspection, updates, parsing, and precedence.

## Argument Boundary

The `--` separator is required in the launch form. It separates the process
name from the managed command. Everything after it is passed as the command
and its arguments; it is not parsed as a Park operation. This includes
arguments beginning with `-` or `--`:

```bash
park dev -- cargo run --release --bin worker
park dev --env-file .env --env-file .env.local -- node server.js
park dev -- -custom-command --flag
```

The first item after `--` is the executable. A launch without a command is a
usage error. Park launches the executable directly rather than reconstructing
a shell command, so shell syntax is not interpreted by Park.

## Names

Process names must contain only ASCII letters, digits, `.`, `_`, `-`, and
`:`, with no whitespace. They are not globally unique, and Park does not reserve
operation words. A name is available again only after its record is removed;
launching the same name in the same project returns a duplicate-record result.

An operation word becomes a name when the launch separator follows it:

```bash
park status -- ./server
```

Dash-prefixed names are also valid in launch form because `-` is an allowed
character:

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
