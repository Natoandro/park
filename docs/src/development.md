# Development Status

Park is an active-development Unix-first project. The package is `park-cli`,
the binary is `park`, and the current package version is `0.1.3`. The latest
release is published to crates.io; see [Installation](installation.md) for the
stable and latest-development installation commands.

The current design and implementation preserve a narrow contract: launch
configuration-free ad-hoc commands, identify them by canonical project path and
name, detach them from the launching terminal, retain separate stdout and
stderr, and provide non-interactive lifecycle control with stable JSON and exit
semantics.

## Available Now

The current feature set includes:

- Configuration-free launch of exact executable argument vectors.
- Canonical project-scoped names with duplicate protection.
- An on-demand per-user daemon independent of the launching terminal.
- Dedicated process groups and conservative Linux process-ownership checks.
- Durable process records with retained, separate stdout and stderr logs.
- Status, log inspection, filtering, following, signals, stop, restart, start,
  removal, cleanup, and wait operations.
- Stable JSON output and lifecycle exit codes for scripts and coding agents.

The normal launch form remains:

```bash
park <name> -- <command> [arguments...]
```

Park does not require a project manifest for routine use. Project resolution is
based on the canonical invocation directory; implicit Git-root detection is not
part of the current policy.

## Platform Limit

Park currently targets Unix. Linux has the strongest ownership and reconciliation
guarantees because Park can validate `/proc` start times, process groups, and
sessions. Other Unix targets retain the Unix interface but do not yet claim the
same process-identity verification across daemon restarts. Windows support is not
yet implemented.

Park does not provide process isolation or sandboxing. Managed commands run as
host processes and share host resources. Use containers or a virtual machine if
that boundary is required.

## Not Yet Implemented

The following capabilities are not yet implemented:

- Opt-in automatic restart policies with backoff and retry limits.
- Filesystem-triggered restarts for development workflows.
- Additional coordination support for shared human-and-agent workflows.
- Optional project configuration with `park up` and `park down`.
- Broader platform-specific process-ownership and lifecycle guarantees.
- Explicit reboot recovery policies.
- Log rotation, retention, pruning, and compression.
- Faster and richer log queries, structured log metadata, and optional external
  log export.

Changes must preserve configuration-free launches and the existing project/name
identity. Process isolation and sandboxing are not supported by Park's core
design.
