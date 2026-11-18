#!/usr/bin/env bash
set -euo pipefail

case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*|Windows_NT) ;;
    *)
        echo "Run this script on Windows, from Git Bash." >&2
        exit 1
        ;;
esac

if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi
if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo was not found. Install Rust from https://rustup.rs/ and open a new terminal." >&2
    exit 1
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
cargo build --locked --package nerd --target-dir target

exe="$root/target/debug/nerd.exe"
if [[ ! -f "$exe" ]]; then
    echo "The build did not produce $exe" >&2
    exit 1
fi

win_path="$exe"
if command -v cygpath >/dev/null 2>&1; then
    win_path="$(cygpath -w "$exe")"
fi
cmd.exe //c start "" "$win_path"
echo "Opened Nerd."
