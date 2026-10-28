//! Locating and running the external converters (Blender, Assimp, Maya, 3ds Max).

use crate::formats::{self, Tool};
use crate::ui::Log;
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const BLENDER_SCRIPT: &str = include_str!("../scripts/blender_convert.py");
const MAYA_SCRIPT: &str = include_str!("../scripts/maya_convert.py");
const MAX_SCRIPT: &str = include_str!("../scripts/max_convert.ms");

pub fn config_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("convert3d").join("tools.txt")
}

/// Tool paths saved with `convert3d set-tool`, one `name = path` per line.
pub fn read_config() -> BTreeMap<String, PathBuf> {
    fs::read_to_string(config_path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), PathBuf::from(v.trim())))
        .collect()
}

pub fn set_tool(tool: Tool, path: Option<&Path>) -> Result<()> {
    let mut cfg = read_config();
    match path {
        Some(p) => {
            let p = p.canonicalize().with_context(|| format!("{} not found", p.display()))?;
            cfg.insert(tool.name().into(), p);
        }
        None => {
            cfg.remove(tool.name());
        }
    }
    let file = config_path();
    fs::create_dir_all(file.parent().unwrap())?;
    let body: String = cfg.iter().map(|(k, v)| format!("{k} = {}\n", v.display())).collect();
    fs::write(&file, body)?;
    Ok(())
}

pub struct Tools {
    paths: BTreeMap<Tool, PathBuf>,
}

impl Tools {
    pub fn detect() -> Tools {
        let cfg = read_config();
        let paths = Tool::ALL
            .into_iter()
            .filter_map(|t| {
                let env = format!("CONVERT3D_{}", t.name().to_ascii_uppercase());
                let found = cfg
                    .get(t.name())
                    .cloned()
                    .filter(|p| p.is_file())
                    .or_else(|| std::env::var_os(&env).map(PathBuf::from).filter(|p| p.is_file()))
                    .or_else(|| find_tool(t));
                found.map(|p| (t, p))
            })
            .collect();
        Tools { paths }
    }

    pub fn path(&self, tool: Tool) -> Option<&Path> {
        self.paths.get(&tool).map(PathBuf::as_path)
    }

    pub fn available(&self) -> Vec<Tool> {
        self.paths.keys().copied().collect()
    }

    /// Runs one conversion step. `search_dirs` are where Blender looks for
    /// textures whose recorded paths are broken.
    pub fn run(
        &self,
        tool: Tool,
        input: &Path,
        output: &Path,
        search_dirs: &[PathBuf],
        work: &Path,
        log: &mut Log,
    ) -> Result<()> {
        let exe = self.path(tool).with_context(|| format!("{} not found", tool.name()))?;
        let out_ext = formats::ext_of(output);
        let mut cmd;
        match tool {
            Tool::Blender => {
                let script = work.join("blender_convert.py");
                fs::write(&script, BLENDER_SCRIPT)?;
                let job = work.join("job.json");
                let tex_dir = work.join("unpacked_textures");
                fs::write(
                    &job,
                    serde_json::json!({
                        "input": input,
                        "output": output,
                        "format": out_ext,
                        "search_dirs": search_dirs,
                        "unpack_dir": tex_dir,
                    })
                    .to_string(),
                )?;
                cmd = Command::new(exe);
                cmd.args(["-b", "--factory-startup", "--python-exit-code", "1", "--python"])
                    .arg(&script)
                    .arg("--")
                    .arg(&job);
            }
            Tool::Assimp => {
                let id = formats::get(&out_ext)
                    .and_then(|f| f.assimp_id)
                    .with_context(|| format!("assimp cannot write .{out_ext}"))?;
                cmd = Command::new(exe);
                cmd.arg("export").arg(input).arg(output).arg(format!("-f{id}"));
            }
            Tool::Maya => {
                let script = work.join("maya_convert.py");
                fs::write(&script, MAYA_SCRIPT)?;
                cmd = Command::new(exe);
                cmd.arg(&script).arg(input).arg(output);
            }
            Tool::Max => {
                let script = work.join("max_convert.ms");
                fs::write(&script, MAX_SCRIPT)?;
                cmd = Command::new(exe);
                cmd.arg(&script)
                    .arg("-mxsString")
                    .arg(format!("src:{}", input.display()))
                    .arg("-mxsString")
                    .arg(format!("dst:{}", output.display()));
            }
        }
        log.line(&format!("$ {cmd:?}"));
        let status = run_logged(&mut cmd, log)?;
        if !output.exists() {
            bail!("{} failed ({status}) and produced no output", tool.name());
        }
        Ok(())
    }
}

