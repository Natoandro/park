#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

usage() {
    printf 'usage: %s [--push] [--dry-run]\n' "$0"
}

fail() {
    printf '%s\n' "$1" >&2
    exit 1
}

push_tag=0
dry_run=0
while [ "$#" -gt 0 ]; do
    case "$1" in
        --push) push_tag=1 ;;
        --dry-run) dry_run=1 ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            usage >&2
            exit 2
            ;;
    esac
    shift
done

if [ "$(git branch --show-current)" != "master" ]; then
    fail 'release tags must be created from the master branch'
fi

if [ -n "$(git status --porcelain)" ]; then
    fail 'working tree must be clean before creating a release tag'
fi

if ! git fetch --quiet origin master; then
    fail 'could not fetch origin/master before creating a release tag'
fi

local_head=$(git rev-parse HEAD)
remote_head=$(git rev-parse --verify refs/remotes/origin/master) ||
    fail 'origin/master is not available'
if [ "$local_head" != "$remote_head" ]; then
    fail 'local master is not synchronized with origin/master'
fi

if ! ci_result=$(gh run list \
    --workflow test.yml \
    --branch master \
    --commit "$remote_head" \
    --limit 1 \
    --json status,conclusion,databaseId \
    --jq 'if length == 0 then "missing" else .[0] | [.status, (.conclusion // "none"), (.databaseId | tostring)] | join(" ") end'
); then
    fail 'could not query the GitHub Actions Test workflow'
fi

case "$ci_result" in
    'completed success '*)
        printf 'CI passed for master (%s)\n' "${ci_result##* }"
        ;;
    missing)
        fail "no GitHub Actions Test workflow run found for master commit $remote_head"
        ;;
    *)
        fail "latest GitHub Actions Test workflow for master is not successful: $ci_result"
        ;;
esac

scripts/check-version.sh

package_id=$(cargo pkgid --package park-cli)
version=${package_id##*@}
tag="v$version"

if git rev-parse --verify --quiet "refs/tags/$tag" >/dev/null; then
    printf 'tag already exists: %s\n' "$tag" >&2
    exit 1
fi

if [ "$dry_run" -eq 1 ]; then
    printf 'would create annotated tag %s at %s\n' "$tag" "$(git rev-parse --short HEAD)"
    if [ "$push_tag" -eq 1 ]; then
        printf 'would push with: git push origin %s\n' "$tag"
    fi
    exit 0
fi

git tag --annotate "$tag" --message "Release $tag"
printf 'created tag %s at %s\n' "$tag" "$(git rev-parse --short HEAD)"
if [ "$push_tag" -eq 1 ]; then
    git push origin "$tag"
else
    printf 'push it with: git push origin %s\n' "$tag"
fi
