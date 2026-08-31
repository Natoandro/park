# End-to-End User Stories

This document is the behavior catalog for Park's end-to-end tests. Each story
describes an observable user outcome rather than an implementation detail. A
test may cover more than one story when the setup and assertions are naturally
shared, but every story should remain independently traceable by its reference
ID.

## Test Conventions

### Story fields

- **Reference:** Stable identifier used by test names, reports, and defects.
- **Scope:** Product area covered by the story.
- **Priority:** `P0` is critical-path behavior, `P1` is important behavior, and
  `P2` is edge or hardening behavior.
- **Actor:** User, script, coding agent, or operating system.
- **Story:** The user-centered outcome in “As a..., I want..., so that...” form.
- **Preconditions:** Required environment and records before the scenario.
- **Scenario:** The command sequence or event that drives the test.
- **Acceptance criteria:** Observable assertions, including exit status and
  output where relevant.

### Isolated test environment

Every e2e test should use a fresh temporary root and set both
`XDG_STATE_HOME` and `XDG_RUNTIME_DIR` inside it. Tests should invoke the
installed `park` binary, run from an explicit project directory, and clean up
the daemon, managed process groups, temporary directories, and any child
processes. Tests must not use the developer's real Park database or runtime
socket.

Use these fixture commands where possible:

- `/bin/sh -c 'printf ...'` for controlled stdout and stderr.
- `/bin/sleep 30` for a process that remains running.
- `/bin/sh -c 'trap ... TERM; sleep 30'` for graceful-stop and escalation
  behavior.
- `/bin/true` and `/bin/false` for immediate successful and unsuccessful exits.
- A temporary executable or an intentionally missing executable for spawn
  outcomes.

Do not assert generated PIDs, timestamps, socket paths, or hashed storage file
names. Assert their presence, type, ordering, and relationships instead.

### Exit-status vocabulary

- `0`: success
- `1`: generic failure
- `2`: command-line usage error
- `3`: missing record
- `4`: duplicate record
- `5`: invalid lifecycle state

### JSON assertions

Inspection and log JSON responses are structured command results. Tests should
assert `status`, `ok`, and relevant `data` or `error` fields. Process names are
ASCII; command executables and arguments may contain arbitrary Unix bytes, so
tests should verify lossless round trips for those values. Paths and process
identifiers should be checked for consistency, not fixed values.

## Foundation And CLI

### PARK-CLI-001: Launch a named command

- **Scope:** Launch form
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want to park a command under a name, so that I
  can manage it after leaving the launching terminal.
- **Preconditions:** An isolated project directory exists and no record named
  `dev` exists in it.
- **Scenario:** Run `park dev -- /bin/sleep 30`, then query `park status dev`.
- **Acceptance criteria:** Launch exits `0`; the record is addressable as
  `dev`; status reports an active lifecycle state, normally `running`; the
  command is not attached to the caller's terminal.

### PARK-CLI-002: Preserve the exact managed command

- **Scope:** Argument handling and restart
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want every executable argument preserved exactly,
  so that restart runs the command I requested rather than a reconstructed
  shell string.
- **Preconditions:** An isolated project directory exists.
- **Scenario:** Launch a command with spaces, quotes, an empty argument, shell
  metacharacters, and an argument beginning with `-`; wait for exit; run
  `park restart <name>`; inspect status and logs.
- **Acceptance criteria:** Launch and restart both succeed; the command
  receives the same argument vector on both runs; shell metacharacters are not
  interpreted; the record retains the executable and arguments as distinct
  values.

### PARK-CLI-003: Use the explicit `run` launch alias

- **Scope:** CLI aliases
- **Priority:** P1
- **Actor:** Developer
- **Story:** As a developer, I want an explicit launch alias, so that I can use
  `park run <name> -- <command>` when it is clearer in a script.
- **Preconditions:** No conflicting record exists.
- **Scenario:** Run `park run worker -- /bin/true` and wait for `worker` to exit.
- **Acceptance criteria:** The command is launched under `worker`; behavior
  matches the short launch form; the terminal record remains available.
- **Test note:** This is an optional compatibility-alias story because `run` is
  not the canonical launch form.

### PARK-CLI-004: Treat operation words as valid launch names

- **Scope:** ASCII process names
- **Priority:** P1
- **Actor:** Developer
- **Story:** As a developer, I want names such as `status` to be valid, so
  that names are not unexpectedly reserved by the CLI.
- **Preconditions:** No record named `status` exists.
- **Scenario:** Run `park status -- /bin/true`, then run `park status status
  --json`.
- **Acceptance criteria:** The first command launches a record named `status`;
  the second command addresses that record; no operation ambiguity occurs when
  the launch separator follows the name.

### PARK-CLI-005: Accept dash-prefixed launch names

- **Scope:** CLI parsing
- **Priority:** P1
- **Actor:** Developer
- **Story:** As a developer, I want a name such as `--status`, so that Park
  supports useful ASCII punctuation without reserving operation words.
- **Preconditions:** No such record exists.
- **Scenario:** Run `park --status -- /bin/true`, then inspect it with the
  canonical status form using the exact name.
- **Acceptance criteria:** The launch is parsed as a launch, not as the status
  alias; the record is created under the dash-prefixed name; status and logs can
  address it.

### PARK-CLI-006: Pass command flags after the separator

- **Scope:** CLI parsing
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want command flags after `--` passed through
  untouched, so that Park does not consume arguments intended for my process.
- **Preconditions:** An executable accepting flags is available.
- **Scenario:** Run `park flags -- /bin/sh -c 'printf "%s" "$1"' sh --child-flag`.
- **Acceptance criteria:** Park exits successfully; the child receives
  `--child-flag`; Park's own option parser does not treat it as a Park option.

### PARK-CLI-007: Reject incomplete launch syntax

- **Scope:** CLI usage errors
- **Priority:** P0
- **Actor:** Script
- **Story:** As a script author, I want malformed invocations rejected clearly,
  so that a typo cannot create an unintended process.
