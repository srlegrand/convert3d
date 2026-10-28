# Run by convert3d with mayapy:  mayapy maya_convert.py <input> <output>
# Converts .ma/.mb -> .fbx, or .fbx -> .ma/.mb.

import sys

import maya.standalone

maya.standalone.initialize(name="python")

import maya.cmds as cmds  # noqa: E402
import maya.mel as mel  # noqa: E402

src, dst = sys.argv[1], sys.argv[2]
cmds.loadPlugin("fbxmaya", quiet=True)

if src.lower().endswith((".ma", ".mb")):
    cmds.file(src, open=True, force=True, ignoreVersion=True)
    mel.eval("FBXResetExport")
    mel.eval("FBXExportEmbeddedTextures -v true")
    mel.eval('FBXExport -f "%s"' % dst.replace("\\", "/"))
else:
    cmds.file(new=True, force=True)
    cmds.file(src, i=True, type="FBX", ignoreVersion=True, mergeNamespacesOnClash=False)
    cmds.file(rename=dst)
    cmds.file(save=True, force=True, type="mayaAscii" if dst.lower().endswith(".ma") else "mayaBinary")

maya.standalone.uninitialize()
