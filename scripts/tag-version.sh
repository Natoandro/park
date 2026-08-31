#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

usage() {
    printf 'usage: %s [--push] [--dry-run]\n' "$0"
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

if [ -n "$(git status --porcelain)" ]; then
    printf 'working tree must be clean before creating %s\n' "$tag" >&2
    exit 1
fi

git tag --annotate "$tag" --message "Release $tag"
printf 'created tag %s at %s\n' "$tag" "$(git rev-parse --short HEAD)"
if [ "$push_tag" -eq 1 ]; then
    git push origin "$tag"
else
    printf 'push it with: git push origin %s\n' "$tag"
fi
