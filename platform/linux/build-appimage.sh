#!/usr/bin/env bash
set -euo pipefail

# Build the Linux AppImage inside Docker and copy it to release/linux.

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

die() { printf 'build-appimage: %s\n' "$*" >&2; exit 1; }
command -v docker >/dev/null 2>&1 || die "Docker is required to build the Linux AppImage on macOS."
docker info >/dev/null 2>&1 || die "Docker is installed but the daemon is not running."

version="${1:-}"
if [[ -z "$version" ]]; then
    version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
fi
[[ -n "$version" ]] || die "Could not read the version from Cargo.toml."

out="$root/release/linux/Nerd-${version}-x64.AppImage"
mkdir -p "$(dirname "$out")"

image="node:22-bookworm"
if docker image inspect rust:1.92-bookworm >/dev/null 2>&1; then
    image="rust:1.92-bookworm"
fi

echo "build-appimage: compiling in ${image}"
docker run --rm \
    --platform linux/amd64 \
    -e NERD_VERSION="$version" \
    -e CARGO_HOME=/cargo \
    -e RUSTUP_HOME=/rustup \
    -v "$root":/src \
    -v "$HOME/.cargo/registry":/cargo/registry \
    -v "$HOME/.cargo/git":/cargo/git \
    -w /src \
    "$image" \
    bash /src/platform/linux/compile-appimage.sh

[[ -f "$out" ]] || die "The container finished without writing $out"
echo "build-appimage: $out"
