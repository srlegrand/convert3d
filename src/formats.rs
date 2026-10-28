//! The table of every file format convert3d knows about, and which external
//! tool can read or write it.

use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tool {
    Blender,
    Assimp,
    Maya,
    Max,
}

impl Tool {
    pub const ALL: [Tool; 4] = [Tool::Blender, Tool::Assimp, Tool::Maya, Tool::Max];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Blender => "blender",
            Tool::Assimp => "assimp",
            Tool::Maya => "mayapy",
            Tool::Max => "3dsmax",
        }
    }

    pub fn from_name(s: &str) -> Option<Tool> {
        Tool::ALL.into_iter().find(|t| t.name().eq_ignore_ascii_case(s)).or(
            match s.to_ascii_lowercase().as_str() {
                "maya" => Some(Tool::Maya),
                "max" | "3dsmaxbatch" => Some(Tool::Max),
                _ => None,
            },
        )
    }

    /// Relative cost of a conversion step, used to prefer Blender when several
    /// routes exist.
    pub fn cost(self) -> u32 {
        match self {
            Tool::Blender => 10,
            Tool::Assimp => 12,
            Tool::Maya | Tool::Max => 15,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Format {
    pub ext: &'static str,
    pub name: &'static str,
    pub read: &'static [Tool],
    pub write: &'static [Tool],
    /// Exporter id passed to `assimp export -f`.
    pub assimp_id: Option<&'static str>,
    /// Offered as a target in the right-click menu.
    pub menu: bool,
}

use Tool::{Assimp as A, Blender as B, Max as X, Maya as M};

const fn fmt(
    ext: &'static str,
    name: &'static str,
    read: &'static [Tool],
    write: &'static [Tool],
    assimp_id: Option<&'static str>,
    menu: bool,
) -> Format {
    Format { ext, name, read, write, assimp_id, menu }
}

/// Assimp-only formats that can be read but not written.
const ASSIMP_READ_ONLY: &[(&str, &str)] = &[
    ("off", "Object File Format"),
    ("lwo", "LightWave Object"),
    ("lws", "LightWave Scene"),
    ("lxo", "Modo"),
    ("ase", "3ds Max ASCII Scene"),
    ("ac", "AC3D"),
    ("ms3d", "MilkShape 3D"),
    ("md2", "Quake II Model"),
    ("md3", "Quake III Model"),
    ("md5mesh", "Doom 3 Model"),
    ("mdl", "Quake / Half-Life Model"),
    ("b3d", "Blitz3D"),
    ("bvh", "BioVision Motion"),
    ("cob", "TrueSpace"),
    ("csm", "CharacterStudio Motion"),
    ("dxf", "AutoCAD DXF"),
    ("hmp", "3D GameStudio Terrain"),
    ("ifc", "IFC (BIM)"),
    ("ifczip", "IFC (BIM, zipped)"),
    ("irr", "Irrlicht Scene"),
    ("irrmesh", "Irrlicht Mesh"),
    ("iqm", "Inter-Quake Model"),
    ("nff", "Neutral File Format"),
    ("ogex", "Open Game Engine Exchange"),
    ("pmx", "MikuMikuDance"),
    ("q3o", "Quick3D Object"),
    ("q3s", "Quick3D Scene"),
    ("smd", "Valve SMD"),
    ("ter", "Terragen Terrain"),
    ("xgl", "XGL"),
    ("zgl", "XGL (zipped)"),
    ("amf", "Additive Manufacturing Format"),
    ("ndo", "Nendo"),
    ("sib", "Silo"),
    ("zae", "Collada (zipped)"),
];

pub static FORMATS: &[Format] = &[
    fmt("glb", "glTF Binary", &[B, A], &[B], None, true),
    fmt("gltf", "glTF", &[B, A], &[B, A], Some("gltf2"), true),
    fmt("fbx", "FBX", &[B, A, M, X], &[B, M, X], None, true),
    fmt("obj", "Wavefront OBJ", &[B, A], &[B], None, true),
    fmt("usd", "USD", &[B], &[B], None, true),
    fmt("usda", "USD ASCII", &[B], &[B], None, true),
    fmt("usdc", "USD Crate", &[B], &[B], None, true),
    fmt("usdz", "USDZ", &[B], &[B], None, true),
    fmt("abc", "Alembic", &[B], &[B], None, true),
    fmt("blend", "Blender", &[B], &[B], None, true),
    fmt("stl", "STL", &[B, A], &[B], None, true),
    fmt("ply", "PLY", &[B, A], &[B], None, true),
    fmt("dae", "Collada", &[A], &[A], Some("collada"), true),
    fmt("3ds", "3D Studio", &[A], &[A], Some("3ds"), true),
    fmt("x", "DirectX", &[A], &[A], Some("x"), true),
    fmt("3mf", "3MF", &[A], &[A], Some("3mf"), true),
    fmt("x3d", "X3D", &[A], &[A], Some("x3d"), true),
    fmt("ma", "Maya ASCII", &[M], &[M], None, true),
    fmt("mb", "Maya Binary", &[M], &[M], None, true),
    fmt("max", "3ds Max", &[X], &[X], None, true),
];

pub fn all() -> impl Iterator<Item = Format> {
    FORMATS
        .iter()
        .copied()
        .chain(ASSIMP_READ_ONLY.iter().map(|&(ext, name)| fmt(ext, name, &[A], &[], None, false)))
}

pub fn get(ext: &str) -> Option<Format> {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    all().find(|f| f.ext == ext)
}

/// Lower-cased extension of a path, without the dot.
pub fn ext_of(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

pub fn is_zip(path: &Path) -> bool {
    ext_of(path) == "zip"
}

/// When a zip holds several models, the first extension in this list wins.
pub const ZIP_PRIORITY: &[&str] = &[
    "fbx", "glb", "gltf", "usdz", "usd", "usdc", "usda", "abc", "obj", "blend", "dae", "3ds", "ma", "mb",
    "max",
];
