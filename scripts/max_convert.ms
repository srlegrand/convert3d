-- Run by convert3d with 3ds Max's batch tool:
--   3dsmaxbatch.exe max_convert.ms -mxsString src:<input> -mxsString dst:<output>
-- Converts .max -> .fbx, or .fbx -> .max.

src = maxOps.mxsCmdLineArgs[#src]
dst = maxOps.mxsCmdLineArgs[#dst]

if matchPattern src pattern:"*.max" then
(
	loadMaxFile src useFileUnits:true quiet:true
	FBXExporterSetParam "EmbedTextures" true
	exportFile dst #noPrompt using:FBXEXP
)
else
(
	resetMaxFile #noPrompt
	importFile src #noPrompt using:FBXIMP
	saveMaxFile dst quiet:true
)
