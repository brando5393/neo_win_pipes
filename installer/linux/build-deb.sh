#!/usr/bin/env bash
# Assembles the pipes-xscreensaver .deb package. Run from the repo root:
#
#   installer/linux/build-deb.sh <version> <pipes-xscreensaver-bin> <pipes-settings-bin> <out.deb>
#
# Meant to run on a real Ubuntu host (this is what release.yml does) -
# nothing here is cross-compiled, and dpkg-deb itself is Debian/Ubuntu
# tooling that doesn't exist on Windows/macOS.
set -euo pipefail

VERSION="$1"
XSCREENSAVER_BIN="$2"
SETTINGS_BIN="$3"
OUT_DEB="$4"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

mkdir -p \
  "$STAGE/DEBIAN" \
  "$STAGE/usr/libexec/xscreensaver" \
  "$STAGE/usr/share/xscreensaver/config" \
  "$STAGE/usr/bin" \
  "$STAGE/usr/share/applications" \
  "$STAGE/usr/share/icons/hicolor/scalable/apps"

sed "s/__VERSION__/${VERSION}/" "$REPO_ROOT/installer/linux/control" > "$STAGE/DEBIAN/control"

install -m 755 "$XSCREENSAVER_BIN" "$STAGE/usr/libexec/xscreensaver/pipes-xscreensaver"
install -m 644 "$REPO_ROOT/installer/linux/xscreensaver-config/pipes-xscreensaver.xml" \
  "$STAGE/usr/share/xscreensaver/config/pipes-xscreensaver.xml"

install -m 755 "$SETTINGS_BIN" "$STAGE/usr/bin/pipes-settings"
install -m 644 "$REPO_ROOT/installer/linux/pipes-settings.desktop" \
  "$STAGE/usr/share/applications/pipes-settings.desktop"

for size_dir in "$REPO_ROOT"/assets/icon/linux/hicolor/*/apps; do
  size="$(basename "$(dirname "$size_dir")")"
  mkdir -p "$STAGE/usr/share/icons/hicolor/$size/apps"
  install -m 644 "$size_dir/neo_win_pipes.png" \
    "$STAGE/usr/share/icons/hicolor/$size/apps/neo_win_pipes.png"
done
install -m 644 "$REPO_ROOT/assets/icon/linux/scalable/apps/neo_win_pipes.svg" \
  "$STAGE/usr/share/icons/hicolor/scalable/apps/neo_win_pipes.svg"

# Refresh the icon cache and desktop-file database on install/remove so
# "Pipes Settings" actually shows up (with its real icon) in application
# menus without requiring a logout - gtk-update-icon-cache/
# update-desktop-database are themselves optional packages, hence `|| true`
# rather than a hard Depends on desktop-environment-specific tooling this
# package doesn't otherwise need.
cat > "$STAGE/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
gtk-update-icon-cache -f /usr/share/icons/hicolor >/dev/null 2>&1 || true
update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
EOF
cat > "$STAGE/DEBIAN/postrm" <<'EOF'
#!/bin/sh
set -e
gtk-update-icon-cache -f /usr/share/icons/hicolor >/dev/null 2>&1 || true
update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
EOF
chmod 755 "$STAGE/DEBIAN/postinst" "$STAGE/DEBIAN/postrm"

mkdir -p "$(dirname "$OUT_DEB")"
dpkg-deb --build --root-owner-group "$STAGE" "$OUT_DEB"
echo "built $OUT_DEB"
