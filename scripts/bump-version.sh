#!/usr/bin/env sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

usage() {
    printf 'usage: %s [--dry-run] <major|minor|patch|X.Y.Z>\n' "$0" >&2
}

dry_run=0
if [ "${1:-}" = "--dry-run" ]; then
    dry_run=1
    shift
fi

if [ "$#" -ne 1 ]; then
    usage
    exit 2
fi

requested=$1
case "$requested" in
    v*) requested=${requested#v} ;;
esac

package_version() {
    package_id=$(cargo pkgid --package "$1")
    version=${package_id##*@}
    if [ "$version" = "$package_id" ]; then
        printf 'could not determine version for %s\n' "$1" >&2
        exit 1
    fi
    printf '%s' "$version"
}

current_version=$(package_version park-cli)
macro_version=$(package_version park-e2e-macros)
if [ "$current_version" != "$macro_version" ]; then
    printf 'version mismatch: park-cli=%s, park-e2e-macros=%s\n' \
        "$current_version" "$macro_version" >&2
    exit 1
fi

new_version=$(
    printf '%s\n' "$current_version" |
        awk -F. -v bump="$requested" '
            BEGIN {
                if (bump ~ /^[0-9]+\.[0-9]+\.[0-9]+$/) {
                    print bump
                    exit
                }
                if (bump != "major" && bump != "minor" && bump != "patch") {
                    exit 1
                }
            }
            NF != 3 || $1 !~ /^[0-9]+$/ || $2 !~ /^[0-9]+$/ || $3 !~ /^[0-9]+$/ {
                exit 1
            }
            {
                major = $1 + 0
                minor = $2 + 0
                patch = $3 + 0
                if (bump == "major") {
                    major++
                    minor = 0
                    patch = 0
                } else if (bump == "minor") {
                    minor++
                    patch = 0
                } else if (bump == "patch") {
                    patch++
                }
                printf "%d.%d.%d\n", major, minor, patch
            }
        '
) || {
    printf 'invalid version or bump: %s\n' "$1" >&2
    exit 2
}

if [ "$current_version" = "$new_version" ]; then
    printf 'version is already %s\n' "$current_version"
    exit 0
fi

if [ "$dry_run" -eq 1 ]; then
    printf '%s -> %s\n' "$current_version" "$new_version"
    exit 0
fi

update_manifest() {
    manifest=$1
    temporary="$manifest.tmp.$$"
    awk -v new_version="$new_version" '
        /^\[package\][[:space:]]*$/ {
            in_package = 1
            package_seen = 1
            print
            next
        }
        in_package && /^\[/ {
            in_package = 0
        }
        in_package && !version_updated && /^[[:space:]]*version[[:space:]]*=/ {
            sub(/"[^"]*"/, "\"" new_version "\"")
            version_updated = 1
        }
        { print }
        END {
            if (!package_seen || !version_updated) {
                exit 1
            }
        }
    ' "$manifest" > "$temporary" || {
        rm -f "$temporary"
        printf 'could not update %s\n' "$manifest" >&2
        exit 1
    }
    mv "$temporary" "$manifest"
}

update_development_version() {
    document=docs/src/development.md
    temporary="$document.tmp.$$"
    awk -v new_version="$new_version" '
        /current package version is `[0-9]+\.[0-9]+\.[0-9]+`/ && !version_updated {
            sub(/[0-9]+\.[0-9]+\.[0-9]+/, new_version)
            version_updated = 1
        }
        { print }
        END {
            if (!version_updated) {
                exit 1
            }
        }
    ' "$document" > "$temporary" || {
        rm -f "$temporary"
        printf 'could not update %s\n' "$document" >&2
        exit 1
    }
    mv "$temporary" "$document"
}

update_manifest Cargo.toml
update_manifest e2e-macros/Cargo.toml
update_development_version

# Refresh the local package versions in Cargo.lock without changing dependency versions.
cargo check --workspace --quiet
scripts/check-version.sh

printf 'bumped version: %s -> %s\n' "$current_version" "$new_version"
