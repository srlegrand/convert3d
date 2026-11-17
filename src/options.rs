//! Saved options: where converted files go, what to do when that output already
//! exists, and which application reads or writes each format. Read on every
//! conversion, so Save takes effect immediately.
//!
//! Blender, Assimp, Maya, 3ds Max and Aztek each have one shared executable. A file
//! type overrides that for its read, its write, or both. Pointing both at the
//! same other program makes that program do the whole conversion.

use crate::formats::{self, Tool};
use crate::plan::{Overrides, Slot};
use crate::tools::{self, Tools};
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What to do when the converted file or folder is already there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Existing {
    /// Ask in the same window as the output folder, when that option is on.
    #[default]
    Ask,
    Overwrite,
    /// Keep the existing file and write `name_(2).ext`.
    Increment,
}

impl Existing {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Overwrite => "overwrite",
            Self::Increment => "increment",
        }
    }

    fn parse(value: Option<&str>) -> Self {
        match value {
            Some("overwrite") => Self::Overwrite,
            Some("increment") => Self::Increment,
            _ => Self::Ask,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ask => "ask before conversion",
            Self::Overwrite => "always overwrite",
            Self::Increment => "increment the name",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub ask_output: bool,
    pub existing: Existing,
    pub overrides: Overrides,
}

pub fn path() -> PathBuf {
    tools::config_dir().join("options.json")
}

pub fn load() -> Result<Settings> {
    let file = path();
    if !file.is_file() {
        return Ok(Settings::default());
    }
    let text =
        std::fs::read_to_string(&file).with_context(|| format!("could not read {}", file.display()))?;
    parse(&text).with_context(|| format!("could not read {}", file.display()))
}

pub fn save(settings: &Settings) -> Result<()> {
    let file = path();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&file, serialize(settings))?;
    Ok(())
}

pub fn slot_label(slot: &Slot) -> String {
    match slot {
        Slot::Tool(tool) => tool.name().to_string(),
        Slot::ToolExe(tool, exe) => format!("{} ({})", tool.name(), exe.display()),
        Slot::Program(exe) => exe.display().to_string(),
    }
}

/// JSON the options window edits.
pub fn dialog_model(tools: &Tools, settings: &Settings) -> serde_json::Value {
    let available = tools.available();
    let tools_json: Vec<_> = Tool::ALL
        .into_iter()
        .map(|tool| {
            serde_json::json!({
                "id": tool.name(),
                "label": tool_label(tool),
                "path": tools.path(tool).map(|p| p.display().to_string()).unwrap_or_default(),
            })
        })
        .collect();
    let rows: Vec<_> = formats::all()
        .map(|format| {
            serde_json::json!({
                "ext": format.ext,
                "name": format.name,
                "can_read": !format.read.is_empty(),
                "can_write": !format.write.is_empty(),
                "read": display_slot(settings.overrides.read.get(format.ext), format.read, &available, tools),
                "write": display_slot(settings.overrides.write.get(format.ext), format.write, &available, tools),
            })
        })
        .collect();
    serde_json::json!({
        "ask_output": settings.ask_output,
        "existing": settings.existing.as_str(),
        "tools": tools_json,
        "rows": rows
    })
}

fn tool_label(tool: Tool) -> &'static str {
    match tool {
        Tool::Blender => "Blender",
        Tool::Assimp => "Assimp",
        Tool::Maya => "Maya (mayapy)",
        Tool::Max => "3ds Max (3dsmaxbatch)",
        Tool::Aztek => "Aztek",
    }
}

