#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
git -C "$repo_root" config core.hooksPath .githooks
printf 'Git hooks enabled for %s\n' "$repo_root"