- **Preconditions:** None.
- **Scenario:** Try a missing name, a missing `--` separator, a missing command,
  and an option with a missing value.
- **Acceptance criteria:** Each invocation exits `2`; no process record is
  created; the diagnostic is written to stderr; output remains non-interactive.

### PARK-CLI-008: Use operation aliases

- **Scope:** CLI aliases
- **Priority:** P1
- **Actor:** Script
- **Story:** As a script author, I want long operation aliases, so that existing
  scripts can use forms such as `park --status dev`.
- **Preconditions:** A record named `dev` exists.
- **Scenario:** Exercise `--ps`, `--status`, `--logs`, `--stop`, `--restart`,
  `--start`, `--signal`, `--rm`, `--clean`, and `--wait` with the same
  arguments as their canonical subcommands.
- **Acceptance criteria:** Each alias selects the same operation and produces
  the same result, exit status, and JSON schema as the canonical form.

## Project Scope And Identity

### PARK-SCOPE-001: Scope names to the invocation project

- **Scope:** Project resolution
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want a process name scoped to my current project,
  so that unrelated projects can use the same name safely.
- **Preconditions:** Two distinct project directories exist.
- **Scenario:** Launch `dev` in each directory; run `ps` and `status dev` from
  each directory.
- **Acceptance criteria:** Both launches succeed; each project sees only its
  own `dev`; each status result has the corresponding canonical project path.

### PARK-SCOPE-002: Reject duplicate names in one project

- **Scope:** Identity and duplicate handling
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want an existing `(project, name)` protected,
  so that a second launch cannot silently replace my process.
- **Preconditions:** A `dev` record exists, including a retained terminal
  record.
- **Scenario:** Run a second `park dev -- /bin/true` in the same project.
- **Acceptance criteria:** The second launch exits `4`; the original record and
  logs remain unchanged; no replacement process is started.

### PARK-SCOPE-003: Canonicalize relative project paths

- **Scope:** Project resolution
- **Priority:** P1
- **Actor:** Developer
- **Story:** As a developer, I want relative path spellings to identify the
  same project, so that `.` and equivalent paths do not create duplicate
  namespaces.
- **Preconditions:** A project is reachable through multiple relative path
  spellings.
- **Scenario:** Launch from one spelling and query from another spelling that
  resolves to the same directory.
- **Acceptance criteria:** The same record is returned; status and `ps` do not
  show a second record; the stored project path is canonical and absolute.

### PARK-SCOPE-004: Canonicalize symlink aliases

- **Scope:** Project resolution
- **Priority:** P1
- **Actor:** Developer
- **Story:** As a developer, I want a symlinked project path to resolve to its
  real directory, so that aliases cannot bypass project scoping.
- **Preconditions:** A real project directory and a symlink to it exist.
- **Scenario:** Launch from the real path and inspect from the symlink path.
- **Acceptance criteria:** Both invocations address the same record; the
  canonical project path is the real path; no duplicate launch is possible via
  the alias.

### PARK-SCOPE-005: Reject an invalid project directory

- **Scope:** Project resolution
- **Priority:** P1
- **Actor:** Developer
- **Story:** As a developer, I want an invalid current directory reported as an
  error, so that a process is never recorded under an ambiguous location.
- **Preconditions:** The test can invoke `park` from a missing path or a file
  path.
- **Scenario:** Run a project-bearing command from the invalid location.
- **Acceptance criteria:** The command exits `1`; stderr explains the project
  resolution failure; no record or daemon state is created for that operation.

### PARK-SCOPE-006: Keep non-UTF-8 command arguments lossless on Unix

- **Scope:** Unix argument handling
- **Priority:** P2
- **Actor:** Developer
- **Story:** As a Unix user, I want non-UTF-8 command arguments preserved, so that
  Park can manage ordinary Unix argument values without corrupting them.
- **Preconditions:** The test runner can construct non-UTF-8 Unix arguments.
- **Scenario:** Launch a command with an ASCII name and a non-UTF-8 argument;
  inspect JSON, restart it, and query its logs.
- **Acceptance criteria:** The record remains addressable by its ASCII name;
  JSON exposes stable field types; restart passes the same argument bytes; no
  lossy replacement changes the command.

## Launch, Capture, And Exit Records

### PARK-LAUNCH-001: Detach a long-running process from the terminal

- **Scope:** Process ownership
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want the parked command to continue after the
  client exits, so that closing a terminal does not stop local development work.
- **Preconditions:** A long-running command is available.
- **Scenario:** Launch it, let the `park` client exit, wait briefly, and query
  status from a separate client process.
- **Acceptance criteria:** The client exits promptly; the process remains
  active; status can reconnect and report it without the original terminal.

### PARK-LAUNCH-002: Record before reporting successful launch

- **Scope:** Launch transaction
- **Priority:** P0
- **Actor:** Script
- **Story:** As a script author, I want a successful launch immediately
  inspectable, so that a success response never refers to missing metadata.
- **Preconditions:** None.
- **Scenario:** Launch a long-running command and immediately call `status` and
  `ps` from another client.
- **Acceptance criteria:** After launch returns successfully, both operations
  find the record; status contains a valid active identity and working
  directory. The scenario does not attempt to prove internal transaction
  ordering.

### PARK-LAUNCH-003: Capture stdout separately

- **Scope:** Output capture
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want standard output retained, so that I can
  inspect normal command output later.
- **Preconditions:** None.
- **Scenario:** Run a command that writes known bytes to stdout and exits.
- **Acceptance criteria:** `park logs <name> --stdout` returns exactly those
  bytes; the stdout log remains available after exit; status is terminal.

### PARK-LAUNCH-004: Capture stderr separately

- **Scope:** Output capture
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want standard error retained independently, so
  that diagnostics are not mixed into normal output.
