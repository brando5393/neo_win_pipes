#!/usr/bin/env bash
# Builds a Pipes Settings AppImage - a portable alternative to the .deb
# for non-Debian distros. Only pipes-settings: the xscreensaver hack
# can't be an AppImage at all (see AppDir/AppRun for why), so
# installer/linux/build-deb.sh is still the only path to actually
# installing the screensaver itself.
#
#   installer/linux/build-appimage.sh <version> <pipes-settings-bin> <out.AppImage>
set -euo pipefail

VERSION="$1"
SETTINGS_BIN="$2"
OUT_APPIMAGE="$3"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

APPDIR="$STAGE/AppDir"
cp -r "$REPO_ROOT/installer/linux/AppDir" "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
install -m 755 "$SETTINGS_BIN" "$APPDIR/usr/bin/pipes-settings"
install -m 644 "$REPO_ROOT/assets/icon/linux/hicolor/256x256/apps/neo_win_pipes.png" \
  "$APPDIR/neo_win_pipes.png"

APPIMAGETOOL="$STAGE/appimagetool-x86_64.AppImage"
curl -fsSL -o "$APPIMAGETOOL" \
  https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
chmod +x "$APPIMAGETOOL"

mkdir -p "$(dirname "$OUT_APPIMAGE")"
# CI runners typically have no FUSE, which appimagetool (and the AppImage
# it produces) normally need to mount themselves - the extract-and-run
# fallback avoids that requirement entirely, at the cost of a slower
# startup for whoever runs the resulting AppImage on a FUSE-less system
# too (same env var works for the *output* AppImage, not just this
# build step).
APPIMAGE_EXTRACT_AND_RUN=1 VERSION="$VERSION" "$APPIMAGETOOL" "$APPDIR" "$OUT_APPIMAGE"
echo "built $OUT_APPIMAGE"
