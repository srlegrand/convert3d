//! Registers the "Convert 3D to" right-click menu with the OS file manager.

#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use crate::formats::{self, Format, Tool};
use crate::tools::Tools;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Where `install` keeps its own copy of the executable.
pub fn install_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("convert3d")
}

pub struct MenuSpec {
    /// The installed executable the menu entries launch.
    pub exe: PathBuf,
    /// Output formats offered in the submenu.
    pub targets: Vec<Format>,
    /// Extensions (without dot) the menu appears on, including "zip".
    /// (The macOS Quick Action accepts any file instead.)
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub inputs: Vec<&'static str>,
}

pub fn install(tools: &Tools) -> Result<()> {
    let exe = copy_self()?;
    let available = tools.available();
    // Blender formats are always listed so the menu works once Blender is installed.
    let usable = |t: &&Tool| **t == Tool::Blender || available.contains(t);
    let targets: Vec<Format> =
        formats::all().filter(|f| f.menu && f.write.iter().any(|t| usable(&t))).collect();
    let mut inputs: Vec<&'static str> =
        formats::all().filter(|f| !f.read.is_empty()).map(|f| f.ext).collect();
    inputs.push("zip");
    let spec = MenuSpec { exe, targets, inputs };

    #[cfg(all(unix, not(target_os = "macos")))]
    linux::install(&spec)?;
    #[cfg(target_os = "macos")]
    macos::install(&spec)?;
    #[cfg(windows)]
    windows::install(&spec)?;

    println!("Installed {}", spec.exe.display());
    #[cfg(all(unix, not(target_os = "macos")))]
    println!("Restart your file manager (close all its windows) to see the menu.");
    println!("Menu targets: {}", spec.targets.iter().map(|f| f.ext).collect::<Vec<_>>().join(", "));
    if tools.path(Tool::Blender).is_none() {
        println!("\nWARNING: Blender was not found. Install it, or run `convert3d set-tool blender <path>`.");
    }
    if tools.path(Tool::Assimp).is_none() {
        println!(
            "Tip: install Assimp for dae/3ds/x/3mf/x3d and ~35 more input formats, then re-run `convert3d install`."
        );
    }
    Ok(())
}

pub fn uninstall() -> Result<()> {
    #[cfg(all(unix, not(target_os = "macos")))]
    linux::uninstall()?;
    #[cfg(target_os = "macos")]
    macos::uninstall()?;
    #[cfg(windows)]
    windows::uninstall()?;
    // On Windows the running exe can't delete itself; ignore that.
    let _ = fs::remove_dir_all(install_dir().join("bin"));
    println!("Removed the right-click menu.");
    Ok(())
}

fn copy_self() -> Result<PathBuf> {
    let current = std::env::current_exe()?.canonicalize()?;
    let dir = install_dir().join("bin");
    fs::create_dir_all(&dir)?;
    let dest = dir.join(current.file_name().unwrap());
    if dest.canonicalize().ok().as_deref() != Some(current.as_path()) {
        fs::copy(&current, &dest).with_context(|| format!("could not copy to {}", dest.display()))?;
    }
    Ok(dest)
}

#[cfg(not(windows))]
pub fn write_file(path: &std::path::Path, body: &str) -> Result<()> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(path, body).with_context(|| format!("could not write {}", path.display()))
}