- **Preconditions:** None.
- **Scenario:** Run a command that writes known bytes to stderr and exits.
- **Acceptance criteria:** `park logs <name> --stderr` returns exactly those
  bytes; stdout does not contain the stderr bytes; the stderr log remains after
  exit.

### PARK-LAUNCH-005: Drain high-volume output without deadlock

- **Scope:** Output capture and monitoring
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want verbose commands to finish normally, so
  that full stdout or stderr pipes cannot deadlock Park.
- **Preconditions:** A command can emit more than the operating-system pipe
  buffer on one or both streams.
- **Scenario:** Launch a command that emits a large known payload and exits.
- **Acceptance criteria:** The command reaches a terminal state; the retained
  log has the complete payload; status and logs remain responsive while capture
  is occurring.

### PARK-LAUNCH-006: Record a successful exit code

- **Scope:** Terminal records
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want a naturally exited command's result saved,
  so that I can diagnose it later without rerunning it.
- **Preconditions:** None.
- **Scenario:** Launch `/bin/sh -c 'exit 7'`; wait for exit; inspect JSON status.
- **Acceptance criteria:** State is `exited`; `exit_code` is `7`; an exit
  timestamp is present; the record remains inspectable and `wait --exit`
  succeeds.

### PARK-LAUNCH-007: Record a spawn failure

- **Scope:** Launch failure handling
- **Priority:** P0
- **Actor:** Script
- **Story:** As a script author, I want a failed spawn represented as a record,
  so that the failure is inspectable instead of leaving ambiguous partial state.
- **Preconditions:** The requested executable does not exist or cannot be run.
- **Scenario:** Launch the invalid executable and then query `status` and
  `logs`.
- **Acceptance criteria:** Launch exits `1`; status finds a `failed` record with
  a diagnostic; no active process remains; the record can later be removed or
  restarted, with a subsequent spawn attempt producing the appropriate result.

### PARK-LAUNCH-008: Preserve child processes in a managed group

- **Scope:** Process groups
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want wrappers and descendants managed together,
  so that stopping Park does not orphan child processes.
- **Preconditions:** A command spawns a long-running descendant and exposes its
  PID to the fixture.
- **Scenario:** Launch the wrapper, verify the descendant is alive, stop the
  named process, and check the descendant.
- **Acceptance criteria:** The managed group reaches a terminal state; the
  descendant is also terminated on supported Unix platforms; no orphan remains.

## Inspection And Listing

### PARK-INSPECT-001: List records with `ps`

- **Scope:** Process listing
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want to list Park records for the current
  project, so that I can discover names before controlling them.
- **Preconditions:** Zero, one, and multiple records can be prepared.
- **Scenario:** Run `park ps` before and after launching records.
- **Acceptance criteria:** Empty projects return success and an empty result;
  existing records appear; only the current canonical project is listed.

### PARK-INSPECT-002: Sort `ps` deterministically

- **Scope:** Process listing
- **Priority:** P1
- **Actor:** Script
- **Story:** As a script author, I want stable list ordering, so that output can
  be compared and processed reliably.
- **Preconditions:** Records with names that sort differently are present.
- **Scenario:** Launch names in reverse order and run `park ps --json`.
- **Acceptance criteria:** Results have deterministic name ordering independent
  of launch order; repeated calls return the same ordering.

### PARK-INSPECT-003: Inspect one record with `status`

- **Scope:** Process status
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want detailed status for one name, so that I can
  see its current state, command, working directory, and outcome.
- **Preconditions:** A record exists.
- **Scenario:** Run `park status <name>` while active and after termination.
- **Acceptance criteria:** Human output is non-interactive and readable; the
  record includes the name, project key, working directory, command details,
  lifecycle state, and terminal outcome when available.

### PARK-INSPECT-004: Return stable JSON for `ps`

- **Scope:** Machine-readable inspection
- **Priority:** P0
- **Actor:** Coding agent
- **Story:** As an automation client, I want structured `ps` output, so that I
  do not need to parse human text.
- **Preconditions:** At least one record exists.
- **Scenario:** Run `park ps --json`.
- **Acceptance criteria:** stdout is valid JSON; the top-level result contains
  `status: "success"`, `ok: true`, and an array in `data`; stderr is empty on
  success; fields are stable across repeated calls.

### PARK-INSPECT-005: Return stable JSON for `status`

- **Scope:** Machine-readable inspection
- **Priority:** P0
- **Actor:** Coding agent
- **Story:** As an automation client, I want structured status data, so that I
  can make lifecycle decisions programmatically.
- **Preconditions:** A record exists.
- **Scenario:** Run `park status <name> --json` before and after exit.
- **Acceptance criteria:** stdout is valid JSON with `status`, `ok`, and record
  `data`; state, timestamps, logs, and terminal fields have consistent names;
  no human diagnostic is mixed into JSON stdout.

### PARK-INSPECT-006: Report a missing record consistently

- **Scope:** Lookup errors
- **Priority:** P0
- **Actor:** Script
- **Story:** As a script author, I want missing names distinguishable from
  generic failures, so that scripts can branch correctly.
- **Preconditions:** No record with the requested name exists in the project.
- **Scenario:** Run `status`, `logs`, `stop`, `restart`, `start`, `signal`,
  `rm`, and `wait --exit` for the missing name.
- **Acceptance criteria:** Each applicable command exits `3`; human diagnostics
  go to stderr; JSON-capable commands report `status: "missing_record"` and
  `ok: false`; no record is created.

## Logs

### PARK-LOG-001: Read combined logs

- **Scope:** Log inspection
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want one canonical log view, so that I can
  inspect all retained output with a single command.
- **Preconditions:** A terminal record has stdout and stderr output.
- **Scenario:** Run `park logs <name>`.
- **Acceptance criteria:** The command succeeds and returns stdout followed by
  stderr deterministically; both streams are present; the result does not claim
  to reconstruct cross-stream timing.

### PARK-LOG-002: Select only stdout

