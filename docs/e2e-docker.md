# Docker E2E Tests

Park's end-to-end scenarios run as an independent Rust binary in a Linux Docker
container. The container build stage provides Rust and Cargo to compile the
application and runner. The final runtime stage contains only the selected
`park` artifact, the `park-e2e` runner, and their operating-system runtime.

The first implemented scenario is `PARK-CLI-001` in `src/bin/park-e2e.rs`. It
does not use `cargo test` or the existing `tests/daemon_integration.rs` target.

## Files

- `docker/e2e/Dockerfile` defines the Linux Rust test image.
- `docker/e2e/entrypoint.sh` validates the runner root and executes the
  prebuilt Rust runner.
- `e2e-macros` provides the local `#[e2e(...)]` scenario metadata macro.
- `build.rs` discovers scenario modules and generates their module declarations
  and sorted registry at build time.
- `src/bin/park-e2e/scenarios/` contains one `.rs` file per scenario.
- `.dockerignore` keeps Git metadata and local build output out of the image
  context.
- `docs/e2e-user-stories.md` is the behavior catalog and acceptance-criteria
  source for the tests.

## Prerequisites

- Docker Engine or a compatible Docker CLI.
- A Linux container runtime. Docker Desktop's Linux VM is suitable, but the
  tests validate Linux-container behavior, not the host operating system.
- Network access while building the image so Cargo and apt can download their
  inputs. Test execution itself only uses local IPC and can run without a
  network.

## Build The Image

Build from the repository root:

```bash
docker build \
  --file docker/e2e/Dockerfile \
  --tag park-e2e:local \
  .
```

The default image uses Rust `1.85`, matching the project's MSRV. To exercise a
different compatible compiler explicitly:

```bash
docker build \
  --build-arg RUST_VERSION=1.85 \
  --file docker/e2e/Dockerfile \
  --tag park-e2e:local \
  .
```

The source tree is copied into the image. Cargo build artifacts stay inside
the disposable builder layer. Cargo is not present in the final runtime layer.

The default artifact profile is release. Build the debug artifact instead:

```bash
docker build \
  --build-arg BUILD_PROFILE=debug \
  --file docker/e2e/Dockerfile \
  --tag park-e2e:debug \
  .
```

## Run The E2E Suite

Run the standalone runner with a disposable container:

```bash
docker run --rm --init park-e2e:local
```

`--init` gives the container a minimal PID 1 that forwards signals and reaps
children. It is recommended because Park intentionally creates detached
daemon, supervisor, and managed-process descendants.

The runner currently executes `PARK-CLI-001`:

```bash
docker run --rm --init park-e2e:local
```

List registered scenarios without executing them:

```bash
docker run --rm --init park-e2e:local --list
```

Select scenarios by story ID, name, or description:

```bash
docker run --rm --init park-e2e:local --filter PARK-CLI-001
```

Select scenarios by tag:

```bash
docker run --rm --init park-e2e:local --tag smoke
```

Each scenario is registered by placing one `.rs` file in
`src/bin/park-e2e/scenarios/` and annotating its function:

```rust
#[e2e(
    story = "PARK-CLI-001",
    scope = "launch",
    priority = "P0",
    description = "Launch a named command and inspect its active record",
    tags = ["smoke", "cli", "lifecycle"]
)]
pub fn launch_named_command() -> Result<(), String> {
    // Scenario steps and assertions.
}
```

The macro emits a uniform `SCENARIO` metadata record and function pointer. The
Cargo build script discovers valid Rust module filenames, generates their
declarations, and builds the sorted registry automatically. This avoids a
manually maintained list while remaining deterministic and avoiding unsafe
linker-section registration. The runner currently executes selected scenarios
serially; parallel jobs can be added after each scenario's isolation and
cleanup contract is established.

## Recommended Isolation

For normal execution, add the following restrictions:

```bash
docker run --rm --init \
  --network none \
  --cap-drop=ALL \
  --security-opt=no-new-privileges \
  --pids-limit=256 \
  park-e2e:local
```

The suite should not require host PID, IPC, network, or filesystem access.
Do not use `--privileged`, `--pid=host`, or a host socket. A separate
container PID namespace is part of the test boundary and prevents a faulty
fixture from targeting host processes.

The entrypoint defaults to `/tmp/park-e2e`, validates that the configured root
is absolute, and makes it private. It does not recursively delete a caller-
provided path. The runner creates a unique child directory for each scenario
and exports scenario-specific values to every `park` subprocess:

- `HOME=<scenario-root>/home`
- `XDG_STATE_HOME=<scenario-root>/state`
- `XDG_RUNTIME_DIR=<scenario-root>/runtime`
- `PARK_E2E_SCENARIO=PARK-<story-id>`

The runner refuses a relative `PARK_BIN` or `PARK_E2E_ROOT`, creates the
scenario root with private permissions, and never uses the caller's XDG
directories. Individual scenarios must not write outside their scenario root
except to execute the configured `PARK_BIN` and standard fixture commands.

Cleanup is ordered as follows: stop or terminate managed records, terminate the
scenario daemon, wait for its process to exit, and only then remove the
scenario root. The Docker container remains a final safety boundary, but tests
must not rely on container teardown for ordinary cleanup.

## Local Iteration

The runtime image intentionally contains no source tree or Cargo. Rebuild the
image after changing the application or runner:

```bash
docker build \
  --file docker/e2e/Dockerfile \
  --tag park-e2e:local \
  .
```

Docker layer caching keeps unchanged dependency and build layers reusable. A
Cargo cache may be mounted into the builder explicitly, but it should remain
separate from the source and should not be shared between unrelated jobs
without builder-level locking.

## Test Design Rules

- Map every test to a `PARK-*` story in `docs/e2e-user-stories.md`.
- Use a fresh XDG state/runtime root per test or test fixture.
- Invoke the binary selected by `PARK_BIN`; do not depend on a host-installed
  binary or `CARGO_BIN_EXE_park`.
- Use `/bin/sh`, `/bin/sleep`, `/bin/true`, `/bin/false`, and temporary fixture
  commands already available in the Debian image.
- Assert observable CLI behavior, JSON fields, logs, lifecycle states, and exit
  codes. Do not assert fixed PIDs, timestamps, generated hashes, or container
  paths.
- Keep stdout and stderr assertions separate. JSON tests must parse stdout and
  verify that diagnostics are not mixed into it.
- Stop active records before a fixture exits. The fixture cleanup must also
  tolerate a daemon that has already exited.
- Avoid timing sleeps where a `wait` condition or bounded polling assertion can
  express the behavior.
- Make tests safe to run in an arbitrary order and more than once.

## Process And Platform Boundaries

The current implementation is Unix-first and uses Linux `/proc`, sessions, and
process groups for the strongest ownership checks. The Docker image therefore
provides the reference environment for process lifecycle stories. Tests that
exercise daemon crash cleanup, PID identity, descendant termination, or process
groups should be marked Linux-specific when they cannot be meaningfully
portable.

Docker does not make a non-Linux implementation Linux-equivalent. A test run on
Docker Desktop still runs inside a Linux VM and validates the Linux container,
not native macOS or Windows process semantics.

## Failure Diagnostics

When a run fails, rebuild and rerun the disposable image:

```bash
docker run --rm --init park-e2e:local
```

For a failing fixture, inspect the runner's captured command output and add
temporary diagnostics to the Rust scenario rather than attaching to a host
process. If a test leaves a process behind, remove the container and verify
that the next run uses a fresh container and fresh XDG directories.

The image currently has no CI-specific behavior. CI can call the same build and
run commands after the repository establishes its CI provider and required
verification policy.
