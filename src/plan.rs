//! Finds the cheapest chain of tools that turns one format into another,
//! e.g. `.dae -> (assimp) -> .gltf -> (blender) -> .usdz`.
//!
//! A step is one application that reads its input format and writes its
//! output format. Options can pin a format's read or write to one application.
//! A program that is not Blender, Assimp, Maya or 3ds Max is asked to write
//! glTF or FBX when the next step needs a different application.

use crate::formats::{self, Format, Tool};
use anyhow::{Context, Result, bail};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Actor {
    Tool(Tool),
    /// `program input output`. Used when Options points a format at some other executable.
    Program(PathBuf),
}

impl Actor {
    pub fn label(&self) -> String {
        match self {
            Actor::Tool(tool) => tool.name().to_string(),
            Actor::Program(path) => path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string()),
        }
    }

    fn cost(&self) -> u32 {
        match self {
            Actor::Tool(tool) => tool.cost(),
            Actor::Program(_) => 15,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub actor: Actor,
    pub from: String,
    pub to: String,
}

/// Which application handles one direction of one format.
/// `Auto` is not stored: a missing entry means the shared tools.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Slot {
    /// Use this built-in tool and its shared executable.
    Tool(Tool),
    /// Use this built-in tool with this executable, not the shared one.
    ToolExe(Tool, PathBuf),
    /// Run this program as `program input output`.
    Program(PathBuf),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Overrides {
    pub read: BTreeMap<String, Slot>,
    pub write: BTreeMap<String, Slot>,
}

/// Executable to use for a built-in step when Options pinned that format.
/// The write pin wins when both ends name one.
pub fn exe_override<'a>(tool: Tool, step: &Step, ov: &'a Overrides) -> Option<&'a Path> {
    for slot in [ov.write.get(&step.to), ov.read.get(&step.from)].into_iter().flatten() {
        if let Slot::ToolExe(t, path) = slot
            && *t == tool
        {
            return Some(path.as_path());
        }
    }
    None
}

/// Any pinned executable for `tool`, used when the shared path was not set.
pub fn any_tool_exe(tool: Tool, ov: &Overrides) -> Option<&Path> {
    ov.read.values().chain(ov.write.values()).find_map(|slot| match slot {
        Slot::ToolExe(t, path) if *t == tool => Some(path.as_path()),
        _ => None,
    })
}

/// Extra cost for passing through a format as an intermediate. glTF keeps
/// materials and textures best, FBX is needed to reach Maya / 3ds Max.
fn hub_penalty(ext: &str) -> u32 {
    match ext {
        "gltf" => 0,
        "glb" => 1,
        "fbx" => 2,
        "obj" => 4,
        _ => 6,
    }
}

fn is_hub(ext: &str) -> bool {
    matches!(ext, "gltf" | "fbx")
}

fn has_tool_exe(tool: Tool, ov: &Overrides) -> bool {
    any_tool_exe(tool, ov).is_some()
}

pub fn plan_with(from: &str, to: &str, available: &[Tool], ov: &Overrides) -> Result<Vec<Step>> {
    let all: Vec<_> = formats::all().collect();
    let Some(src) = all.iter().position(|f| f.ext == from) else {
        bail!("unsupported input format .{from}");
    };
    let Some(dst) = all.iter().position(|f| f.ext == to) else {
        bail!("unsupported output format .{to}");
    };

    let mut best: BTreeMap<usize, (u32, Option<(usize, Actor)>)> = BTreeMap::new();
    let mut heap = BinaryHeap::new();
    best.insert(src, (0, None));
    heap.push(Reverse((0u32, src)));

    while let Some(Reverse((cost, node))) = heap.pop() {
        if node == dst {
            break;
        }
        if best.get(&node).is_some_and(|&(c, _)| c < cost) {
            continue;
        }
        for actor in readers(&all[node], node == src, available, ov, true) {
            for (next, format) in all.iter().enumerate() {
                if next == node || !writes(&actor, format, available, ov, true) {
                    continue;
                }
                let penalty = if next == dst { 0 } else { hub_penalty(format.ext) };
                let c = cost + actor.cost() + penalty;
                if best.get(&next).is_none_or(|&(old, _)| c < old) {
                    best.insert(next, (c, Some((node, actor.clone()))));
                    heap.push(Reverse((c, next)));
                }
            }
        }
    }

    if !best.contains_key(&dst) {
        if ov.read.contains_key(from) || ov.write.contains_key(to) {
            bail!("no conversion route from .{from} to .{to} with the applications set in Options");
        }
        let needed: Vec<_> = Tool::ALL
            .into_iter()
            .filter(|t| all[src].read.contains(t) || all[dst].write.contains(t))
            .filter(|t| !available.contains(t))
            .map(|t| t.name())
            .collect();
        if needed.is_empty() {
            bail!("no conversion route from .{from} to .{to}");
        }
        bail!(
            "converting .{from} to .{to} needs {} (not found; see `convert3d doctor`)",
            needed.join(" or ")
        );
    }

    let mut steps = Vec::new();
    let mut node = dst;
    while let Some((prev, actor)) = best.get(&node).and_then(|(_, prev)| prev.clone()) {
        steps.push(Step { actor, from: all[prev].ext.into(), to: all[node].ext.into() });
        node = prev;
    }
    steps.reverse();
    Ok(steps)
}