- **Scope:** Log filtering
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want only stdout, so that diagnostics do not
  pollute data output.
- **Preconditions:** Both streams contain distinct lines.
- **Scenario:** Run `park logs <name> --stdout`.
- **Acceptance criteria:** Only stdout bytes are returned; stderr bytes are
  absent; the command succeeds for empty stdout as well.

### PARK-LOG-003: Select only stderr

- **Scope:** Log filtering
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want only stderr, so that I can focus on errors
  and diagnostics.
- **Preconditions:** Both streams contain distinct lines.
- **Scenario:** Run `park logs <name> --stderr`.
- **Acceptance criteria:** Only stderr bytes are returned; stdout bytes are
  absent; the command succeeds for empty stderr as well.

### PARK-LOG-004: Reject conflicting stream selectors

- **Scope:** CLI usage errors
- **Priority:** P1
- **Actor:** Script
- **Story:** As a script author, I want conflicting log options rejected, so
  that the selected stream is never ambiguous.
- **Preconditions:** A record exists.
- **Scenario:** Run `park logs <name> --stdout --stderr`.
- **Acceptance criteria:** The command exits `2`; no log request is performed;
  stderr explains the option conflict.

### PARK-LOG-005: Return a bounded head of logs

- **Scope:** Log slicing
- **Priority:** P1
- **Actor:** Developer
- **Story:** As a developer, I want the first `N` retained lines, so that I can
  inspect startup output quickly.
- **Preconditions:** A record has more than `N` lines.
- **Scenario:** Run `park logs <name> --head N`.
- **Acceptance criteria:** Exactly the first `N` matching retained lines are
  returned; line content and line endings are preserved; `--head 0` returns
  empty content.

### PARK-LOG-006: Return a bounded tail of logs

- **Scope:** Log slicing
- **Priority:** P1
- **Actor:** Developer
- **Story:** As a developer, I want the last `N` retained lines, so that I can
  inspect the most recent output efficiently.
- **Preconditions:** A record has more than `N` lines.
- **Scenario:** Run `park logs <name> --tail N`.
- **Acceptance criteria:** Exactly the last `N` matching retained lines are
  returned; fewer than `N` lines returns all available lines; `--tail 0`
  returns empty content.

### PARK-LOG-007: Apply literal grep before head or tail

- **Scope:** Log filtering
- **Priority:** P1
- **Actor:** Developer
- **Story:** As a developer, I want to filter retained lines before slicing,
  so that `--tail` means the tail of the matching results.
- **Preconditions:** Matching and non-matching lines are present.
- **Scenario:** Run `park logs <name> --grep literal --tail 1` and repeat with
  `--head 1`.
- **Acceptance criteria:** Matching is literal substring search, not regex;
  filtering occurs before head/tail selection; only matching lines are emitted.

### PARK-LOG-008: Handle empty and invalid-byte logs

- **Scope:** Log rendering
- **Priority:** P2
- **Actor:** Developer
- **Story:** As a developer, I want empty output to be a successful empty result
  and invalid bytes to remain inspectable, so that arbitrary Unix output does
  not break log commands.
- **Preconditions:** One command emits no output; another emits invalid UTF-8.
- **Scenario:** Read both records in human and JSON modes.
- **Acceptance criteria:** Empty logs exit `0` with empty content; invalid bytes
  are retained on disk; human and JSON rendering completes without crashing.

### PARK-LOG-009: Follow output from an active process

- **Scope:** Streaming logs
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want new output streamed as it arrives, so that
  I can watch a parked process without attaching to it.
- **Preconditions:** A process emits output in multiple phases and remains
  alive between phases.
- **Scenario:** Run `park logs <name> --follow` while the process emits the
  phases.
- **Acceptance criteria:** Retained initial output is emitted first; subsequent
  output appears as appended; stdout/stderr selection and literal grep behave
  as requested; initial head/tail limits apply only to the retained snapshot;
  the client remains non-interactive.

### PARK-LOG-010: End follow when the process terminates

- **Scope:** Streaming logs
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want follow to finish automatically, so that a
  script never waits forever after the process exits.
- **Preconditions:** A followed process eventually exits.
- **Scenario:** Follow its logs until termination.
- **Acceptance criteria:** All available output is delivered; the stream ends
  cleanly; the command exits `0`; the final state is observable as terminal in
  JSON mode or through a subsequent status query.

### PARK-LOG-011: Preserve appended logs across restart and start

- **Scope:** Log retention and lifecycle
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want one retained history across relaunches, so
  that restart diagnostics are not lost.
- **Preconditions:** A command writes distinct output on each invocation.
- **Scenario:** Let it exit; run `restart`; let it exit; run `start`; let it
  exit; read stdout and stderr.
- **Acceptance criteria:** Output from every invocation is present in order
  within its selected stream; logs are appended rather than truncated; status
  reflects the latest terminal run.

### PARK-LOG-012: Return structured log JSON

- **Scope:** Machine-readable logs
- **Priority:** P0
- **Actor:** Coding agent
- **Story:** As an automation client, I want log content and metadata in JSON,
  so that I can consume logs without parsing human output.
- **Preconditions:** A record has known output.
- **Scenario:** Run `park logs <name> --stdout --json`, then combined and
  filtered JSON variants.
- **Acceptance criteria:** JSON contains `status`, `ok`, and `data` with
  `stream`, `content`, and `state`; selected and filtered content matches the
  non-JSON command; stdout contains only JSON.

## Lifecycle Control

### PARK-LIFE-001: Stop gracefully

- **Scope:** Stop
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want `stop` to request graceful termination, so
  that a well-behaved process can clean up before exiting.
- **Preconditions:** A running process handles SIGTERM and records its cleanup.
- **Scenario:** Run `park stop <name>` and inspect status and output.
- **Acceptance criteria:** Stop exits `0`; SIGTERM is sent to the managed group;
  cleanup output is captured; the record reaches a terminal state; the command
  does not return before termination is observed.

