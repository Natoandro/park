# Code Review Checklist

Use this checklist for recurring reviews of Park changes. Reviewers should focus on behavior, safety, and public-contract regressions before style preferences.

## Scope And Contract

- [ ] The change preserves the primary identity: canonical `(project directory, name)`.
- [ ] No command silently treats process names as globally unique.
- [ ] README, architecture documents, and implementation plan accurately describe implemented behavior.
- [ ] Public commands, aliases, JSON fields, and exit codes are either implemented or clearly documented as deferred.
- [ ] Human output remains non-interactive and script-friendly.
- [ ] JSON output does not expose undocumented persistence internals as a stable API accidentally.

## CLI And Project Resolution

- [ ] Opaque names and exact OS command arguments remain lossless, including non-UTF-8 Unix values where supported.
- [ ] `--` command boundaries and dash-prefixed names are parsed without ambiguity regressions.
- [ ] The CLI canonicalizes the invocation directory before creating lookup keys.
- [ ] The daemon independently canonicalizes every project path received over IPC.
- [ ] Relative, nonexistent, non-directory, and symlink-alias project paths have intentional, tested behavior.

## IPC And Daemon Startup

- [ ] IPC requests and responses are bounded before untrusted input can allocate unbounded memory.
- [ ] Newline framing, protocol version, and request ID are validated on both sides.
- [ ] Socket failures start a daemon only for absent or refused endpoints; permission and protocol faults are surfaced directly.
- [ ] Daemon ownership still relies on the advisory lock, not PID markers or socket-file existence alone.
- [ ] Runtime files and directories are private to the user and stale-endpoint recovery cannot remove a live daemon endpoint.
- [ ] Slow, malformed, or disconnected clients cannot block child monitoring or consume unbounded daemon resources.

## Process Lifecycle And Safety

- [ ] A launch is reserved or otherwise serialized by complete process key through its creation transaction.
- [ ] Duplicate launches consistently return the documented duplicate-record result.
- [ ] Commands are spawned directly from the stored executable and argument vector, without an implicit shell.
- [ ] The recorded working directory matches the canonical project key.
- [ ] Managed commands run in a dedicated process group/session where supported.
- [ ] Daemon loss cannot orphan a managed process group on supported platforms.
- [ ] PID, start-time, process-group, and session identity are verified before reconciliation or group signaling.
- [ ] Persisted process identifiers are validated before conversion to platform PID types.
- [ ] Lifecycle transitions cannot overwrite terminal states or make live processes removable.

## Output Capture And Monitoring

- [ ] Stdout and stderr remain separate and are drained independently.
- [ ] Capture failures terminate or otherwise safely contain the managed process group.
- [ ] Wait, capture, and terminal-persistence failures are recorded as durable failures.
- [ ] Terminal record persistence is retried rather than silently discarded after transient storage errors.
- [ ] Child output cannot deadlock because a log reader or IPC client is slow.
- [ ] Exit codes and signal termination are persisted exactly once with intentional state semantics.

## Persistence And Filesystem Safety

- [ ] Every loaded record is validated before listing, reconciliation, lifecycle actions, or removal.
- [ ] Record key, record filename, working directory, lifecycle fields, timestamps, and log paths agree.
- [ ] Removal deletes only key-derived logs after validating a terminal record.
- [ ] Record creation and replacement use exclusive temporary files, atomic publication, file sync, and parent-directory sync.
- [ ] Stale temporary files and stale log-only artifacts have a safe recovery path.
- [ ] Concurrent record mutations cannot allow stale state to replace a later terminal transition.
- [ ] XDG state and runtime paths are used without hard-coded home-directory paths.

## Tests And Verification

- [ ] New behavior has focused unit tests and an integration test when it crosses CLI, IPC, daemon, or filesystem boundaries.
- [ ] Failure paths are tested: malformed IPC, spawn failure, capture failure, persistence failure, daemon crash, and duplicate launch where relevant.
- [ ] Process tests clean up daemons, process groups, files, and XDG directories reliably.
- [ ] `cargo fmt --all` passes.
- [ ] `cargo test` passes.
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes.
- [ ] `cargo build` and `git diff --check` pass.

## Review Outcome

- [ ] Findings include severity, file and line reference, failure mode, and a concrete remediation.
- [ ] Deferred findings are linked to an unchecked implementation-plan item.
- [ ] Tests or platform limits that could not be covered are called out explicitly.
