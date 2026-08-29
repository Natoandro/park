# Park Agent Guide

- This repository is currently a design baseline; no Rust workspace, test suite, formatter, CI configuration, or executable entrypoint exists yet. Do not invent verification commands or claim they were run.
- The implementation language is Rust. Before adding any third-party crate that is not near-universal Rust tooling (for example, `clap` or `serde`), present its purpose and alternatives and obtain explicit user approval. Record approved dependencies and their rationale in the implementation plan or manifest comments.
- `README.md` defines the user-facing product contract. `docs/architecture.md` defines component boundaries; `docs/low-level-architecture.md` defines persistence, IPC, and lifecycle invariants. Keep all three aligned when changing behavior.
- The crates.io package is `park-cli`; the installed binary and every user-facing command use `park`.
- A managed process is uniquely identified by canonicalized `(project directory, name)`. Never make names globally unique or silently replace an existing record.
- Park is configuration-free by default and development-machine scoped. Preserve ad-hoc `park <name> -- <command> [args...]` as the primary interface; configuration and orchestration remain optional extensions.
- Preserve stdout and stderr separately, retain records and logs after exit, and make lifecycle operations target the process group where the platform supports it so child processes are not orphaned.
- Human output must remain script-friendly and non-interactive. JSON output and stable lifecycle exit-code semantics are first-class public behavior.
- Use XDG state/runtime directories rather than hard-coded home-directory paths. A daemon should start on demand and recover safely from stale runtime state.
