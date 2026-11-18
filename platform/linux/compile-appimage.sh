#!/usr/bin/env bash
set -euo pipefail

# Runs inside the Linux container. Writes /src/release/linux/Nerd-<version>-x64.AppImage.

cd /src
version="${NERD_VERSION:?}"
export DEBIAN_FRONTEND=noninteractive

apt-get update
apt-get install -y --no-install-recommends \
    build-essential ca-certificates curl file pkg-config \
    libegl1-mesa-dev libwayland-dev libx11-dev libxcb1-dev \
    libxcursor-dev libxi-dev libxkbcommon-dev libxrandr-dev libvulkan-dev

if ! command -v cargo >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain 1.92.0 --profile minimal --component rustfmt,clippy
fi
# shellcheck disable=SC1091
source "${CARGO_HOME:-$HOME/.cargo}/env"

cargo build --release --locked --package nerd --target-dir /tmp/nerd-target -j 1
binary=/tmp/nerd-target/release/nerd
[[ -x "$binary" ]] || { echo "The Linux build did not produce $binary" >&2; exit 1; }

appdir=/tmp/Nerd.AppDir
rm -rf "$appdir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/lib" "$appdir/usr/share/icons/hicolor/256x256/apps"
install -m 755 "$binary" "$appdir/usr/bin/nerd"
install -m 644 /src/assets/icons/app-linux.png "$appdir/nerd.png"
install -m 644 /src/assets/icons/app-linux.png "$appdir/usr/share/icons/hicolor/256x256/apps/nerd.png"

cat >"$appdir/nerd.desktop" <<'EOF'
[Desktop Entry]
Name=Nerd
Exec=nerd
Icon=nerd
Type=Application
Categories=System;TerminalEmulator;
EOF

cat >"$appdir/AppRun" <<'EOF'
#!/bin/sh
here=$(dirname "$(readlink -f "$0")")
export LD_LIBRARY_PATH="$here/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$here/usr/bin/nerd" "$@"
EOF
chmod 755 "$appdir/AppRun"

ldd "$binary" | awk '/=>/ { print $3 }' | while read -r lib; do
    [[ -n "$lib" && -f "$lib" ]] || continue
    base=$(basename "$lib")
    case "$base" in
        libc.so*|libm.so*|libdl.so*|libpthread.so*|librt.so*|libresolv.so*|ld-linux*)
            continue
            ;;
    esac
    cp -a "$lib" "$appdir/usr/lib/"
done

curl -fsSL "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" \
    -o /tmp/appimagetool
chmod 755 /tmp/appimagetool
APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 /tmp/appimagetool "$appdir" "/src/release/linux/Nerd-${version}-x64.AppImage"
chmod 755 "/src/release/linux/Nerd-${version}-x64.AppImage"