/// Writes the shared tool paths and the per-format overrides from a dialog result.
pub fn apply_dialog(tools: &Tools, before: &Settings, dialog: &serde_json::Value) -> Result<()> {
    let old_paths = tools.shared().clone();
    let old_available = available_of(&old_paths);
    let initials = initial_paths(tools, before);

    for tool in Tool::ALL {
        let entered = tool_text(dialog, tool).unwrap_or("");
        match (entered.is_empty(), old_paths.get(&tool)) {
            (true, Some(_)) => tools::set_tool(tool, None)?,
            (false, Some(current)) if same_path(current, Path::new(entered)) => {}
            (false, _) => tools::set_tool(tool, Some(Path::new(entered)))?,
            (true, None) => {}
        }
    }

    let new_tools = Tools::detect();
    let new_paths = new_tools.shared().clone();
    let new_available = available_of(&new_paths);
    let mut settings = Settings {
        ask_output: dialog.get("ask_output").and_then(|v| v.as_bool()).unwrap_or(false),
        existing: Existing::parse(dialog.get("existing").and_then(|v| v.as_str())),
        overrides: Overrides::default(),
    };

    for row in list(dialog, "rows")? {
        let ext = row.get("ext").and_then(|v| v.as_str()).context("options row is missing its extension")?;
        let Some(format) = formats::get(ext) else { continue };
        if !format.read.is_empty() {
            let text = field(row, "read");
            let initial = initials.read.get(ext).map(String::as_str).unwrap_or("");
            let slot =
                classify(text, initial, format.read, &old_available, &old_paths, &new_available, &new_paths)
                    .with_context(|| format!("read .{ext}"))?;
            remember(&mut settings.overrides.read, ext, slot, &new_paths)?;
        }
        if !format.write.is_empty() {
            let text = field(row, "write");
            let initial = initials.write.get(ext).map(String::as_str).unwrap_or("");
            let slot =
                classify(text, initial, format.write, &old_available, &old_paths, &new_available, &new_paths)
                    .with_context(|| format!("write .{ext}"))?;
            remember(&mut settings.overrides.write, ext, slot, &new_paths)?;
        }
    }
    promote_unset_tools(&mut settings.overrides)?;
    relax_pins(&mut settings.overrides);
    save(&settings)
}

fn remember(
    map: &mut BTreeMap<String, Slot>,
    ext: &str,
    slot: Option<Slot>,
    shared: &BTreeMap<Tool, PathBuf>,
) -> Result<()> {
    let Some(slot) = slot else { return Ok(()) };
    let slot = canonicalize_slot(slot)?;
    match &slot {
        Slot::Tool(tool) if !shared.contains_key(tool) => {
            bail!("{} has no executable; set it under Applications", tool.name());
        }
        Slot::Tool(_) | Slot::ToolExe(_, _) | Slot::Program(_) => {
            map.insert(ext.to_string(), slot);
        }
    }
    Ok(())
}

fn canonicalize_slot(slot: Slot) -> Result<Slot> {
    let canon =
        |path: PathBuf| path.canonicalize().with_context(|| format!("{} was not found", path.display()));
    Ok(match slot {
        Slot::Program(path) => Slot::Program(canon(path)?),
        Slot::ToolExe(tool, path) => Slot::ToolExe(tool, canon(path)?),
        Slot::Tool(tool) => Slot::Tool(tool),
    })
}

/// A built-in executable filled in on a file type, while Applications has none,
/// becomes the shared executable. `.ma` and `.mb` then both see Maya.
fn promote_unset_tools(ov: &mut Overrides) -> Result<()> {
    let tools = Tools::detect();
    let mut found: BTreeMap<Tool, PathBuf> = BTreeMap::new();
    for slot in ov.read.values().chain(ov.write.values()) {
        if let Slot::ToolExe(tool, path) = slot
            && tools.path(*tool).is_none()
        {
            found.entry(*tool).or_insert_with(|| path.clone());
        }
    }
    for (tool, path) in &found {
        tools::set_tool(*tool, Some(path))?;
    }
    if found.is_empty() {
        return Ok(());
    }
    let tools = Tools::detect();
    for map in [&mut ov.read, &mut ov.write] {
        for slot in map.values_mut() {
            if let Slot::ToolExe(tool, path) = slot
                && found.contains_key(tool)
                && tools.path(*tool).is_some_and(|saved| same_path(saved, path))
            {
                *slot = Slot::Tool(*tool);
            }
        }
    }
    Ok(())
}

