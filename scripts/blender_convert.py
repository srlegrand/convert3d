# Run by convert3d inside headless Blender:
#   blender -b --factory-startup --python blender_convert.py -- job.json
# job.json: {"input", "output", "format", "search_dirs": [...], "unpack_dir"}

import json
import os
import re
import sys

import bpy

IMAGE_EXTS = (".png", ".jpg", ".jpeg", ".tga", ".tif", ".tiff", ".bmp", ".exr", ".hdr", ".webp", ".dds", ".psd")
MAX_INDEXED_FILES = 50000


def import_file(path, ext):
    if ext == ".blend":
        bpy.ops.wm.open_mainfile(filepath=path, load_ui=False)
        return
    bpy.ops.wm.read_homefile(use_empty=True)
    if ext == ".fbx":
        try:
            bpy.ops.wm.fbx_import(filepath=path)
        except (AttributeError, RuntimeError) as e:
            print(f"native FBX importer failed ({e}), trying the Python one")
            bpy.ops.import_scene.fbx(filepath=path, use_image_search=True)
    elif ext in (".glb", ".gltf"):
        bpy.ops.import_scene.gltf(filepath=path)
    elif ext == ".obj":
        bpy.ops.wm.obj_import(filepath=path)
    elif ext == ".abc":
        bpy.ops.wm.alembic_import(filepath=path, as_background_job=False)
    elif ext in (".usd", ".usda", ".usdc", ".usdz"):
        bpy.ops.wm.usd_import(filepath=path, import_textures_mode="IMPORT_PACK")
    elif ext == ".stl":
        bpy.ops.wm.stl_import(filepath=path)
    elif ext == ".ply":
        bpy.ops.wm.ply_import(filepath=path)
    else:
        raise RuntimeError(f"Blender cannot import {ext}")


def index_files(dirs):
    """Maps lower-case file names (and stems of images) to paths under dirs."""
    by_name, by_stem = {}, {}
    count = 0
    for root_dir in dirs:
        for root, subdirs, files in os.walk(root_dir):
            subdirs[:] = [d for d in subdirs if not d.startswith(".") and d != "__MACOSX"]
            for f in files:
                p = os.path.join(root, f)
                low = f.lower()
                by_name.setdefault(low, p)
                stem, ext = os.path.splitext(low)
                if ext in IMAGE_EXTS:
                    by_stem.setdefault(stem, p)
                count += 1
                if count > MAX_INDEXED_FILES:
                    return by_name, by_stem
    return by_name, by_stem


def relink_images(search_dirs):
    """Finds textures whose stored path is broken (e.g. C:\\Users\\artist\\...)."""
    broken = [
        img for img in bpy.data.images
        if img.source in {"FILE", "SEQUENCE", "TILED"}
        and not img.packed_file
        and img.filepath
        and not os.path.isfile(bpy.path.abspath(img.filepath))
    ]
    if not broken:
        return
    by_name, by_stem = index_files(search_dirs)
    for img in broken:
        name = re.split(r"[\\/]", img.filepath)[-1].lower()
        found = by_name.get(name) or by_stem.get(os.path.splitext(name)[0])
        if found:
            print(f"relinked texture {img.filepath} -> {found}")
            img.filepath = found
            img.reload()
        else:
            print(f"missing texture: {img.filepath}")


def unpack_images(unpack_dir):
    """Writes packed textures (from .blend/.usdz/.glb) to real files so every
    exporter can copy or embed them."""
    os.makedirs(unpack_dir, exist_ok=True)
    used = set()
    for img in bpy.data.images:
        if not img.packed_file or img.source != "FILE":
            continue
        name = bpy.path.clean_name(os.path.splitext(img.name)[0]) or "texture"
        ext = os.path.splitext(img.filepath)[1].lower() or "." + (img.file_format or "png").lower()
        if ext not in IMAGE_EXTS:
            ext = ".png"
        fname = name + ext
        i = 1
        while fname.lower() in used:
            i += 1
            fname = f"{name}_{i}{ext}"
        used.add(fname.lower())
        path = os.path.join(unpack_dir, fname)
        with open(path, "wb") as f:
            f.write(img.packed_file.data)
        img.filepath = path
        img.unpack(method="REMOVE")


def export_file(path, fmt):
    if fmt == "blend":
        bpy.ops.file.pack_all()
        bpy.ops.wm.save_as_mainfile(filepath=path, copy=True, compress=True)
        return
    unpack_images(JOB["unpack_dir"])
    if fmt == "fbx":
        bpy.ops.export_scene.fbx(filepath=path, path_mode="COPY", embed_textures=True, add_leaf_bones=False)
    elif fmt == "glb":
        bpy.ops.export_scene.gltf(filepath=path, export_format="GLB")
    elif fmt == "gltf":
        bpy.ops.export_scene.gltf(filepath=path, export_format="GLTF_SEPARATE")
    elif fmt == "obj":
        bpy.ops.wm.obj_export(filepath=path, path_mode="COPY", export_materials=True, export_pbr_extensions=True)
    elif fmt == "abc":
        bpy.ops.wm.alembic_export(filepath=path, as_background_job=False)
    elif fmt in ("usd", "usda", "usdc", "usdz"):
        bpy.ops.wm.usd_export(
            filepath=path,
            export_materials=True,
            generate_preview_surface=True,
            export_textures_mode="NEW",
            relative_paths=True,
        )
    elif fmt == "stl":
        bpy.ops.wm.stl_export(filepath=path)
    elif fmt == "ply":
        bpy.ops.wm.ply_export(filepath=path)
    else:
        raise RuntimeError(f"Blender cannot export .{fmt}")


argv = sys.argv[sys.argv.index("--") + 1:]
with open(argv[0], encoding="utf-8") as f:
    JOB = json.load(f)

src = JOB["input"]
import_file(src, os.path.splitext(src)[1].lower())
relink_images(list(dict.fromkeys([os.path.dirname(src)] + JOB["search_dirs"])))
export_file(JOB["output"], JOB["format"])
print("convert3d: done")
