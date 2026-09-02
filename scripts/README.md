# Maintenance Scripts

These scripts are primarily for maintainers. Contributors generally only need
`setup-hooks.sh` and the local checks in [Contributing](../CONTRIBUTING.md).

| Script | Purpose |
| --- | --- |
| `setup-hooks.sh` | Install the repository's local pre-commit hooks. |
| `check-version.sh` | Verify package, workspace, and documented versions match. |
| `bump-version.sh` | Update package versions, `Cargo.lock`, and the documented version. |
| `tag-version.sh` | Validate CI and create or push an annotated release tag. |
| `e2e.sh` | Build and run the Docker end-to-end scenarios. |

For the release sequence and documentation deployment policy, see the
[maintainer guide](../MAINTAINERS.md).
