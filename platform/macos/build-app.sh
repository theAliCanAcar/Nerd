#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != Darwin ]]; then
    echo "Run this script on macOS." >&2
    exit 1
fi

cd "$(dirname "$0")/../.."
if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi
if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo was not found. Install Rust from https://rustup.rs/ and open a new terminal." >&2
    exit 1
fi
cargo build --locked --package nerd --target-dir target

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
iconset="$scratch/Nerd.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" assets/icons/app-linux.png \
        --out "$iconset/icon_${size}x${size}.png" >/dev/null
    doubled="$((size * 2))"
    sips -z "$doubled" "$doubled" assets/icons/app-linux.png \
        --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$scratch/Nerd.icns"

app="target/Nerd.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
install -m 755 target/debug/nerd "$app/Contents/MacOS/nerd"
install -m 644 platform/macos/Info.plist "$app/Contents/Info.plist"
install -m 644 "$scratch/Nerd.icns" "$app/Contents/Resources/Nerd.icns"
echo "Open $PWD/$app"