### PARK-LIFE-002: Escalate a stubborn process

- **Scope:** Stop timeout
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want a process that ignores SIGTERM forcibly
  terminated after the grace period, so that `stop` cannot hang indefinitely.
- **Preconditions:** A process ignores SIGTERM and remains alive.
- **Scenario:** Run `park stop <name>`.
- **Acceptance criteria:** Park waits approximately the documented two-second
  grace period, sends SIGKILL to the managed group, exits successfully after
  termination, and records state `killed` with a termination signal.

### PARK-LIFE-003: Force stop immediately

- **Scope:** Forceful stop
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want `stop --force` to skip graceful waiting, so
  that I can terminate an unresponsive process immediately.
- **Preconditions:** A running process exists.
- **Scenario:** Run `park stop <name> --force`.
- **Acceptance criteria:** SIGKILL is sent without the graceful wait; the group
  terminates; the record reaches `killed`; the command exits `0`.

### PARK-LIFE-004: Reject stop for a terminal record

- **Scope:** Invalid lifecycle transitions
- **Priority:** P0
- **Actor:** Script
- **Story:** As a script author, I want invalid lifecycle actions distinguished,
  so that I can tell an already-finished process from an operational failure.
- **Preconditions:** A record is `exited`, `failed`, or `killed`.
- **Scenario:** Run `park stop <name>`.
- **Acceptance criteria:** The command exits `5`; the terminal record is not
  overwritten; its exit code, signal, and logs remain unchanged.

### PARK-LIFE-005: Send each supported named signal

- **Scope:** Signal control
- **Priority:** P1
- **Actor:** Developer
- **Story:** As a developer, I want named Unix signals, so that I can ask a
  parked process to reload, interrupt, continue, or terminate itself.
- **Preconditions:** A running process records the received signal and remains
  available for repeated test runs.
- **Scenario:** Exercise `HUP`, `INT`, `QUIT`, `TERM`, `USR1`, `USR2`, `STOP`,
  `CONT`, and `KILL`, with and without the `SIG` prefix where applicable.
- **Acceptance criteria:** Each supported spelling is accepted and targeted at
  the managed process group; the process observes the signal; terminal signals
  produce the documented terminal state.

### PARK-LIFE-006: Reject unsupported and numeric signals

- **Scope:** Signal validation
- **Priority:** P1
- **Actor:** Script
- **Story:** As a script author, I want unsupported signal values rejected, so
  that a typo cannot target an unintended signal.
- **Preconditions:** A running record exists.
- **Scenario:** Run `park signal <name> 9` and use an unknown signal name.
- **Acceptance criteria:** Each request exits `1`; stderr identifies the
  supported named values; the process remains unaffected and active.

### PARK-LIFE-007: Restart a terminal record from its saved command

- **Scope:** Restart
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want to restart a retained record, so that I can
  rerun it without repeating its command line.
- **Preconditions:** A record is terminal and its executable remains runnable.
- **Scenario:** Run `park restart <name>` and inspect the new run.
- **Acceptance criteria:** Restart exits `0`; the saved executable, arguments,
  and working directory are used; lifecycle fields reset for the new run; logs
  append; the record remains under the same key.

### PARK-LIFE-008: Restart an active record safely

- **Scope:** Restart and serialization
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want restart to stop an active process before
  relaunching it, so that two copies never overlap under one name.
- **Preconditions:** A running record exists.
- **Scenario:** Run `park restart <name>` and observe process generations.
- **Acceptance criteria:** The original process group is stopped first; a new
  run starts from the recorded command; only one active group exists; the
  latest run owns the record and appended logs.

### PARK-LIFE-009: Start only a retained terminal record

- **Scope:** Start
- **Priority:** P1
- **Actor:** Developer
- **Story:** As a developer, I want `start` to relaunch a stopped historical
  record, so that I can resume it without supplying a command.
- **Preconditions:** A terminal record exists.
- **Scenario:** Run `park start <name>`; then try `park start` while the record
  is active.
- **Acceptance criteria:** Start succeeds for terminal records and appends
  output; start on an active record exits `5`; no duplicate process is created.

### PARK-LIFE-010: Remove a terminal record and its logs

- **Scope:** Remove
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want to remove a completed record and its logs,
  so that I can clean up obsolete local state.
- **Preconditions:** The record is terminal and its process group is gone.
- **Scenario:** Run `park rm <name>`; then query status and logs.
- **Acceptance criteria:** Remove exits `0`; status and logs return missing
  record with exit `3`; metadata and both log files are gone; unrelated records
  and logs remain.

### PARK-LIFE-011: Remove a record while keeping logs

- **Scope:** Remove retention option
- **Priority:** P1
- **Actor:** Developer
- **Story:** As a developer, I want to delete metadata while retaining logs, so
  that I can preserve diagnostics independently.
- **Preconditions:** A terminal record and its two log files exist.
- **Scenario:** Run `park rm <name> --keep-logs`.
- **Acceptance criteria:** The record is no longer addressable; the log files
  remain; the command exits `0`; the retained files are not mistaken for an
  active process or a new record.

### PARK-LIFE-012: Refuse removal of an active record

- **Scope:** Removal safety
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want active records protected from `rm`, so that
  metadata cannot be deleted while a process is still running.
- **Preconditions:** A running record exists.
- **Scenario:** Run `park rm <name>` and then query status.
- **Acceptance criteria:** Remove exits `5`; the process and record remain
  active; logs remain; the user must stop the process before removal.

### PARK-LIFE-013: Clean terminal records globally

- **Scope:** Cleanup
- **Priority:** P1
- **Actor:** Developer
- **Story:** As a developer, I want `clean` to remove obsolete terminal records
  across my Park state, so that cleanup does not depend on revisiting each
  project.
- **Preconditions:** Terminal records exist in multiple projects and at least
  one active record exists.
