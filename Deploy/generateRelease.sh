#!/usr/bin/env bash
set -euo pipefail

# Build the macOS disk image, the Windows application, and the Linux AppImage.
# Run this on a Mac with Rust, Docker, and enough free disk for one platform at a time.
#
#   ./Deploy/generateRelease.sh

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi

die() { printf 'generateRelease: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "Missing required command: $1"; }

[[ "$(uname -s)" == Darwin ]] || die "Run this script on macOS. It cross-builds Windows and Linux from there."
need cargo
need docker
need hdiutil
need wine
docker info >/dev/null 2>&1 || die "Docker is installed but the daemon is not running."

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
[[ -n "$version" ]] || die "Could not read the version from Cargo.toml."
arch="$(uname -m)"

echo "generateRelease: Nerd ${version}"
echo "generateRelease: removing the previous compile output so the three packages fit on disk"
cargo clean

macos_dir="$root/release/macos"
windows_dir="$root/release/windows"
linux_dir="$root/release/linux"
mkdir -p "$macos_dir" "$windows_dir" "$linux_dir"

echo "generateRelease: macOS"
"$root/platform/macos/build-app.sh" --release
codesign --force --deep --sign - "$root/target/Nerd.app"
dmg="$macos_dir/Nerd-${version}-${arch}.dmg"
rm -f "$dmg"
hdiutil create -volname "Nerd" -srcfolder "$root/target/Nerd.app" -ov -format UDZO "$dmg"
echo "generateRelease: $dmg"

echo "generateRelease: removing the macOS compile output"
cargo clean

echo "generateRelease: Windows"
"$root/platform/windows/build-exe.sh" "$version"
echo "generateRelease: removing the Windows compile output"
cargo clean

echo "generateRelease: Linux"
"$root/platform/linux/build-appimage.sh" "$version"

echo
echo "generateRelease: Nerd ${version} is ready"
echo "  $macos_dir/Nerd-${version}-${arch}.dmg"
echo "  $windows_dir/Nerd-${version}-x64.exe"
echo "  $linux_dir/Nerd-${version}-x64.AppImage"