fn run_logged(cmd: &mut Command, log: &mut Log) -> Result<std::process::ExitStatus> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
        .stdin(Stdio::null())
        .stdout(log.stdio()?)
        .stderr(log.stdio()?)
        .status()
        .with_context(|| format!("could not start {:?}", cmd.get_program()))?;
    Ok(out)
}

fn exe(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_string() }
}

fn on_path(name: &str) -> Option<PathBuf> {
    let name = exe(name);
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(&name)).find(|p| p.is_file())
}

/// Subdirectories of `parent` whose name starts with `prefix`, newest
/// version (highest name) first.
fn versioned_dirs(parent: &Path, prefix: &str) -> Vec<PathBuf> {
    let mut dirs: Vec<_> = fs::read_dir(parent)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(prefix)))
        .collect();
    dirs.sort_by_key(|p| natural_key(&p.file_name().unwrap().to_string_lossy()));
    dirs.reverse();
    dirs
}

/// Sort key that orders "Blender 4.10" after "Blender 4.2".
fn natural_key(s: &str) -> Vec<(u64, String)> {
    let mut key = Vec::new();
    let mut num = String::new();
    let mut text = String::new();
    for c in s.chars().chain(std::iter::once('\0')) {
        if c.is_ascii_digit() {
            if !text.is_empty() {
                key.push((0, std::mem::take(&mut text)));
            }
            num.push(c);
        } else {
            if !num.is_empty() {
                key.push((num.parse().unwrap_or(0), String::new()));
                num.clear();
            }
            text.push(c);
        }
    }
    key
}

fn first_file(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|p| p.is_file())
}

fn program_files() -> Vec<PathBuf> {
    ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect()
}

fn find_tool(tool: Tool) -> Option<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    match tool {
        Tool::Blender => on_path("blender").or_else(|| {
            let mut c = vec![
                PathBuf::from("/Applications/Blender.app/Contents/MacOS/Blender"),
                home.join("Applications/Blender.app/Contents/MacOS/Blender"),
                PathBuf::from("/snap/bin/blender"),
                PathBuf::from("/usr/bin/blender"),
                PathBuf::from("/usr/local/bin/blender"),
            ];
            for pf in program_files() {
                for d in versioned_dirs(&pf.join("Blender Foundation"), "Blender") {
                    c.push(d.join("blender.exe"));
                }
                c.push(pf.join("Steam/steamapps/common/Blender/blender.exe"));
            }
            for d in versioned_dirs(&home.join("Applications"), "blender") {
                c.push(d.join("blender"));
            }
            first_file(c)
        }),
        Tool::Assimp => on_path("assimp").or_else(|| {
            let mut c =
                vec![PathBuf::from("/opt/homebrew/bin/assimp"), PathBuf::from("/usr/local/bin/assimp")];
            for pf in program_files() {
                c.push(pf.join("Assimp/bin/x64/assimp.exe"));
                c.push(pf.join("Assimp/bin/assimp.exe"));
            }
            first_file(c)
        }),
        Tool::Maya => on_path("mayapy").or_else(|| {
            let mut c = Vec::new();
            for pf in program_files() {
                for d in versioned_dirs(&pf.join("Autodesk"), "Maya") {
                    c.push(d.join("bin/mayapy.exe"));
                }
            }
            for d in versioned_dirs(Path::new("/usr/autodesk"), "maya") {
                c.push(d.join("bin/mayapy"));
            }
            for d in versioned_dirs(Path::new("/Applications/Autodesk"), "maya") {
                c.push(d.join("Maya.app/Contents/bin/mayapy"));
            }
            first_file(c)
        }),
        Tool::Max => on_path("3dsmaxbatch").or_else(|| {
            let mut c = Vec::new();
            for pf in program_files() {
                for d in versioned_dirs(&pf.join("Autodesk"), "3ds Max") {
                    c.push(d.join("3dsmaxbatch.exe"));
                }
            }
            first_file(c)
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::natural_key;

    #[test]
    fn natural_sort() {
        assert!(natural_key("Blender 4.10") > natural_key("Blender 4.2"));
        assert!(natural_key("Maya2026") > natural_key("Maya2025"));
    }
}