- **Scenario:** Run `park clean`, then inspect all projects.
- **Acceptance criteria:** `clean` exits `0`; eligible terminal records and logs
  are removed; active records are retained; the result reports the number
  removed; unrelated state is unchanged.

### PARK-LIFE-014: Serialize concurrent lifecycle mutations

- **Scope:** Lifecycle concurrency
- **Priority:** P0
- **Actor:** Coding agent
- **Story:** As an automation client, I want concurrent stop, restart, signal,
  start, and remove requests serialized per record, so that races cannot corrupt
  state or create overlapping groups.
- **Preconditions:** A record exists and multiple clients can issue requests.
- **Scenario:** Race two or more lifecycle operations against the same name.
- **Acceptance criteria:** Responses are stable lifecycle outcomes; no terminal
  record is overwritten by stale data; at most one active process group remains;
  metadata and logs are internally consistent.
- **Test level:** Linux system/stress test. The standalone binary runner can
  drive the race, but proving the process-group and stale-update invariants
  requires system-level inspection beyond ordinary CLI assertions.

## Waiting And Agent Coordination

### PARK-WAIT-001: Wait for an exact state

- **Scope:** `wait --state`
- **Priority:** P0
- **Actor:** Coding agent
- **Story:** As an automation client, I want to wait until a record reaches an
  exact state, so that dependent steps can start at a known lifecycle point.
- **Preconditions:** A command transitions through the requested state.
- **Scenario:** Run `park wait <name> --state running` and
  `park wait <name> --state exited`.
- **Acceptance criteria:** Each wait exits `0` when the exact state is observed
  and returns the matching record; a different state does not satisfy the wait.

### PARK-WAIT-002: Wait for any terminal exit

- **Scope:** `wait --exit`
- **Priority:** P0
- **Actor:** Coding agent
- **Story:** As an automation client, I want to wait for any terminal outcome,
  so that I can continue whether a process succeeds, fails, or is killed.
- **Preconditions:** A running command will terminate with a known outcome.
- **Scenario:** Run `park wait <name> --exit`.
- **Acceptance criteria:** The wait exits `0` for `exited`, `failed`, or `killed`;
  returned data identifies the final state and exit details; it does not return
  early for a non-terminal state.

### PARK-WAIT-003: Wait for literal output in either stream

- **Scope:** `wait --match`
- **Priority:** P0
- **Actor:** Coding agent
- **Story:** As an automation client, I want to wait for a readiness marker in
  stdout or stderr, so that I can coordinate with a development server.
- **Preconditions:** A process emits a known marker after a delay.
- **Scenario:** Run `park wait <name> --match ready --timeout 2s`.
- **Acceptance criteria:** The wait exits `0` when the literal byte substring
  appears in either stream; the returned record is current; regex metacharacters
  are treated literally.

### PARK-WAIT-004: Match historical output

- **Scope:** Wait and retained logs
- **Priority:** P1
- **Actor:** Coding agent
- **Story:** As an automation client, I want a match to include already-retained
  output, so that a late observer does not miss a readiness marker.
- **Preconditions:** A terminal record's logs already contain the marker.
- **Scenario:** Run `park wait <name> --match marker` after the process exits.
- **Acceptance criteria:** The wait succeeds immediately; the marker may be in
  stdout or stderr; no new process activity is required.

### PARK-WAIT-005: Match output appended after restart

- **Scope:** Wait and log history
- **Priority:** P1
- **Actor:** Coding agent
- **Story:** As an automation client, I want matching to cover later starts and
  restarts, so that the complete retained history is observable.
- **Preconditions:** A retained record has multiple lifecycle runs.
- **Scenario:** Add a marker during a later run and wait for that marker.
- **Acceptance criteria:** The wait succeeds for output appended by the later
  run; both streams and all retained runs participate in matching.

### PARK-WAIT-006: Treat an empty match as immediate

- **Scope:** Wait validation
- **Priority:** P2
- **Actor:** Script
- **Story:** As a script author, I want an empty literal match to be defined, so
  that generated conditions do not hang unexpectedly.
- **Preconditions:** A record exists.
- **Scenario:** Run `park wait <name> --match ""`.
- **Acceptance criteria:** The wait succeeds immediately and returns the current
  record.

### PARK-WAIT-007: Honor millisecond, second, and minute timeouts

- **Scope:** Wait duration parsing
- **Priority:** P1
- **Actor:** Coding agent
- **Story:** As an automation client, I want readable timeout units, so that
  coordination deadlines are explicit.
- **Preconditions:** A process will not satisfy the requested condition before
  the deadline.
- **Scenario:** Use `--timeout 0ms`, `--timeout 0s`, and `--timeout 0m` to verify
  all units, then use a short non-zero timeout for elapsed-time behavior.
- **Acceptance criteria:** Each suffix is accepted; the wait stops around the
  requested duration; timeout is a generic failure with exit `1`.

### PARK-WAIT-008: Reject invalid wait conditions

- **Scope:** CLI usage errors
- **Priority:** P0
- **Actor:** Script
- **Story:** As a script author, I want exactly one wait condition required, so
  that a wait never has ambiguous semantics.
- **Preconditions:** None.
- **Scenario:** Run wait with no condition and with two conditions; use an
  invalid state and invalid duration.
- **Acceptance criteria:** Each malformed invocation exits `2`; no wait request
  is sent; stderr identifies the usage problem.

### PARK-WAIT-009: Disconnect a wait client safely

- **Scope:** IPC streaming and concurrency
- **Priority:** P1
- **Actor:** Coding agent
- **Story:** As an automation client, I want to cancel a wait by disconnecting,
  so that abandoned waits do not block lifecycle operations.
- **Preconditions:** A long-running record exists.
- **Scenario:** Start `wait --exit`, terminate the waiting client, then query
  status and stop the managed process.
- **Acceptance criteria:** The daemon releases the abandoned stream; status and
  stop remain responsive; process capture and monitoring continue normally.

