#!/bin/sh
set -eu

root=${PARK_E2E_ROOT:-/tmp/park-e2e}
case "$root" in
    /*) ;;
    *)
        printf '%s\n' 'PARK_E2E_ROOT must be an absolute dedicated directory' >&2
        exit 2
        ;;
esac

mkdir -p "$root"
chmod 700 "$root"

export PARK_E2E_ROOT="$root"

runner=${PARK_E2E_RUNNER:-/usr/local/bin/park-e2e}

exec "$runner" "$@"
