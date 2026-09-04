# Configuration

Park is configuration-free by default. The normal launch workflow does not
need a project manifest or a configuration file:

```bash
park <name> [--env-file <path>]... -- <command> [arguments...]
```

Configuration is global to the current user. It is not project configuration,
and it does not change Park's project-scoped `(project directory, name)`
identity.

## Config File

The configuration layer reads an optional TOML file at:

```text
$XDG_CONFIG_HOME/park/config.toml
```

If `XDG_CONFIG_HOME` is not set, Park uses:

```text
$HOME/.config/park/config.toml
```

An absent file means that built-in defaults are used. An unreadable file,
malformed TOML, unknown field, or invalid setting is an error; Park does not
silently replace a user-provided file with defaults. Configuration-aware CLI
behavior is still being delivered, so the file does not change ordinary
launches today.

Create the parent directory before creating the file:

```bash
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/park"
```

## File Format

The complete initial configuration shape is:

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

All fields have defaults, so a partial file is allowed. Unknown fields are
rejected to catch misspelled settings instead of ignoring them.

## Daemon Re-exec

`daemon.reexec.active_processes` controls what a re-exec request should do when
managed records are active:

- `defer` is the default. An active daemon is not stopped just to upgrade it.
- `restart` opts in to stopping active process groups, re-execing while idle,
  and starting the previously active records again.

An explicit `park daemon reexec --force` selects the restart policy for that
request without modifying the config file. The re-exec command is still under
development; this setting establishes its policy boundary and does not itself
turn an ordinary launch into a supervised service.

## Automatic Restart

`managed_processes.restart` defines the global defaults for future automatic
restart behavior:

- `policy` is `never`, `on-failure`, or `always`. The default is `never`.
- `max_attempts` is the maximum number of automatic relaunches for one desired
  process run. Zero disables automatic relaunches.
- `initial_delay` is the delay before the first automatic relaunch.
- `max_delay` caps the exponential backoff delay.
- `multiplier` scales each successive delay and must be finite and at least
  `1.0`.

Delay values are non-negative integers with an `ms`, `s`, or `m` suffix, for
example `250ms`, `2s`, or `1m`. `initial_delay` cannot be greater than
`max_delay`.

Automatic process restart is not active yet. In particular, this configuration
does not change the behavior of explicit `park restart`, `park start`, or an
intentional `park stop`.

## Inspect Daemon Settings

Inspect the daemon runtime and effective configuration without selecting a
project:

```bash
park daemon status
park daemon status --json
park daemon config
park daemon config --json
```

`daemon status` reports the daemon PID, binary and protocol versions, handoff
format version, daemon generation, re-exec state, and active-record count. The
current implementation reports handoff version `0` and generation `1` until
handoff manifests and persistent daemon generations are implemented.

`daemon config` reports the effective values, whether they came from built-in
defaults or the config file, and the candidate config path. JSON output keeps
these fields structured for scripts.

## Scope And Status

The configuration file is optional and user-scoped. Park does not currently
provide `park up` or `park down`, per-record configuration, filesystem watch
rules, or project orchestration. Those would be separate features and must not
replace the configuration-free launch form.

See [Persistence and IPC State](architecture/persistence.md) for the durable
state and runtime directory layout, and the [development status](development.md)
for the implementation roadmap.