## Daemon, Persistence, And Recovery

### PARK-DAEMON-001: Start the daemon on first use

- **Scope:** Daemon startup
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want Park to start its daemon automatically, so
  that routine commands require no manual service setup.
- **Preconditions:** No daemon is running and the isolated socket is absent.
- **Scenario:** Run `park ps --json`.
- **Acceptance criteria:** The command succeeds; subsequent requests can use
  the same daemon state; no foreground daemon process is required. Do not count
  daemon processes as part of this black-box story.

### PARK-DAEMON-002: Share one daemon among concurrent first clients

- **Scope:** Daemon ownership
- **Priority:** P1
- **Actor:** Coding agent
- **Story:** As an automation client, I want concurrent first requests to share
  one owner, so that startup races do not create competing daemons.
- **Preconditions:** Runtime state is fresh.
- **Scenario:** Start several Park clients simultaneously.
- **Acceptance criteria:** All valid clients complete; requests observe
  consistent registry state; no request requires manual daemon startup. Daemon
  owner counting belongs to system-level coverage.

### PARK-DAEMON-003: Recover stale runtime markers

- **Scope:** Daemon recovery
- **Priority:** P1
- **Actor:** Developer
- **Story:** As a developer, I want stale socket and PID marker files replaced
  safely, so that a previous crash does not prevent future use.
- **Preconditions:** No live daemon owns the endpoint; stale runtime artifacts
  exist.
- **Scenario:** Run a normal Park command.
- **Acceptance criteria:** A new daemon starts and serves the request; stale
  artifacts do not cause a false successful connection; live ownership is not
  removed merely because marker files look stale.
- **Test level:** Linux daemon-recovery/system test because it prepares and
  inspects runtime artifacts outside the normal user CLI.

### PARK-DAEMON-004: Use XDG state and runtime locations

- **Scope:** Filesystem layout
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want Park state isolated under XDG directories,
  so that state is predictable and does not pollute arbitrary project paths.
- **Preconditions:** Custom XDG directories are writable.
- **Scenario:** Launch a command with custom `XDG_STATE_HOME` and
  `XDG_RUNTIME_DIR`; inspect the directories.
- **Acceptance criteria:** SQLite metadata and logs are under the state
  directory; socket, lock, and PID marker are under runtime; no hard-coded home
  location is used.

### PARK-DAEMON-005: Fall back when runtime environment is unavailable

- **Scope:** Filesystem layout
- **Priority:** P1
- **Actor:** Developer
- **Story:** As a developer, I want Park to work without `XDG_RUNTIME_DIR`, so
  that minimal Unix environments remain usable.
- **Preconditions:** The state fallback is writable.
- **Scenario:** First run with `XDG_RUNTIME_DIR` unset or empty. Then run a
  separate case with `XDG_RUNTIME_DIR` set to an unusable path.
- **Acceptance criteria:** With the variable unset or empty, Park starts
  successfully and uses the documented private state fallback for runtime
  files. With an unusable configured path, the test asserts the documented
  failure rather than assuming an additional fallback.
- **Test note:** Keep the two environment cases separate; they have different
  public outcomes in the current version.

### PARK-DAEMON-006: Reconcile an active record after daemon loss

- **Scope:** Crash recovery
- **Priority:** P0
- **Actor:** Operating system and developer
- **Story:** As a developer, I want daemon loss to be safe, so that managed
  groups do not outlive their owner and stale active records become inspectable.
- **Preconditions:** A running process is managed by the daemon.
- **Scenario:** Kill the daemon abruptly, wait for the managed group to end,
  start a new client, and inspect status and logs.
- **Acceptance criteria:** On Linux, the managed group is terminated; the next
  daemon reconciles the record to a terminal state; logs remain intact; no
  record is silently discarded.
- **Test level:** Linux system/recovery test. It intentionally kills the daemon
  through the documented runtime marker before reconnecting with a new client.

### PARK-DAEMON-007: Keep records after normal process exit

- **Scope:** Persistence
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want historical records retained, so that status,
  logs, and exit details remain available after a process finishes.
- **Preconditions:** A process exits normally or with failure.
- **Scenario:** Wait for exit, allow the daemon/client boundary to be crossed,
  then query status and logs.
- **Acceptance criteria:** The record remains present with terminal state and
  exit metadata; both log streams remain readable until `rm` or `clean`.

### PARK-DAEMON-008: Recover an interrupted pre-spawn log artifact

- **Scope:** Persistence recovery
- **Priority:** P2
- **Actor:** Script
- **Story:** As a script author, I want a stale log-only artifact not to block a
  new launch, so that an interrupted launch can be retried safely.
- **Preconditions:** Key-derived log files exist without a corresponding
  record.
- **Scenario:** Launch the same name with a valid command.
- **Acceptance criteria:** The launch succeeds; only artifacts for that exact
  key are recreated; the new record and logs are coherent; unrelated logs are
  untouched.

### PARK-DAEMON-009: Keep slow log readers from blocking capture

- **Scope:** IPC backpressure
- **Priority:** P1
- **Actor:** Coding agent
- **Story:** As an automation client, I want a slow logs reader not to stop the
  child, so that output capture remains reliable under backpressure.
- **Preconditions:** A command emits substantial output; a log client reads
  slowly or pauses.
- **Scenario:** Read logs slowly while the process runs, then inspect status and
  final log size.
- **Acceptance criteria:** The child can finish; the final retained payload
  equals the fixture payload; lifecycle and status operations remain
  responsive; no daemon-wide stall occurs.

### PARK-DAEMON-010: Keep malformed or oversized IPC isolated

- **Scope:** IPC hardening
- **Priority:** P2
- **Actor:** Fault-injecting client
- **Story:** As a local client, I want malformed requests rejected without
  destabilizing Park, so that a bad integration cannot corrupt daemon work.
