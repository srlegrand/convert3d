# convert3d

Right-click any 3D file — or a **zip** with a model and its textures — and convert it to another 3D format.

```
chair.fbx  ──right-click──▶  Convert 3D to ▸ glTF Binary (.glb)
                                            USDZ (.usdz)
                                            Wavefront OBJ (.obj)
                                            Alembic (.abc)  …
```

Works on **Windows** (Explorer), **macOS** (Finder Quick Action) and **Linux** (KDE Dolphin, GNOME Files).

## How it works

convert3d is a small Rust program that handles the menu, zips and file handling. The actual 3D work
is done by tools you already have (or can install for free):

| Tool | Needed for |
|---|---|
| [Blender](https://www.blender.org/download/) 4.2+ (**required**) | fbx, glb, gltf, obj, abc, usd/usda/usdc/usdz, blend, stl, ply |
| [Assimp](https://github.com/assimp/assimp) (optional) | dae, 3ds, x, 3mf, x3d, and ~35 more input formats (lwo, ase, ms3d, md2/md3/md5, ifc, ac, b3d, off, smd, …) |
| Maya / `mayapy` (optional) | .ma, .mb |
| 3ds Max / `3dsmaxbatch` (optional, Windows only) | .max |

Any format can be converted to any other: convert3d finds the cheapest chain of tools, e.g.
`.dae → (assimp) → .gltf → (blender) → .usdz`, or `.max → (3ds Max) → .fbx → (blender) → .glb`.

### Zips

Right-click a zip containing e.g. `model.obj` + `model.mtl` + `textures/*.png` (or an FBX and its
textures). convert3d unpacks it, picks the model (FBX > glTF > USD > Alembic > OBJ > …, top-most
folder first) and converts it.

Texture paths that are broken — very common in FBX files that point to `C:\Users\artist\...` — are
repaired by searching the zip (or the model's folder) for a file with the same name.

### Where results go

- Single-file results (glb, usdz, fbx with embedded textures, blend with packed textures, abc, stl, ply) are
  written next to the source: `chair.fbx` → `chair.glb`.
- Results with separate texture files (obj, gltf, usda, dae, …) go into a folder `chair_obj/`, or into
  `chair_obj.zip` when the input was a zip.
- Existing files are never overwritten; you get `chair (2).glb` instead (use `--overwrite` on the command line to replace).

## Install

1. Install [Blender](https://www.blender.org/download/). Optionally install Assimp:
   - Linux: `sudo apt install assimp-utils` (or your distro's `assimp` package)
   - macOS: `brew install assimp`
   - Windows: build it or grab `assimp.exe` from a release, then `convert3d set-tool assimp C:\path\to\assimp.exe`
2. Download convert3d for your OS from [Releases](../../releases) and unpack it.
3. In a terminal, run:

   ```sh
   ./convert3d install        # Windows: .\convert3d.exe install
   ```

   This copies convert3d to your user folder and adds the menu. No admin rights needed. Run it again after
   installing Assimp/Maya/Max so their formats appear in the menu.

- **Windows 11:** the menu is under *Show more options* (or Shift+right-click).
- **macOS:** the menu is under *Quick Actions → Convert 3D…* and asks which format you want. If Gatekeeper
  blocks the downloaded binary, run `xattr -d com.apple.quarantine convert3d` first.
- **Linux:** Dolphin shows *Convert 3D to ▸*; GNOME Files shows it under *Scripts*. convert3d also registers MIME
  types for 3D extensions your system doesn't know (fbx, usd, abc, ply, …) so the menu shows up for them.

Uninstall with `convert3d uninstall`.

## Command line

```sh
convert3d convert --to glb chair.fbx table.obj props.zip
convert3d convert --to usdz --out-dir ./usd *.fbx
convert3d formats            # what can be read/written with the tools you have
convert3d doctor             # which tools were found
convert3d set-tool blender /path/to/blender
```

Tools are found on `PATH`, in their standard install locations, via `CONVERT3D_BLENDER` / `CONVERT3D_ASSIMP` /
`CONVERT3D_MAYAPY` / `CONVERT3D_3DSMAX`, or with `set-tool`. Logs of each run are kept in your cache folder
(`convert3d doctor` prints where); when a right-click conversion fails, the error dialog shows the end of the log.

## Limitations

- **.max** files can only be read by 3ds Max itself, so they need 3ds Max installed (Windows).
- **.ma/.mb** need Maya. Maya ASCII is text, but it's a full scene language; there's no reliable way to
  read it without Maya.
- Alembic, STL and PLY don't store materials, so converting *to* them drops textures.
- Each format keeps what it can: rigs and animation survive FBX ↔ glTF ↔ USD well, less so via OBJ or Assimp formats.

## Building

```sh
cargo build --release
cargo test
```

Pushing a `v*` tag builds release binaries for Linux (x86_64, arm64), Windows (x86_64) and macOS (arm64, x86_64).

## License

MIT
