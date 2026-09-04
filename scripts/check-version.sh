#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

if [ "$#" -gt 1 ]; then
    printf 'usage: %s [tag]\n' "$0" >&2
    exit 2
fi

package_version() {
    package_id=$(cargo pkgid --package "$1")
    version=${package_id##*@}
    if [ "$version" = "$package_id" ]; then
        printf 'could not determine version for %s\n' "$1" >&2
        exit 1
    fi
    printf '%s' "$version"
}

root_version=$(package_version park-cli)
macro_version=$(package_version park-e2e-macros)
e2e_version=$(package_version park-e2e)
documented_version=$(awk '
    /current package version is `[0-9]+\.[0-9]+\.[0-9]+`/ {
        if (match($0, /[0-9]+\.[0-9]+\.[0-9]+/)) {
            print substr($0, RSTART, RLENGTH)
            version_found = 1
            exit
        }
    }
    END {
        if (!version_found) {
            exit 1
        }
    }
' docs/src/development.md) || {
    printf 'could not determine version documented in docs/src/development.md\n' >&2
    exit 1
}

if [ "$root_version" != "$macro_version" ] || [ "$root_version" != "$e2e_version" ]; then
    printf 'version mismatch: park-cli=%s, park-e2e-macros=%s, park-e2e=%s\n' \
        "$root_version" "$macro_version" "$e2e_version" >&2
    exit 1
fi

if [ "$root_version" != "$documented_version" ]; then
    printf 'version mismatch: park-cli=%s, docs/src/development.md=%s\n' \
        "$root_version" "$documented_version" >&2
    exit 1
fi

if [ "$#" -eq 1 ]; then
    tag=$1
    case "$tag" in
        v*) expected_version=${tag#v} ;;
        *) expected_version=$tag ;;
    esac

    if [ "$root_version" != "$expected_version" ]; then
        printf 'tag version mismatch: tag=%s, package=%s\n' \
            "$tag" "$root_version" >&2
        exit 1
    fi
fi

if [ "$#" -eq 1 ]; then
    printf 'version check passed: %s (%s)\n' "$root_version" "$1"
else
    printf 'version check passed: %s\n' "$root_version"
fi