fn relax_pins(ov: &mut Overrides) {
    let tools = Tools::detect();
    let available = tools.available();
    for format in formats::all() {
        relax(&mut ov.read, format.ext, format.read, &available, &tools);
        relax(&mut ov.write, format.ext, format.write, &available, &tools);
    }
}

fn relax(
    map: &mut BTreeMap<String, Slot>,
    ext: &str,
    format_tools: &[Tool],
    available: &[Tool],
    tools: &Tools,
) {
    let Some(slot) = map.get(ext).cloned() else { return };
    let pref = preferred(format_tools, available);
    match slot {
        Slot::Tool(tool) if Some(tool) == pref => {
            map.remove(ext);
        }
        Slot::ToolExe(tool, path) if tools.path(tool).is_some_and(|saved| same_path(saved, &path)) => {
            if Some(tool) == pref {
                map.remove(ext);
            } else {
                map.insert(ext.to_string(), Slot::Tool(tool));
            }
        }
        _ => {}
    }
}

struct Shown {
    read: BTreeMap<String, String>,
    write: BTreeMap<String, String>,
}

fn initial_paths(tools: &Tools, settings: &Settings) -> Shown {
    let available = tools.available();
    let mut shown = Shown { read: BTreeMap::new(), write: BTreeMap::new() };
    for format in formats::all() {
        shown.read.insert(
            format.ext.into(),
            display_slot(settings.overrides.read.get(format.ext), format.read, &available, tools),
        );
        shown.write.insert(
            format.ext.into(),
            display_slot(settings.overrides.write.get(format.ext), format.write, &available, tools),
        );
    }
    shown
}

fn display_slot(slot: Option<&Slot>, format_tools: &[Tool], available: &[Tool], tools: &Tools) -> String {
    match slot {
        Some(Slot::Program(path) | Slot::ToolExe(_, path)) => path.display().to_string(),
        Some(Slot::Tool(tool)) => tools.path(*tool).map(|p| p.display().to_string()).unwrap_or_default(),
        None if format_tools.is_empty() => String::new(),
        None => preferred(format_tools, available)
            .and_then(|tool| tools.path(tool))
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    }
}

/// `None` means "use the shared application".
pub fn classify(
    text: &str,
    initial: &str,
    format_tools: &[Tool],
    old_available: &[Tool],
    old_paths: &BTreeMap<Tool, PathBuf>,
    new_available: &[Tool],
    new_paths: &BTreeMap<Tool, PathBuf>,
) -> Result<Option<Slot>> {
    let text = text.trim();
    if text.is_empty() || format_tools.is_empty() {
        return Ok(None);
    }
    if same_text(text, initial.trim()) {
        let pref = preferred(format_tools, old_available);
        if let Some(tool) = pref
            && old_paths.get(&tool).is_some_and(|path| same_path(path, Path::new(text)))
        {
            return Ok(None);
        }
        let slot = classify_text(text, format_tools, pref, old_paths)?;
        if let Some(Slot::Tool(tool)) = &slot
            && !new_paths.contains_key(tool)
        {
            return Ok(None);
        }
        return Ok(slot);
    }
    classify_text(text, format_tools, preferred(format_tools, new_available), new_paths)
}

fn classify_text(
    text: &str,
    format_tools: &[Tool],
    preferred: Option<Tool>,
    paths: &BTreeMap<Tool, PathBuf>,
) -> Result<Option<Slot>> {
    let path = PathBuf::from(text);
    let Some(tool) = Tool::tool_of_exe(&path) else {
        return Ok(Some(Slot::Program(path)));
    };
    if !format_tools.contains(&tool) {
        bail!("{} cannot be used for this file type", tool.name());
    }
    if paths.get(&tool).is_some_and(|saved| same_path(saved, &path)) {
        return Ok(if preferred == Some(tool) { None } else { Some(Slot::Tool(tool)) });
    }
    Ok(Some(Slot::ToolExe(tool, path)))
}

