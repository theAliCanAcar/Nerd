#!/usr/bin/env bash
set -euo pipefail

# Pack nerd.exe into an NSIS installer by running makensis under Wine.
# Usage: pack-installer.sh <version> <nerd.exe> <output.exe>

version="${1:?version}"
app="${2:?path to nerd.exe}"
dest="${3:?output installer}"

die() { printf 'pack-installer: %s\n' "$*" >&2; exit 1; }
command -v wine >/dev/null 2>&1 || die "Wine is required to build the Windows installer on macOS."
[[ -f "$app" ]] || die "Windows application was not found: $app"

makensis="${NSIS_MAKENSIS:-$HOME/Library/Caches/electron-builder/nsis/nsis-3.0.4.1/Bin/makensis.exe}"
[[ -f "$makensis" ]] || die "NSIS was not found at $makensis. Set NSIS_MAKENSIS to makensis.exe."

stage="$(mktemp -d /tmp/nerd-win-release.XXXXXX)"
prefix="$(mktemp -d /tmp/nerd-wine.XXXXXX)"
cleanup() {
    wineserver -k >/dev/null 2>&1 || true
    rm -rf "$stage" "$prefix"
}
trap cleanup EXIT

cp "$app" "$stage/nerd.exe"
cat >"$stage/nerd.nsi" <<EOF
Unicode true
Name "Nerd"
OutFile "Nerd-${version}-x64.exe"
InstallDir "\$LOCALAPPDATA\\Nerd"
RequestExecutionLevel user
SetCompressor /SOLID lzma

Page directory
Page instfiles

Section "Install"
  SetOutPath \$INSTDIR
  File "nerd.exe"
  CreateShortcut "\$SMPROGRAMS\\Nerd.lnk" "\$INSTDIR\\nerd.exe"
  CreateShortcut "\$DESKTOP\\Nerd.lnk" "\$INSTDIR\\nerd.exe"
  WriteUninstaller "\$INSTDIR\\Uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "\$INSTDIR\\nerd.exe"
  Delete "\$INSTDIR\\Uninstall.exe"
  Delete "\$SMPROGRAMS\\Nerd.lnk"
  Delete "\$DESKTOP\\Nerd.lnk"
  RMDir "\$INSTDIR"
SectionEnd
EOF

export WINEPREFIX="$prefix"
export WINEDEBUG="${WINEDEBUG:--all}"
export WINEDLLOVERRIDES="${WINEDLLOVERRIDES:-mscoree,mshtml=}"
export MVK_CONFIG_LOG_LEVEL="${MVK_CONFIG_LOG_LEVEL:-0}"
echo "pack-installer: preparing a temporary Wine prefix"
wineboot --init
wineserver -w || true

echo "pack-installer: running NSIS under Wine"
(
    cd "$stage"
    wine "$makensis" nerd.nsi
)
[[ -f "$stage/Nerd-${version}-x64.exe" ]] || die "NSIS did not write the installer."
mkdir -p "$(dirname "$dest")"
cp -f "$stage/Nerd-${version}-x64.exe" "$dest"
echo "pack-installer: $dest"
