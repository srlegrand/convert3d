# convert3d

[![Build](https://github.com/srlegrand/convert3d/actions/workflows/build.yml/badge.svg)](https://github.com/srlegrand/convert3d/actions/workflows/build.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Right-click a 3D file, or a zip with a model and its textures, and convert it to another 3D format,
or preview it.

Works in Windows Explorer, macOS Finder, KDE Dolphin and GNOME Files. Free, open source, and everything
happens on your own computer.

**Website, downloads and docs: <https://srlegrand.github.io/convert3d/>**

## Formats

| Format | Needs |
|---|---|
| fbx, glb, gltf, obj, abc, usd, usda, usdc, usdz, blend, stl, ply | Blender 4.2+ |
| dae, 3ds, x, 3mf, x3d (and ~35 more read-only: lwo, ase, ms3d, md2, md3, ifc, off, smd…) | Assimp |
| ma, mb | Maya or Aztek |
| max | 3ds Max (Windows only) |

Blender is required. The rest are optional. Any format can be converted to any other if the tools for both are installed.

## Install

| System | Download | Then |
| --- | --- | --- |
| **Windows** | [Convert3D-Setup.exe](https://github.com/srlegrand/convert3d/releases/latest/download/Convert3D-Setup.exe) | Run it. No admin rights needed. It offers to install Blender. |
| **macOS** | [Convert3D-macOS.pkg](https://github.com/srlegrand/convert3d/releases/latest/download/Convert3D-macOS.pkg) | Right-click the file and choose **Open**. |
| **Linux** | [Convert3D-x86_64.AppImage](https://github.com/srlegrand/convert3d/releases/latest/download/Convert3D-x86_64.AppImage) | Make it executable, double-click it, then reopen your file manager. |

Or from a terminal on macOS and Linux (x86_64 and ARM), for your user only:

```sh
curl -fsSL https://raw.githubusercontent.com/srlegrand/convert3d/main/install.sh | sh
```

convert3d needs [Blender](https://www.blender.org/download/) 4.2 or newer:
`winget install BlenderFoundation.Blender`, `brew install --cask blender`, or `sudo snap install blender --classic`.
Assimp is optional: `brew install assimp`, `sudo apt install assimp-utils` or `sudo dnf install assimp`.

The installers are not code-signed, so Windows SmartScreen ("More info" > "Run anyway")
and macOS Gatekeeper (right-click > Open, or System Settings > Privacy & Security >
"Open Anyway") ask for confirmation the first time.

## Use

Right-click a 3D file, or a zip:

- **Windows** - **Convert 3D to** or **Preview 3D** (on Windows 11 under **Show more options**, or Shift + right-click)
- **macOS** - **Quick Actions** → **Convert 3D…** and pick a format, or **Preview 3D**
- **Linux** - **Convert 3D to** or **Preview 3D** in Dolphin, under **Scripts** in GNOME Files

<img width="517" height="495" alt="2026-11-15_16-00-14" src="https://github.com/user-attachments/assets/93d69689-1e5c-4e6a-991a-34f846392939" />

<img width="978" height="845" alt="2026-11-15_16-02-05" src="https://github.com/user-attachments/assets/4ebe2121-2d78-458c-9dde-2e4b425bc7aa" />

**Options**, at the bottom of the **Convert 3D to** menu (on macOS the **Convert 3D Options** Quick Action),
chooses where converted files are saved, what happens when the name is taken, and which application
reads or writes each format.

On macOS, if the Quick Actions don't appear: System Settings → General → Login Items & Extensions → Finder,
and turn on **Convert 3D…** and **Preview 3D**.

Flatpak Blender isn't detected automatically. Point convert3d at it: `convert3d set-tool blender /path/to/blender`.

### From source (any platform)

Needs [Rust](https://rustup.rs).

```sh
cargo install --git https://github.com/srlegrand/convert3d
convert3d install
```

## After installing new tools

If you install Assimp, Maya or 3ds Max later, run `convert3d install` again to add their formats to the menu.

The command is `convert3d` on Linux (linked in `~/.local/bin`) and after the macOS package,
`%LOCALAPPDATA%\convert3d\bin\convert3d.exe` on Windows, and
`~/Library/Application Support/convert3d/bin/convert3d` after the macOS terminal install.

Check what was found:

```sh
convert3d doctor
```

If a tool is installed somewhere unusual:

```sh
convert3d set-tool blender "/path/to/blender"
convert3d set-tool assimp  "/path/to/assimp"
convert3d set-tool mayapy  "/path/to/mayapy"
convert3d set-tool 3dsmax  "C:\Program Files\Autodesk\3ds Max 2026\3dsmaxbatch.exe"
```

## Uninstall

- **Windows** - *Settings > Apps > Installed apps > Convert 3D > Uninstall*
- **macOS** - `convert3d uninstall`, then `sudo rm /usr/local/bin/convert3d` if you used the package
- **Linux** - double-click the AppImage again and choose **Remove**, or run `convert3d uninstall`

This removes the menus, the thumbnailer and convert3d's copy of itself.

## Where output goes

- `chair.fbx` → `chair.glb`, in the same folder.
- Formats with separate texture files (obj, gltf, usda, dae…) go into a folder: `chair_obj/`.
- If the input was a zip, multi-file results are zipped: `chair_obj.zip`.
- Nothing is overwritten. An existing file gets `chair_(2).glb`, then `chair_(3).glb`. `chair (2)` is the same file as `chair`.

## Previews

**Preview 3D** opens the model in your web browser: drag to orbit, right-drag to pan, scroll to zoom. It shows
triangle, material and texture counts and the model's size, plays animations, and has wireframe, grid and
background toggles. The page works offline. A `.glb` opens instantly; other formats are converted with
Blender first, which takes a few seconds. The last 20 preview pages are kept in the cache folder.

### Thumbnails

On Linux, `convert3d install` also adds a thumbnailer, so file managers that support freedesktop thumbnailers
show the model as its icon: KDE Plasma 6 Dolphin, Thunar, Nemo and Caja. Each thumbnail is rendered by Blender
and takes a couple of seconds the first time. The file manager caches it after that.

- KDE Plasma 5 and GNOME Files don't show them. Plasma 5 doesn't read thumbnailer files. GNOME runs thumbnailers
  in a sandbox that can't reach Blender.
- In Dolphin, check that previews are turned on, and that the file size limit in Settings → Interface → Previews
  isn't below your model sizes.
- macOS Finder and Windows Explorer have no thumbnails from convert3d. Finder's Quick Look (space bar) already
  previews `.usdz`, `.usd`, `.obj`, `.stl`, `.ply` and `.abc`.

Make one by hand:

```sh
convert3d thumbnail --size 512 chair.fbx chair.png
```

## Zips

The zip is searched for a model (FBX first, then glTF, USD, Alembic, OBJ…). Texture paths that point
somewhere that no longer exists, such as `C:\Users\someone\textures\wood.png`, are fixed by finding a file with the same
name in the zip.

## Command line

```sh
convert3d convert --to glb chair.fbx table.obj props.zip
convert3d convert --to usdz --out-dir ./usd *.fbx
convert3d formats
convert3d view chair.fbx                # open a preview in the browser
convert3d view --no-open chair.fbx      # just print the preview page's path
```

## Limitations

- `.max` needs 3ds Max, which is Windows only.
- `.ma` / `.mb` need Maya or Aztek.
- Alembic, STL and PLY have no materials, so textures are dropped when converting to them.

## Troubleshooting

When a conversion fails, the error dialog shows the end of the log. Full logs are in the folder shown by
`convert3d doctor`.

## Releasing

Every push to `main` is built and tested by GitHub Actions and, if everything passes,
published as a new release with the next patch version (v0.1.0 -> v0.1.1). The download
links above always point at the latest release, so they need no updating.

To start a new minor or major version, push a tag yourself:

    git tag v0.2.0 && git push origin v0.2.0

| Path | What |
| --- | --- |
| `src/`, `scripts/` | the program (Rust) and the scripts it runs inside Blender, Maya, 3ds Max and Aztek |
| `viewer/` | the preview page; `npm run build` rebuilds `viewer/dist` |
| `packaging/` | installer definitions (Inno Setup, macOS package, AppImage) |
| `install.sh` | the terminal installer for macOS and Linux |
| `tests/` | smoke test run against a real Blender by CI |
| `docs/` | the website (GitHub Pages) |

## License

[MIT](LICENSE)
