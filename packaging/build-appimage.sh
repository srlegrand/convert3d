#!/usr/bin/env bash
# Builds dist/Convert3D-x86_64.AppImage around an already built convert3d.
#   packaging/build-appimage.sh path/to/convert3d
# Uses `appimagetool` from PATH, or the one given in $APPIMAGETOOL.
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN=${1:?usage: build-appimage.sh path/to/convert3d}
ARCH=${ARCH:-x86_64}
TOOL=${APPIMAGETOOL:-appimagetool}
APPDIR="$ROOT/build/Convert3D.AppDir"

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$ROOT/dist"
install -m755 "$BIN" "$APPDIR/usr/bin/convert3d"
install -m755 "$ROOT/packaging/appimage/AppRun" "$APPDIR/AppRun"
install -m644 "$ROOT/packaging/appimage/convert3d.desktop" "$ROOT/packaging/convert3d.svg" "$APPDIR/"
ln -s convert3d.svg "$APPDIR/.DirIcon"

# --appimage-extract-and-run: lets appimagetool itself run without FUSE (CI).
ARCH=$ARCH "$TOOL" --appimage-extract-and-run "$APPDIR" "$ROOT/dist/Convert3D-$ARCH.AppImage"
echo "Built $ROOT/dist/Convert3D-$ARCH.AppImage"
