#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != Darwin ]]; then
    echo "Run this script on macOS." >&2
    exit 1
fi

if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
"$root/platform/macos/build-app.sh"
open "$root/target/Nerd.app"
