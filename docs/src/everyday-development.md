# Everyday Development

Park does not require an AI coding agent. It is useful whenever a local command
needs to keep running after the terminal that started it is gone.

## The Basic Handoff

Start a process from the project directory where it should be managed:

```bash
park api -- ./bin/api --port 3000
```

You can close the terminal or move to another shell. Later, from the same
project directory, inspect and control the process:

```bash
park ps
park status api
park logs api --tail 100
park restart api
park stop api
```

The process name belongs to the canonical project directory. A process named
`api` in `~/code/shop` is independent from one named `api` in `~/code/billing`.
Park rejects a duplicate name in the same project instead of silently replacing
the existing process.

## Good Fits

### Development Servers

Keep an API, frontend, or preview server available while switching terminals,
branches, or tasks. Use `status` and `logs` instead of searching for a PID or
scrolling through an old terminal.

```bash
park web -- pnpm dev
park wait web --match 'ready' --timeout 30s
```

When a service needs project-local variables, let the daemon load them at
launch time:

```bash
park web --env-file .env --env-file .env.local -- pnpm dev
park env web
```

### Workers And Local Services

Run a queue worker, consumer, local database wrapper, or test service as a named
process. The command runs as a normal host process; Park does not isolate it or
manage production deployment.

```bash
park worker -- cargo run --bin worker
park service -- ./scripts/start-local-service
```

### Watchers And Repeatable Tasks

Use Park for documentation previews, file watchers, and other commands that are
useful across several sessions but do not justify a project manifest.

```bash
park docs -- mdbook serve docs
park watch -- ./scripts/watch-assets
```

### Scripts

Use stable exit codes and JSON inspection when a shell script needs to coordinate
with a long-running local command:

```bash
park api -- ./bin/api
park wait api --state running --timeout 5s
park status api --json
```

## Park Compared With Common Alternatives

- Use **Park** when you want named, project-scoped records, retained logs, and lifecycle control for ad-hoc commands.
- Use **tmux** when you want an interactive terminal session and live multiplexing.
- Use **`nohup` or shell backgrounding** when you only need to detach a command and do not need a durable process record.
- Use **Docker Compose or a project runner** when you want a configured group of services, networking, or reproducible orchestration.
- Use **systemd or a production supervisor** for boot-time services, privileged management, and production reliability policies.

Park is intentionally between shell backgrounding and configured orchestration:
it adds durable identity and control without requiring a manifest.

## Not A Fit Yet

Park may not be the right tool if you need Windows support, process isolation,
automatic restart policies, boot-time recovery, multi-service orchestration, or
cross-platform process-ownership guarantees. Those boundaries are explicit so a
Park-managed command does not look more reliable than it is.

## Try It And Tell Us

The most useful feedback is concrete: what command you parked, what you used
before, whether you returned to it on another day, and where Park got in the way.
That evidence will determine whether Park should remain a focused CLI or grow
additional orchestration features.