/// A single step that reads and writes `ext` with the same application.
pub fn reexport_with(ext: &str, available: &[Tool], ov: &Overrides) -> Result<Vec<Step>> {
    let format = formats::get(ext).with_context(|| format!("unsupported format .{ext}"))?;
    for actor in readers(&format, false, available, ov, false) {
        if writes(&actor, &format, available, ov, false) {
            return Ok(vec![Step { actor, from: ext.into(), to: ext.into() }]);
        }
    }
    bail!("no available tool can re-export .{ext}");
}

fn readers(format: &Format, is_source: bool, available: &[Tool], ov: &Overrides, bridge: bool) -> Vec<Actor> {
    let mut out = Vec::new();
    match ov.read.get(format.ext) {
        Some(Slot::Program(path)) => out.push(Actor::Program(path.clone())),
        Some(Slot::Tool(tool)) if available.contains(tool) && format.read.contains(tool) => {
            out.push(Actor::Tool(*tool));
        }
        Some(Slot::ToolExe(tool, _)) if format.read.contains(tool) => out.push(Actor::Tool(*tool)),
        Some(_) => {}
        None => {
            // Blender reads the source when it can: it repairs broken texture paths, Assimp doesn't.
            let blender_reads =
                is_source && format.read.contains(&Tool::Blender) && available.contains(&Tool::Blender);
            for tool in format.read {
                if available.contains(tool) && !(*tool == Tool::Assimp && blender_reads) {
                    out.push(Actor::Tool(*tool));
                }
            }
            if bridge && is_hub(format.ext) {
                for slot in ov.write.values() {
                    if let Slot::Program(path) = slot {
                        let actor = Actor::Program(path.clone());
                        if !out.contains(&actor) {
                            out.push(actor);
                        }
                    }
                }
            }
        }
    }
    out
}

