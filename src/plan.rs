//! Finds the cheapest chain of tools that turns one format into another,
//! e.g. `.dae -> (assimp) -> .gltf -> (blender) -> .usdz`.

use crate::formats::{self, Tool};
use anyhow::{Context, Result, bail};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub tool: Tool,
    pub from: String,
    pub to: String,
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

pub fn plan(from: &str, to: &str, available: &[Tool]) -> Result<Vec<Step>> {
    let all: Vec<_> = formats::all().collect();
    let Some(src) = all.iter().position(|f| f.ext == from) else {
        bail!("unsupported input format .{from}");
    };
    let Some(dst) = all.iter().position(|f| f.ext == to) else {
        bail!("unsupported output format .{to}");
    };

    let mut best: BTreeMap<usize, (u32, Option<(usize, Tool)>)> = BTreeMap::new();
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
        // Blender reads the source when it can: it repairs broken texture
        // paths, Assimp doesn't.
        let blender_reads =
            node == src && all[node].read.contains(&Tool::Blender) && available.contains(&Tool::Blender);
        let readers = all[node].read.iter().filter(|t| available.contains(t));
        for &tool in readers.filter(|&&t| !(t == Tool::Assimp && blender_reads)) {
            for (next, f) in all.iter().enumerate() {
                if next == node || !f.write.contains(&tool) {
                    continue;
                }
                let penalty = if next == dst { 0 } else { hub_penalty(f.ext) };
                let c = cost + tool.cost() + penalty;
                if best.get(&next).is_none_or(|&(old, _)| c < old) {
                    best.insert(next, (c, Some((node, tool))));
                    heap.push(Reverse((c, next)));
                }
            }
        }
    }

    if !best.contains_key(&dst) {
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
    while let Some((prev, tool)) = best[&node].1 {
        steps.push(Step { tool, from: all[prev].ext.into(), to: all[node].ext.into() });
        node = prev;
    }
    steps.reverse();
    Ok(steps)
}

/// A single step that reads and writes `ext` with the same tool.
pub fn reexport(ext: &str, available: &[Tool]) -> Result<Vec<Step>> {
    let f = formats::get(ext).with_context(|| format!("unsupported format .{ext}"))?;
    let Some(&tool) = f.read.iter().find(|t| f.write.contains(t) && available.contains(t)) else {
        bail!("no available tool can re-export .{ext}");
    };
    Ok(vec![Step { tool, from: ext.into(), to: ext.into() }])
}

#[cfg(test)]
mod tests {
    use super::*;
    use Tool::*;

    fn route(from: &str, to: &str, tools: &[Tool]) -> Vec<(Tool, String)> {
        plan(from, to, tools).unwrap().into_iter().map(|s| (s.tool, s.to)).collect()
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
    fn missing_tool_is_reported() {
        let err = plan("max", "glb", &[Blender]).unwrap_err().to_string();
        assert!(err.contains("3dsmax"), "{err}");
    }
}
