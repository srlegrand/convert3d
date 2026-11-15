# Run by convert3d:  aztek --batch --script aztek_convert.py
# The job path is written into JOB below. Aztek opens .ma .mb .fbx .obj and USD,
# then saves one of those. USD is .usd .usda .usdc .usdz.

import os
import sys

import maya.cmds as cmds
import maya.mel as mel

JOB = "__JOB__"
USD = (".usd", ".usda", ".usdc", ".usdz")


def mel_path(path):
    return path.replace("\\", "/").replace('"', '\\"')


def ext_of(path):
    return os.path.splitext(path)[1].lower()


def open_source(src):
    ext = ext_of(src)
    quoted = mel_path(src)
    if ext == ".ma":
        cmds.file(src, open=True, force=True, ignoreVersion=True, type="mayaAscii")
    elif ext == ".mb":
        cmds.file(src, open=True, force=True, ignoreVersion=True, type="mayaBinary")
    elif ext == ".fbx":
        cmds.file(new=True, force=True)
        mel.eval('importFbx "%s"' % quoted)
    elif ext == ".obj":
        cmds.file(new=True, force=True)
        mel.eval('importObj "%s"' % quoted)
    elif ext in USD:
        cmds.file(new=True, force=True)
        mel.eval('mayaUSDImport -file "%s"' % quoted)
    else:
        raise RuntimeError("Aztek cannot read %s" % ext)


def save_scene(dst):
    ext = ext_of(dst)
    quoted = mel_path(dst)
    if ext in (".ma", ".mb"):
        cmds.file(rename=dst)
        cmds.file(save=True, force=True, type="mayaAscii" if ext == ".ma" else "mayaBinary")
    elif ext == ".fbx":
        mel.eval("FBXResetExport")
        mel.eval("FBXExportEmbeddedTextures -v true")
        mel.eval('FBXExport -f "%s"' % quoted)
    elif ext == ".obj":
        cmds.select(all=True)
        mel.eval('exportObj "%s"' % quoted)
    elif ext in USD:
        mel.eval('mayaUSDExport -file "%s"' % quoted)
    else:
        raise RuntimeError("Aztek cannot write %s" % ext)


def main():
    import json

    with open(JOB, encoding="utf-8") as handle:
        job = json.load(handle)
    open_source(job["input"])
    save_scene(job["output"])


try:
    main()
except Exception as exc:
    sys.stderr.write("aztek convert: %s\n" % exc)
    sys.exit(1)
