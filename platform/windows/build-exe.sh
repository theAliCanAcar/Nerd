#!/usr/bin/env bash
set -euo pipefail

# Cross-compile the Windows application from macOS, then pack an NSIS installer
# with Wine. Wine does not change the user's ~/.wine prefix.
# The result is release/windows/Nerd-<version>-x64.exe

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi

die() { printf 'build-exe: %s\n' "$*" >&2; exit 1; }
[[ "$(uname -s)" == Darwin ]] || die "Run this script on macOS."
command -v cargo >/dev/null 2>&1 || die "cargo was not found."

version="${1:-}"
if [[ -z "$version" ]]; then
    version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
fi
[[ -n "$version" ]] || die "Could not read the version from Cargo.toml."

tools="$root/target/tools"
mkdir -p "$tools"
xwin="$tools/cargo-xwin"
if [[ ! -x "$xwin" ]]; then
    echo "build-exe: downloading cargo-xwin"
    archive="$(mktemp -d)"
    curl -fsSL "https://github.com/rust-cross/cargo-xwin/releases/download/v0.23.1/cargo-xwin-v0.23.1.universal2-apple-darwin.tar.gz" \
        -o "$archive/cargo-xwin.tar.gz"
    tar -xzf "$archive/cargo-xwin.tar.gz" -C "$archive"
    found="$(find "$archive" -type f -name cargo-xwin | head -1)"
    [[ -n "$found" ]] || die "The cargo-xwin archive did not contain the binary."
    install -m 755 "$found" "$xwin"
    rm -rf "$archive"
fi

rustup target add x86_64-pc-windows-msvc
"$xwin" build --release --locked --target x86_64-pc-windows-msvc --package nerd --target-dir target

out="$root/release/windows/Nerd-${version}-x64.exe"
mkdir -p "$(dirname "$out")"
"$root/platform/windows/pack-installer.sh" "$version" target/x86_64-pc-windows-msvc/release/nerd.exe "$out"
echo "build-exe: $out"