fn preferred(format_tools: &[Tool], available: &[Tool]) -> Option<Tool> {
    format_tools
        .iter()
        .copied()
        .find(|tool| available.contains(tool))
        .or_else(|| format_tools.first().copied())
}

fn available_of(paths: &BTreeMap<Tool, PathBuf>) -> Vec<Tool> {
    Tool::ALL.into_iter().filter(|tool| paths.contains_key(tool)).collect()
}

fn same_text(a: &str, b: &str) -> bool {
    a == b || (!a.is_empty() && !b.is_empty() && same_path(Path::new(a), Path::new(b)))
}

pub fn same_path(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => cfg!(windows) && a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy()),
    }
}

fn tool_text(dialog: &serde_json::Value, tool: Tool) -> Option<&str> {
    list(dialog, "tools").ok()?.iter().find_map(|value| {
        (value.get("id").and_then(|id| id.as_str()) == Some(tool.name()))
            .then(|| value.get("path").and_then(|path| path.as_str()).unwrap_or("").trim())
    })
}

fn field<'a>(row: &'a serde_json::Value, key: &str) -> &'a str {
    row.get(key).and_then(|v| v.as_str()).unwrap_or("").trim()
}

fn list<'a>(value: &'a serde_json::Value, key: &str) -> Result<Vec<&'a serde_json::Value>> {
    match value.get(key) {
        Some(serde_json::Value::Array(items)) => Ok(items.iter().collect()),
        Some(item @ serde_json::Value::Object(_)) => Ok(vec![item]),
        Some(serde_json::Value::Null) | None => Ok(Vec::new()),
        Some(_) => bail!("options {key} is not a list"),
    }
}

pub fn parse(text: &str) -> Result<Settings> {
    let value: serde_json::Value = serde_json::from_str(text)?;
    let mut settings = Settings {
        ask_output: value.get("ask_output").and_then(|v| v.as_bool()).unwrap_or(false),
        existing: Existing::parse(value.get("existing").and_then(|v| v.as_str())),
        ..Settings::default()
    };
    if let Some(read) = value.get("read") {
        settings.overrides.read = parse_slots(read)?;
    }
    if let Some(write) = value.get("write") {
        settings.overrides.write = parse_slots(write)?;
    }
    Ok(settings)
}

fn parse_slots(value: &serde_json::Value) -> Result<BTreeMap<String, Slot>> {
    let obj = value.as_object().context("expected an object of file types")?;
    let mut map = BTreeMap::new();
    for (ext, slot) in obj {
        map.insert(ext.clone(), parse_slot(slot)?);
    }
    Ok(map)
}

fn parse_slot(value: &serde_json::Value) -> Result<Slot> {
    if let Some(exe) = value.get("program").and_then(|v| v.as_str()) {
        return Ok(Slot::Program(PathBuf::from(exe)));
    }
    let name = value.get("tool").and_then(|v| v.as_str()).context("application is missing a tool name")?;
    let tool = Tool::from_name(name).with_context(|| format!("unknown tool {name}"))?;
    if let Some(exe) = value.get("path").and_then(|v| v.as_str()) {
        return Ok(Slot::ToolExe(tool, PathBuf::from(exe)));
    }
    Ok(Slot::Tool(tool))
}

