// A GUI-subsystem exe on Windows so right-click conversions don't flash a
// console window; `attach_console` restores output when run from a terminal.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod convert;
mod formats;
mod install;
mod plan;
mod tools;
mod ui;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use formats::Tool;
use std::fs::File;
use std::path::PathBuf;
use tools::Tools;

#[derive(Parser)]
#[command(version, about = "Convert any 3D file (or a zip of model + textures) to another 3D format")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Convert files. A zip is searched for the model and its textures.
    Convert {
        /// Output format extension, e.g. glb, fbx, usdz. Asks if omitted with --gui.
        #[arg(short, long)]
        to: Option<String>,
        /// Write results here instead of next to each input.
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
        /// Replace existing files instead of adding " (2)" to the name.
        #[arg(long)]
        overwrite: bool,
        /// Report with desktop notifications and dialogs (used by the right-click menu).
        #[arg(long)]
        gui: bool,
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
    },
    /// Add "Convert 3D to" to the file manager's right-click menu.
    Install,
    /// Remove the right-click menu.
    Uninstall,
    /// Show which converters were found.
    Doctor,
    /// List supported formats.
    Formats,
    /// Point convert3d at a tool: blender, assimp, mayapy or 3dsmax. Omit the path to reset.
    SetTool { tool: String, path: Option<PathBuf> },
}

fn main() {
    attach_console();
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.cmd {
        Cmd::Convert { to, out_dir, overwrite, gui, inputs } => {
            convert_cmd(to, out_dir, overwrite, gui, inputs)
        }
        Cmd::Install => install::install(&Tools::detect()),
        Cmd::Uninstall => install::uninstall(),
        Cmd::Doctor => {
            let tools = Tools::detect();
            for t in Tool::ALL {
                match tools.path(t) {
                    Some(p) => println!("{:8} {}", t.name(), p.display()),
                    None => println!("{:8} not found", t.name()),
                }
            }
            println!("\nSaved tool paths: {}", tools::config_path().display());
            println!("Logs: {}", ui::cache_dir().join("logs").display());
            Ok(())
        }
        Cmd::Formats => {
            let tools = Tools::detect();
            let avail = tools.available();
            println!("{:8} {:32} {:7} {:7} via", "ext", "format", "read", "write");
            for f in formats::all() {
                let mark = |ts: &[Tool]| match (ts.is_empty(), ts.iter().any(|t| avail.contains(t))) {
                    (true, _) => "-",
                    (false, true) => "yes",
                    (false, false) => "missing",
                };
                let via: std::collections::BTreeSet<_> =
                    f.read.iter().chain(f.write).map(|t| t.name()).collect();
                println!(
                    "{:8} {:32} {:7} {:7} {}",
                    f.ext,
                    f.name,
                    mark(f.read),
                    mark(f.write),
                    via.into_iter().collect::<Vec<_>>().join(", ")
                );
            }
            Ok(())
        }
        Cmd::SetTool { tool, path } => {
            let Some(t) = Tool::from_name(&tool) else {
                bail!("unknown tool {tool}; use blender, assimp, mayapy or 3dsmax");
            };
            tools::set_tool(t, path.as_deref())
        }
    }
}

fn convert_cmd(
    to: Option<String>,
    out_dir: Option<PathBuf>,
    overwrite: bool,
    gui: bool,
    inputs: Vec<PathBuf>,
) -> Result<()> {
    let tools = Tools::detect();
    let to = match to {
        Some(t) => t,
        None if gui => {
            let choices: Vec<_> = formats::all()
                .filter(|f| f.menu && !f.write.is_empty())
                .map(|f| (f.ext.into(), f.name.into()))
                .collect();
            match ui::pick_format(&choices) {
                Some(t) => t,
                None => return Ok(()),
            }
        }
        None => bail!("--to is required"),
    };
    let mut log = ui::Log::open(!gui)?;
    let opts = convert::Options { to: to.clone(), out_dir, overwrite };

    // One conversion at a time: Explorer starts a process per selected file.
    let lock = File::create(ui::cache_dir().join("lock"))?;
    if gui && lock.try_lock().is_err() {
        ui::notify("Convert 3D", "Queued behind another conversion…");
    }
    lock.lock()?;

    let mut failures = Vec::new();
    for input in &inputs {
        let name = input.file_name().unwrap_or_default().to_string_lossy().to_string();
        if gui {
            ui::notify("Converting…", &format!("{name} → .{to}"));
        }
        log.line(&format!("== {}", input.display()));
        match convert::convert(input, &opts, &tools, &mut log) {
            Ok(out) => {
                log.line(&format!("wrote {}", out.display()));
                if gui {
                    ui::notify("Converted", &out.file_name().unwrap_or_default().to_string_lossy());
                } else {
                    println!("{}", out.display());
                }
            }
            Err(e) => {
                log.line(&format!("error: {e:#}"));
                failures.push(format!("{name}: {e:#}"));
            }
        }
    }
    if failures.is_empty() {
        return Ok(());
    }
    if gui {
        ui::error_dialog(
            "Convert 3D failed",
            &format!("{}\n\nLog: {}\n\n{}", failures.join("\n"), log.path.display(), log.tail(15)),
        );
    }
    bail!("{} of {} conversions failed", failures.len(), inputs.len())
}

#[cfg(windows)]
fn attach_console() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

#[cfg(not(windows))]
fn attach_console() {}
