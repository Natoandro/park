# Logs

`logs` is Park's canonical log interface:

```text
park logs <name> [--tail N|--head N] [--follow] [--grep PATTERN] [--stdout|--stderr] [--json]
```

Park retains stdout and stderr separately, including after the command exits.
By default, `logs` combines them deterministically as stdout followed by
stderr. Use one of the stream selectors to read only one stream:

```bash
park logs dev --stdout
park logs dev --stderr
```

`--stdout` and `--stderr` cannot be used together.

## Filtering

`--grep PATTERN` keeps retained lines containing the literal `PATTERN`.
Patterns are substring searches; regular expressions are not supported. The
filter is applied before `--head` or `--tail`:

```bash
park logs dev --grep ready --tail 10
```

`--head N` keeps the first `N` matching lines. `--tail N` keeps the last `N`
matching lines. The two options cannot be used together.

## Following

`--follow` prints the initial retained output and then streams output as it is
appended. The initial output honors the selected stream, literal grep filter,
and head/tail filter. Later appended output is streamed without the head/tail
limit; the literal grep filter and stream selection continue to apply.

Following ends when the observed record reaches a terminal state. It works for
active and already-terminal records, and the retained output is still
available after following ends.

## JSON

`--json` emits the command result as JSON instead of the human log stream. A
log result's `data` contains the selected `stream`, collected `content`, and
observed `state`. With `--follow --json`, appended content is collected into
that result rather than written as raw follow output while it arrives.

The `--json` option is also available on `ps` and `status`; see the scripting
reference for the common result envelope and exit status behavior.
