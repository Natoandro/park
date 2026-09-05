# Park Agent Guide

- The implementation is a Rust workspace with a `park` executable and focused unit/integration tests. Use the repository's documented Cargo checks; do not claim checks were run unless they were actually run.
- The implementation language is Rust. Before adding any third-party crate that is not near-universal Rust tooling (for example, `clap` or `serde`), present its purpose and alternatives and obtain explicit user approval. Record approved dependencies and their rationale in the implementation plan or manifest comments.
- `README.md` defines the user-facing product contract. `docs/architecture.md` defines component boundaries; `docs/low-level-architecture.md` defines persistence, IPC, and lifecycle invariants. Keep all three aligned when changing behavior.
- The crates.io package is `park-cli`; the installed binary and every user-facing command use `park`.
- A managed process is uniquely identified by canonicalized `(project directory, name)`. Never make names globally unique or silently replace an existing record.
- Park is configuration-free by default and development-machine scoped. Preserve ad-hoc `park <name> -- <command> [args...]` as the primary interface; configuration and orchestration remain optional extensions.
- Preserve stdout and stderr separately, retain records and logs after exit, and make lifecycle operations target the process group where the platform supports it so child processes are not orphaned.
- Human output must remain script-friendly and non-interactive. JSON output and stable lifecycle exit-code semantics are first-class public behavior.
- Use XDG state/runtime directories rather than hard-coded home-directory paths. A daemon should start on demand and recover safely from stale runtime state.
- Keep source files focused and preferably under roughly 300 lines, including tests. Split files that exceed this size or combine unrelated responsibilities, while preferring a small number of cohesive modules over arbitrary fragmentation.
