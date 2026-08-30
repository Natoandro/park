# Architecture

Park separates a short-lived CLI client from a per-user daemon that owns
managed processes, persistence, output capture, and lifecycle control.

- [Overview](overview.md) explains the component boundaries, project-scoped
  identity, and local IPC.
- [Persistence and IPC State](persistence.md) documents XDG paths, records,
  logs, transactions, recovery, and the socket protocol.
- [Process Lifecycle](process-lifecycle.md) documents launch, monitoring,
  signals, reconciliation, ownership checks, and platform limits.