- **Preconditions:** A daemon is serving a valid record.
- **Scenario:** Send malformed, unterminated, protocol-version-mismatched, and
  oversized newline-delimited messages over the local socket, then issue a
  normal CLI request.
- **Acceptance criteria:** Invalid requests are rejected; bounded parsing and
  response deadlines prevent unbounded blocking; the daemon continues serving
  valid requests; managed output capture is unaffected.
- **Test level:** Protocol/daemon integration test, not standalone binary e2e.
  It connects directly to the Unix socket and should live in a separate
  hardening suite.

### PARK-DAEMON-011: Preserve state across daemon reconnects

- **Scope:** Persistence and client reconnection
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want a new Park client to see the same registry,
  so that terminal sessions are interchangeable.
- **Preconditions:** A record and logs exist.
- **Scenario:** Use one client to launch, terminate or disconnect it, then use a
  separate client with the same XDG directories to inspect and control the
  record.
- **Acceptance criteria:** The second client sees identical key, state, command,
  and logs; no duplicate record or daemon-owned state is created.

## Cross-Cutting Error And Safety Stories

### PARK-SAFETY-001: Duplicate protection coverage reference

- **Scope:** Data safety
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a test maintainer, I want duplicate protection covered by one
  canonical scenario, so that the same behavior is not tested twice under
  different IDs.
- **Preconditions:** See `PARK-SCOPE-002`.
- **Scenario:** Execute `PARK-SCOPE-002` for both active and terminal records.
- **Acceptance criteria:** This ID is a traceability reference only; the
  executable e2e scenario and assertions live under `PARK-SCOPE-002`.
- **Test level:** Cross-reference, not an independent test.

### PARK-SAFETY-002: Protect process identity from PID reuse

- **Scope:** Linux ownership safety
- **Priority:** P1
- **Actor:** Operating system
- **Story:** As a developer, I want Park to avoid signaling an unrelated reused
  PID or process group, so that lifecycle actions cannot damage another process.
- **Preconditions:** A record's PID identity is stale or a PID has been reused;
  the unrelated process is observable by the fixture.
- **Scenario:** Trigger reconciliation and attempt status, stop, signal, remove,
  and clean operations.
- **Acceptance criteria:** Park does not treat the unrelated process as owned;
  it does not signal it; the Park record is reconciled conservatively; active
  records are not removed while a verified group remains.
- **Test level:** Linux fault-injection/security test, not ordinary standalone
  binary e2e. Deterministic PID reuse is not practical to guarantee in the
  Docker scenario runner.

### PARK-SAFETY-003: Preserve lifecycle terminal outcomes exactly once

- **Scope:** Monitoring and persistence
- **Priority:** P0
- **Actor:** Coding agent
- **Story:** As an automation client, I want one authoritative terminal result,
  so that status cannot change after a later run begins.
- **Preconditions:** A record can exit while a lifecycle request or monitor
  update is in flight.
- **Scenario:** Race natural exit with stop, restart, or start, then inspect the
  final record and logs.
- **Acceptance criteria:** Each run has one terminal transition; a stale monitor
  update cannot overwrite a later run; exit code/signal and timestamps belong to
  the correct run.
- **Test level:** Concurrency/system test. Keep it outside the basic binary-e2e
  story set unless the runner has explicit invariant inspection support.

### PARK-SAFETY-004: Keep human output script-friendly

- **Scope:** CLI output
- **Priority:** P0
- **Actor:** Script
- **Story:** As a script author, I want commands to be non-interactive and
  predictable, so that Park can run in CI and coding-agent workflows.
- **Preconditions:** Commands are run with stdout/stderr captured and no TTY.
- **Scenario:** Exercise successful, missing, duplicate, invalid-state, usage,
  timeout, and generic failure paths.
- **Acceptance criteria:** No command prompts; success data is on stdout;
  diagnostics are on stderr; exit codes match the public vocabulary; follow and
  wait terminate without user input.
- **Test level:** Suite-level quality rule. Apply these assertions to each
  concrete CLI story rather than implementing this as one standalone test.

### PARK-SAFETY-005: Keep JSON free of human diagnostics

- **Scope:** Machine-readable output
- **Priority:** P0
- **Actor:** Coding agent
- **Story:** As an automation client, I want JSON output parseable even on
  failure, so that I can handle errors by status code and structured fields.
- **Preconditions:** JSON-capable commands can be made to succeed and fail.
- **Scenario:** Run `ps`, `status`, and `logs` in success and missing-record
  cases with `--json`.
- **Acceptance criteria:** stdout contains exactly one valid JSON result; error
  results include `status`, `ok: false`, and a structured `error`; no prose is
  prepended or appended to JSON stdout.

### PARK-SAFETY-006: Keep unrelated project state isolated during cleanup

- **Scope:** Cleanup safety
- **Priority:** P0
- **Actor:** Developer
- **Story:** As a developer, I want cleanup to affect only Park-managed eligible
  records, so that other projects and files are never deleted accidentally.
- **Preconditions:** Multiple projects, active records, terminal records, and
  unrelated files exist under the test state root.
- **Scenario:** Run `rm` and `clean` in different projects and inspect all
  records, logs, and unrelated files.
- **Acceptance criteria:** Only the targeted or eligible Park records and their
  key-derived logs are removed; active groups and unrelated files remain.

## Explicitly Out Of Scope For Current E2E Tests

The following capabilities are not yet implemented, so they are not missing test
cases for the current contract:

- Project configuration files and `park up` / `park down` orchestration.
- Git-root project resolution as an alternate policy.
- Automatic process restart after an operating-system reboot.
- Windows support and equivalent cross-platform process ownership guarantees.
- Log rotation, retention limits, pruning policy, and compression.
- Structured log envelopes and separate event/observed/ingested timestamps.
- External or cloud log export.
- Numeric signal syntax.

When one of these capabilities is implemented, add new stories with a new
reference prefix rather than changing the acceptance criteria above without a
contract update.
