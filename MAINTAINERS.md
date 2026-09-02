# Maintainer Guide

This document covers release and publication operations for Park maintainers.
External contributors should use [Contributing](CONTRIBUTING.md).

## Release Checklist

1. Finish the release changes on `master`.
2. Bump the workspace and documented versions:

   ```bash
   scripts/bump-version.sh patch
   # or: scripts/bump-version.sh 0.2.0
   ```

3. Review the generated changes, run the local checks, and commit the version
   bump.
4. Create and push the annotated release tag after CI passes:

   ```bash
   scripts/tag-version.sh --dry-run
   scripts/tag-version.sh --wait-ci --push
   ```

The tag script requires the clean, synchronized `master` branch, checks that the
workspace and documented versions agree, and verifies the GitHub Actions `Test`
workflow for the exact commit. It requires an authenticated `gh` CLI.

Pushing a `v*` tag starts `.github/workflows/release.yml`. After the reusable
test workflow succeeds, it:

- Publishes the Linux `x86_64-unknown-linux-gnu` binary and checksums as a GitHub release asset.
- Publishes the workspace packages to crates.io.
- Builds and deploys the versioned mdBook to GitHub Pages.

## Documentation Publication

Documentation changes committed to `master` are not published automatically.
They are published by the release workflow when a version tag is pushed.

For a post-release correction or a manual rebuild, run the standalone workflow
from the desired branch or tag:

```bash
gh workflow run docs.yml --ref master
gh workflow run docs.yml --ref v0.2.1
```

The workflow is manual-only. Running it from `master` intentionally publishes
the current `master` documentation, so use that mode only for a correction that
should be visible before the next product release.

## Maintenance Scripts

See [`scripts/README.md`](scripts/README.md) for the available maintenance
scripts and their responsibilities.