pub fn serialize(settings: &Settings) -> String {
    let encode = |slot: &Slot| match slot {
        Slot::Tool(tool) => serde_json::json!({ "tool": tool.name() }),
        Slot::ToolExe(tool, exe) => {
            serde_json::json!({ "tool": tool.name(), "path": exe.display().to_string() })
        }
        Slot::Program(exe) => serde_json::json!({ "program": exe.display().to_string() }),
    };
    let map = |slots: &BTreeMap<String, Slot>| {
        slots.iter().map(|(ext, slot)| (ext.clone(), encode(slot))).collect::<serde_json::Map<_, _>>()
    };
    serde_json::to_string_pretty(&serde_json::json!({
        "ask_output": settings.ask_output,
        "existing": settings.existing.as_str(),
        "read": serde_json::Value::Object(map(&settings.overrides.read)),
        "write": serde_json::Value::Object(map(&settings.overrides.write)),
    }))
    .unwrap_or_else(|_| "{}".into())
        + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use Tool::*;

    fn paths(items: &[(Tool, &str)]) -> BTreeMap<Tool, PathBuf> {
        items.iter().map(|(tool, path)| (*tool, PathBuf::from(path))).collect()
    }

    #[test]
    fn untouched_shared_path_follows_the_application() {
        let old = paths(&[(Blender, "C:/Blender/blender.exe")]);
        let new = paths(&[(Blender, "D:/Blender/blender.exe")]);
        let slot = classify(
            "C:/Blender/blender.exe",
            "C:/Blender/blender.exe",
            &[Blender],
            &[Blender],
            &old,
            &[Blender],
            &new,
        )
        .unwrap();
        assert_eq!(slot, None);
    }

    #[test]
    fn another_program_is_kept_on_that_file_type() {
        let old = paths(&[(Blender, "C:/Blender/blender.exe")]);
        let slot =
            classify("C:/tools/ma.exe", "C:/Blender/blender.exe", &[Maya], &[Maya], &old, &[Maya], &old)
                .unwrap();
        assert_eq!(slot, Some(Slot::Program(PathBuf::from("C:/tools/ma.exe"))));
    }

    #[test]
    fn known_tool_on_the_wrong_format_is_rejected() {
        let paths = paths(&[(Blender, "C:/Blender/blender.exe")]);
        let err = classify("C:/Maya/mayapy.exe", "", &[Blender], &[Blender], &paths, &[Blender], &paths)
            .unwrap_err()
            .to_string();
        assert!(err.contains("mayapy"), "{err}");
    }

    #[test]
    fn different_blender_is_only_for_that_field() {
        let shared = paths(&[(Blender, "C:/Blender/blender.exe")]);
        let slot = classify(
            "D:/Blender/blender.exe",
            "C:/Blender/blender.exe",
            &[Blender],
            &[Blender],
            &shared,
            &[Blender],
            &shared,
        )
        .unwrap();
        assert_eq!(slot, Some(Slot::ToolExe(Blender, PathBuf::from("D:/Blender/blender.exe"))));
    }

    #[test]
    fn pinning_assimp_when_blender_is_preferred() {
        let shared = paths(&[(Blender, "C:/Blender/blender.exe"), (Assimp, "C:/Assimp/assimp.exe")]);
        let slot = classify(
            "C:/Assimp/assimp.exe",
            "C:/Blender/blender.exe",
            &[Blender, Assimp],
            &[Blender, Assimp],
            &shared,
            &[Blender, Assimp],
            &shared,
        )
        .unwrap();
        assert_eq!(slot, Some(Slot::Tool(Assimp)));
    }

    #[test]
    fn options_roundtrip() {
        let mut settings =
            Settings { ask_output: true, existing: Existing::Overwrite, overrides: Overrides::default() };
        settings.overrides.read.insert("ma".into(), Slot::Program(PathBuf::from("C:/tools/ma.exe")));
        settings.overrides.write.insert("usda".into(), Slot::Tool(Blender));
        let back = parse(&serialize(&settings)).unwrap();
        assert!(back.ask_output);
        assert_eq!(back.existing, Existing::Overwrite);
        assert!(matches!(back.overrides.read.get("ma"), Some(Slot::Program(p)) if p.ends_with("ma.exe")));
        assert_eq!(back.overrides.write.get("usda"), Some(&Slot::Tool(Blender)));
    }

    #[test]
    fn missing_existing_asks() {
        let settings = parse(r#"{"ask_output":false,"read":{},"write":{}}"#).unwrap();
        assert_eq!(settings.existing, Existing::Ask);
    }
}
