# Commands

Park's command interface is configuration-free by default. Run commands from
the project directory whose process records you want to manage.

The primary launch form is:

```text
park <name> -- <command> [arguments...]
```

Use the command pages for the complete behavior of each operation:

- [Launch](launch.md) a named command and understand argument boundaries.
- [Lifecycle](lifecycle.md) inspect, stop, restart, start, signal, remove, and
  clean records.
- [Logs](logs.md) inspect, filter, and follow retained output.
- [Scripting](scripting.md) use JSON output, exit codes, and wait conditions.
- [Configuration](../configuration.md) describes optional user-scoped daemon and
  restart policies.
- [AI Agent Integration](../ai-agents.md) install the Park skill and discover the
  agent workflow with `park help --skills`.
