#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)

profile=release
build=1
runner_args=()

usage() {
    cat <<'EOF'
Usage: scripts/e2e.sh [options]

Build and run the standalone Docker e2e runner.

Options:
  --release          Build and run the release artifact (default)
  --debug            Build and run the debug artifact
  --no-build         Reuse the existing Docker image
  --list             List registered scenarios
  --filter TEXT      Run scenarios matching story ID or text
  --tag TAG          Run scenarios with TAG
  -h, --help         Show this help
  --                 Pass remaining arguments to park-e2e
EOF
}

require_value() {
    if [[ $# -lt 2 || -z "$2" ]]; then
        printf 'missing value for %s\n' "$1" >&2
        exit 2
    fi
}

while (($# > 0)); do
    case "$1" in
        --release)
            profile=release
            shift
            ;;
        --debug)
            profile=debug
            shift
            ;;
        --no-build)
            build=0
            shift
            ;;
        --list)
            runner_args+=(--list)
            shift
            ;;
        --filter)
            require_value "$@"
            runner_args+=(--filter "$2")
            shift 2
            ;;
        --tag)
            require_value "$@"
            runner_args+=(--tag "$2")
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --)
            shift
            runner_args+=("$@")
            break
            ;;
        *)
            printf 'unknown option: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if ! command -v docker >/dev/null 2>&1; then
    printf '%s\n' 'docker is required to run e2e tests' >&2
    exit 1
fi

image=${PARK_E2E_IMAGE:-park-e2e:$profile}
if ((build)); then
    docker build \
        --build-arg "BUILD_PROFILE=$profile" \
        --file "$repo_root/docker/e2e/Dockerfile" \
        --tag "$image" \
        "$repo_root"
fi

exec docker run --rm --init \
    --network none \
    --cap-drop=ALL \
    --security-opt=no-new-privileges \
    --pids-limit=256 \
    "$image" \
    "${runner_args[@]}"
