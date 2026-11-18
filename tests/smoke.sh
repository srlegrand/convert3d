#!/usr/bin/env bash
# End-to-end check with a real Blender: convert a tiny model and build a preview page.
#   tests/smoke.sh path/to/convert3d
set -eu
BIN=$(readlink -f "${1:?usage: smoke.sh path/to/convert3d}")
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

"$BIN" doctor
printf 'v 0 0 0\nv 1 0 0\nv 0 1 0\nv 0 0 1\nf 1 2 3\nf 1 2 4\nf 1 3 4\nf 2 3 4\n' > tet.obj

"$BIN" convert --overwrite --to glb tet.obj
[[ $(head -c4 tet.glb) == glTF ]] || { echo "tet.glb is not a glTF binary" >&2; exit 1; }

"$BIN" convert --overwrite --to fbx tet.glb
test -s tet.fbx

page=$("$BIN" view --no-open tet.obj)
grep -q 'tet.obj' "$page" || { echo "the preview page does not name the model" >&2; exit 1; }
echo "smoke test passed"
