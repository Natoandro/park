# AI Agent Integration

Park includes a canonical agent skill for coding agents that need to run and
inspect long-lived development processes. The skill teaches agents to use
project-scoped names, inspect existing records, wait for readiness, preserve
stdout and stderr, and avoid disrupting processes owned by another actor.

The skill is distributed through the [`npx skills`](https://skills.sh/) CLI. It
is separate from the `park` executable: `npx skills` installs agent instructions,
while Park manages processes.

## Install The Skill

Run the project-level command from the repository where the skill should be
available:

```bash
npx skills add Natoandro/park --skill park -a opencode
```

Replace `opencode` with another supported agent, such as `codex`,
`claude-code`, or `cursor`. To install the skill for all projects, use the
global option:

```bash
npx skills add Natoandro/park --skill park -g -a opencode
```

For a one-off session without installing the skill:

```bash
npx skills use Natoandro/park --skill park --agent opencode
```

Review a skill before installing it, especially when it comes from a source you
do not control. The skill contains instructions for an agent; it does not grant
Park additional permissions or provide process isolation.

## Update Or Remove It

List installed skills, update Park's skill, or remove it through the same CLI:

```bash
npx skills list
npx skills update park
npx skills remove park
```

## Recommended Agent Workflow

The operational interface remains Park itself. Agents should:

1. Run from the project directory associated with the process.
2. Inspect existing records with `park ps --json` before choosing a name.
3. Launch with `park <name> -- <command> [arguments...]`.
4. Wait for `running` or a literal readiness message with `park wait`.
5. Use `park status <name> --json` and `park logs <name>` to diagnose failures.
6. Stop or remove only records that belong to the task or that the user asked to control.

See [Scripting](commands/scripting.md) for JSON envelopes, exit codes, and wait
semantics. See [Quick Start](quick-start.md) for the complete command workflow.

## Discovery

The installed skill is the detailed behavioral guide. Park's normal CLI help and
the documentation remain the source of truth for the commands supported by the
installed Park version. Do not assume that a newer skill adds commands that the
installed binary does not expose.
