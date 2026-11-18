#!/bin/sh
# Installs convert3d for the current user on Linux or macOS (no sudo):
#
#   curl -fsSL https://raw.githubusercontent.com/srlegrand/convert3d/main/install.sh | sh
#
# It downloads the latest release for this computer and runs `convert3d install`,
# which keeps its own copy and adds the right-click menus.
# To remove everything again, add:  -s -- --uninstall
set -eu

REPO="https://github.com/srlegrand/convert3d"

case "$(uname -s)" in
    Linux)
        case "$(uname -m)" in
            x86_64 | amd64) asset=convert3d-linux-x86_64 ;;
            aarch64 | arm64) asset=convert3d-linux-arm64 ;;
            *) echo "No download for $(uname -m). Build from source: $REPO#from-source" >&2; exit 1 ;;
        esac
        installed="${XDG_DATA_HOME:-$HOME/.local/share}/convert3d/bin/convert3d" ;;
    Darwin)
        asset=convert3d-macos-universal
        installed="$HOME/Library/Application Support/convert3d/bin/convert3d" ;;
    *)  echo "This installer is for Linux and macOS. On Windows use Convert3D-Setup.exe." >&2; exit 1 ;;
esac

if [ "${1-}" = "--uninstall" ]; then
    [ -x "$installed" ] || { echo "convert3d is not installed."; exit 0; }
    exec "$installed" uninstall
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
echo "Downloading $asset..."
curl -fsSL "$REPO/releases/latest/download/$asset.tar.gz" | tar xz -C "$tmp"
"$tmp/$asset/convert3d" install
