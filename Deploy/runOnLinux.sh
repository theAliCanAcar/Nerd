#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != Linux ]]; then
    echo "Run this script on Linux." >&2
    exit 1
fi

if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi
if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo was not found. Install Rust from https://rustup.rs/ and open a new terminal." >&2
    exit 1
fi
if [[ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]]; then
    echo "No desktop display was found. Run this from a graphical session." >&2
    exit 1
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
cargo build --locked --package nerd --target-dir target

bin="$root/target/debug/nerd"
if [[ ! -x "$bin" ]]; then
    echo "The build did not produce $bin" >&2
    exit 1
fi

nohup "$bin" >/dev/null 2>&1 &
disown || true
echo "Opened Nerd."