fn writes(actor: &Actor, format: &Format, available: &[Tool], ov: &Overrides, bridge: bool) -> bool {
    let slot = ov.write.get(format.ext);
    match actor {
        Actor::Program(path) => match slot {
            Some(Slot::Program(pinned)) => pinned == path,
            None if bridge && is_hub(format.ext) => {
                ov.read.values().any(|s| matches!(s, Slot::Program(p) if p == path))
            }
            _ => false,
        },
        Actor::Tool(tool) => {
            if !format.write.contains(tool) {
                return false;
            }
            match slot {
                Some(Slot::Program(_)) => false,
                Some(Slot::Tool(pinned)) => pinned == tool && available.contains(tool),
                Some(Slot::ToolExe(pinned, _)) => pinned == tool,
                None => available.contains(tool) || has_tool_exe(*tool, ov),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Tool::*;

    fn route(from: &str, to: &str, tools: &[Tool]) -> Vec<(Tool, String)> {
        plan_with(from, to, tools, &Overrides::default())
            .unwrap()
            .into_iter()
            .map(|s| {
                let Actor::Tool(tool) = s.actor else { panic!("expected a built-in tool") };
                (tool, s.to)
            })
            .collect()
    }

    #[test]
    fn blender_direct() {
        assert_eq!(route("fbx", "glb", &[Blender, Assimp]), [(Blender, "glb".into())]);
        assert_eq!(route("abc", "usdz", &[Blender]), [(Blender, "usdz".into())]);
    }

    #[test]
    fn assimp_in_then_blender() {
        assert_eq!(
            route("dae", "usdz", &[Blender, Assimp]),
            [(Assimp, "gltf".into()), (Blender, "usdz".into())]
        );
    }

    #[test]
    fn blender_then_assimp_out() {
        assert_eq!(
            route("usd", "dae", &[Blender, Assimp]),
            [(Blender, "gltf".into()), (Assimp, "dae".into())]
        );
    }

    #[test]
    fn blender_reads_fbx_even_for_assimp_targets() {
        assert_eq!(
            route("fbx", "dae", &[Blender, Assimp]),
            [(Blender, "gltf".into()), (Assimp, "dae".into())]
        );
        assert_eq!(route("fbx", "dae", &[Assimp]), [(Assimp, "dae".into())]);
    }

    #[test]
    fn maya_via_fbx() {
        assert_eq!(route("ma", "glb", &[Blender, Maya]), [(Maya, "fbx".into()), (Blender, "glb".into())]);
        assert_eq!(route("ma", "fbx", &[Blender, Maya]), [(Maya, "fbx".into())]);
    }

    #[test]
    fn aztek_converts_maya_files_directly() {
        assert_eq!(route("ma", "usda", &[Blender, Maya, Aztek]), [(Aztek, "usda".into())]);
        assert_eq!(route("ma", "fbx", &[Aztek]), [(Aztek, "fbx".into())]);
        assert_eq!(route("ma", "fbx", &[Maya, Aztek]), [(Maya, "fbx".into())]);
        assert_eq!(route("obj", "fbx", &[Blender, Aztek]), [(Blender, "fbx".into())]);
    }

    #[test]
    fn missing_tool_is_reported() {
        let err = plan_with("max", "glb", &[Blender], &Overrides::default()).unwrap_err().to_string();
        assert!(err.contains("3dsmax"), "{err}");
    }

    #[test]
    fn custom_reader_hands_gltf_to_blender() {
        let mut ov = Overrides::default();
        ov.read.insert("ma".into(), Slot::Program(PathBuf::from("C:/tools/ma.exe")));
        let steps = plan_with("ma", "usda", &[Blender, Maya], &ov).unwrap();
        assert_eq!(steps.len(), 2, "{steps:?}");
        assert!(matches!(&steps[0].actor, Actor::Program(p) if p.ends_with("ma.exe")));
        assert_eq!(steps[0].to, "gltf");
        assert_eq!(steps[1].actor, Actor::Tool(Blender));
        assert_eq!(steps[1].to, "usda");
    }

    #[test]
    fn same_program_does_the_whole_conversion() {
        let path = PathBuf::from("C:/tools/convert.exe");
        let mut ov = Overrides::default();
        ov.read.insert("ma".into(), Slot::Program(path.clone()));
        ov.write.insert("usda".into(), Slot::Program(path));
        let steps = plan_with("ma", "usda", &[Blender], &ov).unwrap();
        assert_eq!(steps.len(), 1, "{steps:?}");
        assert!(matches!(steps[0].actor, Actor::Program(_)));
        assert_eq!(steps[0].to, "usda");
    }

    #[test]
    fn custom_writer_reads_the_source_when_it_is_a_hub() {
        let mut ov = Overrides::default();
        ov.write.insert("usda".into(), Slot::Program(PathBuf::from("C:/tools/usd.exe")));
        let steps = plan_with("fbx", "usda", &[Blender], &ov).unwrap();
        assert_eq!(steps.len(), 1, "{steps:?}");
        assert!(matches!(steps[0].actor, Actor::Program(_)));
    }

    #[test]
    fn pinned_tool_replaces_the_default_reader() {
        let mut ov = Overrides::default();
        ov.read.insert("fbx".into(), Slot::Tool(Assimp));
        let steps = plan_with("fbx", "dae", &[Blender, Assimp], &ov).unwrap();
        assert_eq!(steps.len(), 1, "{steps:?}");
        assert_eq!(steps[0].actor, Actor::Tool(Assimp));
        assert_eq!(steps[0].to, "dae");
    }

    #[test]
    fn tool_exe_keeps_the_builtin_route_and_records_the_path() {
        let path = PathBuf::from("D:/Blender/blender.exe");
        let mut ov = Overrides::default();
        ov.write.insert("usda".into(), Slot::ToolExe(Blender, path));
        let steps = plan_with("fbx", "usda", &[Blender], &ov).unwrap();
        assert_eq!(steps, vec![Step { actor: Actor::Tool(Blender), from: "fbx".into(), to: "usda".into() }]);
        assert!(exe_override(Blender, &steps[0], &ov).unwrap().ends_with("blender.exe"));
    }
}
