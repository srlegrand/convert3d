# convert3d

Right-click a 3D file, or a zip with a model and its textures, and convert it to another 3D format.

Works in Windows Explorer, macOS Finder, KDE Dolphin and GNOME Files.

## Formats

| Format | Needs |
|---|---|
| fbx, glb, gltf, obj, abc, usd, usda, usdc, usdz, blend, stl, ply | Blender 4.2+ |
| dae, 3ds, x, 3mf, x3d (and ~35 more read-only: lwo, ase, ms3d, md2, md3, ifc, off, smd…) | Assimp |
| ma, mb | Maya |
| max | 3ds Max (Windows only) |

Blender is required. The rest are optional. Any format can be converted to any other if the tools for both are installed.

## Install

### Windows

1. Install Blender: [blender.org/download](https://www.blender.org/download/) or `winget install BlenderFoundation.Blender`
2. Download `convert3d-windows-x86_64.zip` from [Releases](../../releases) and extract it.
3. Open PowerShell in that folder and run:
   ```powershell
   .\convert3d.exe install
   ```
4. Right-click a file → **Show more options** → **Convert 3D to**.

You can delete the downloaded folder afterwards. convert3d copies itself to `%LOCALAPPDATA%\convert3d`.

### macOS

1. Install Blender: [blender.org/download](https://www.blender.org/download/) or `brew install --cask blender`
2. Optional: `brew install assimp`
3. Download `convert3d-macos-arm64.tar.gz` (Apple Silicon) or `convert3d-macos-x86_64.tar.gz` (Intel) from [Releases](../../releases), then in Terminal:
   ```sh
   cd ~/Downloads
   tar xzf convert3d-macos-*.tar.gz
   cd convert3d-macos-*
   xattr -d com.apple.quarantine convert3d
   ./convert3d install
   ```
4. Right-click a file → **Quick Actions** → **Convert 3D…** and pick a format.

If the Quick Action doesn't appear: System Settings → General → Login Items & Extensions → Finder, and turn on **Convert 3D…**.

### Linux

1. Install Blender: `sudo snap install blender --classic`, your distro's package, or the tarball from blender.org.
2. Optional: `sudo apt install assimp-utils` (Debian/Ubuntu) or `sudo dnf install assimp` (Fedora)
3. Download `convert3d-linux-x86_64.tar.gz` (or `-arm64`) from [Releases](../../releases), then:
   ```sh
   tar xzf convert3d-linux-*.tar.gz
   ./convert3d-linux-*/convert3d install
   ```
4. Close all file manager windows and reopen.
5. Right-click a file → **Convert 3D to** (Dolphin), or **Scripts → Convert 3D to** (GNOME Files).

Flatpak Blender isn't detected automatically. Point convert3d at it: `convert3d set-tool blender /path/to/blender`.

### From source (any platform)

Needs [Rust](https://rustup.rs).

```sh
cargo install --git https://github.com/srlegrand/convert3d
convert3d install
```

## After installing new tools

If you install Assimp, Maya or 3ds Max later, run `convert3d install` again to add their formats to the menu.

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

```sh
convert3d uninstall
```

## Where output goes

- `chair.fbx` → `chair.glb`, in the same folder.
- Formats with separate texture files (obj, gltf, usda, dae…) go into a folder: `chair_obj/`.
- If the input was a zip, multi-file results are zipped: `chair_obj.zip`.
- Nothing is overwritten. An existing file gets `chair (2).glb`.

## Zips

The zip is searched for a model (FBX first, then glTF, USD, Alembic, OBJ…). Texture paths that point
somewhere that no longer exists, such as `C:\Users\someone\textures\wood.png`, are fixed by finding a file with the same
name in the zip.

## Command line

```sh
convert3d convert --to glb chair.fbx table.obj props.zip
convert3d convert --to usdz --out-dir ./usd *.fbx
convert3d formats
```

## Limitations

- `.max` needs 3ds Max, which is Windows only.
- `.ma` / `.mb` need Maya.
- Alembic, STL and PLY have no materials, so textures are dropped when converting to them.

## Troubleshooting

When a conversion fails, the error dialog shows the end of the log. Full logs are in the folder shown by
`convert3d doctor`.

## License

MIT
