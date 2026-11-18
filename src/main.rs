// A GUI-subsystem exe on Windows so right-click conversions don't flash a
// console window; `attach_console` restores output when run from a terminal.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod convert;
mod formats;
mod install;
mod options;
mod plan;
mod preview;
mod tools;
mod ui;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use formats::Tool;
use std::fs::File;
use std::path::{Path, PathBuf};
use tools::Tools;

/// The release version when built by CI, else the one in Cargo.toml.
const VERSION: &str = match option_env!("CONVERT3D_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

#[derive(Parser)]
#[command(
    version = VERSION,
    about = "Convert or preview any 3D file (or a zip of model + textures)"
)]
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
        /// Replace existing files instead of adding "_(2)" to the name.
        #[arg(long)]
        overwrite: bool,
        /// Report with desktop notifications and dialogs (used by the right-click menu).
        #[arg(long)]
        gui: bool,
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
    },
    /// Open an interactive 3D preview in the web browser.
    View {
        /// Only write the preview page and print its path.
        #[arg(long)]
        no_open: bool,
        /// Report with desktop notifications and dialogs (used by the right-click menu).
        #[arg(long)]
        gui: bool,
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
    },
    /// Render a PNG thumbnail (used by the file manager).
    Thumbnail {
        /// Width and height in pixels.
        #[arg(short, long, default_value_t = 256)]
        size: u32,
        input: PathBuf,
        output: PathBuf,
    },
    /// Add "Convert 3D to" and "Preview 3D" to the file manager's right-click
    /// menu, and 3D thumbnails where the file manager supports them.
    Install,
    /// Remove the right-click menu and thumbnails.
    Uninstall,
    /// Show which converters were found.
    Doctor,
    /// List supported formats.
    Formats,
    /// Point convert3d at a tool: blender, assimp, mayapy or 3dsmax. Omit the path to reset.
    SetTool { tool: String, path: Option<PathBuf> },
    /// Choose the application for each file type, and where converted files are saved.
    Options {
        /// Selected files, ignored. File managers may append them.
        #[arg(hide = true)]
        _files: Vec<PathBuf>,
    },
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
        Cmd::View { no_open, gui, inputs } => view_cmd(no_open, gui, inputs),
        Cmd::Thumbnail { size, input, output } => {
            let mut log = ui::Log::open_in("thumbnail-logs", false)?;
            log.line(&format!("== {}", input.display()));
            let result =
                preview::thumbnail(&input, &output, size.clamp(16, 2048), &Tools::detect(), &mut log);
            if let Err(e) = &result {
                log.line(&format!("error: {e:#}"));
            }
            result
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
            match options::load() {
                Ok(settings) => {
                    println!(
                        "Output: {}",
                        if settings.ask_output {
                            "ask for a folder before each conversion"
                        } else {
                            "same folder as the original"
                        }
                    );
                    println!("Existing files: {}", settings.existing.label());
                    if settings.overrides.read.is_empty() && settings.overrides.write.is_empty() {
                        println!("Format overrides: none");
                    } else {
                        println!("Format overrides:");
                        for (ext, slot) in &settings.overrides.read {
                            println!("  .{ext} read   {}", options::slot_label(slot));
                        }
                        for (ext, slot) in &settings.overrides.write {
                            println!("  .{ext} write  {}", options::slot_label(slot));
                        }
                    }
                }
                Err(e) => println!("Options: could not read {} ({e:#})", options::path().display()),
            }
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
                bail!("unknown tool {tool}; use blender, assimp, mayapy, 3dsmax or aztek");
            };
            tools::set_tool(t, path.as_deref())
        }
        Cmd::Options { .. } => options_cmd(),
    }
}

fn options_cmd() -> Result<()> {
    let tools = Tools::detect();
    let settings = options::load()?;
    let model = options::dialog_model(&tools, &settings);
    match ui::edit_options(&model) {
        Ok(None) => Ok(()),
        Ok(Some(edited)) => {
            options::apply_dialog(&tools, &settings, &edited)?;
            ui::notify("Convert 3D", "Options saved. The next conversion uses them.");
            Ok(())
        }
        Err(e) => {
            ui::error_dialog("Convert 3D Options", &format!("{e:#}"));
            Err(e)
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
    let settings = match options::load() {
        Ok(settings) => settings,
        Err(e) => {
            if gui {
                ui::error_dialog("Convert 3D failed", &format!("{e:#}"));
            }
            return Err(e);
        }
    };
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
    let (out_dir, overwrite) = match placement(&inputs, &to, &settings, overwrite, out_dir) {
        Ok(Some(place)) => place,
        Ok(None) => return Ok(()),
        Err(e) => {
            if gui {
                ui::error_dialog("Convert 3D", &format!("{e:#}"));
            }
            return Err(e);
        }
    };
    let mut log = ui::Log::open(!gui)?;
    let opts = convert::Options { to: to.clone(), out_dir, overwrite };

    // One conversion at a time: Explorer starts a process per selected file.
    // try_lock already holds the exclusive lock. lock() again waits forever on
    // Windows, because the same process never grants that lock to itself.
    let lock = File::create(ui::cache_dir().join("lock"))?;
    if lock.try_lock().is_err() {
        if gui {
            ui::notify("Convert 3D", "Queued behind another conversion…");
        }
        lock.lock()?;
    }

    let mut failures = Vec::new();
    for input in &inputs {
        let name = input.file_name().unwrap_or_default().to_string_lossy().to_string();
        if gui {
            ui::notify("Converting…", &format!("{name} → .{to}"));
        }
        log.line(&format!("== {}", input.display()));
        match convert::convert(input, &opts, &tools, &settings.overrides, &mut log) {
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

/// Folder and overwrite choice for this run. `None` means the user cancelled.
/// `--out-dir` skips the folder half and `--overwrite` skips the conflict half.
/// When both halves are on, they are one window.
fn placement(
    inputs: &[PathBuf],
    to: &str,
    settings: &options::Settings,
    cli_overwrite: bool,
    cli_out_dir: Option<PathBuf>,
) -> Result<Option<(Option<PathBuf>, bool)>> {
    let ask_folder = cli_out_dir.is_none() && settings.ask_output;
    let ask_conflict = !cli_overwrite && settings.existing == options::Existing::Ask;
    let overwrite = cli_overwrite || settings.existing == options::Existing::Overwrite;
    if !ask_folder && !ask_conflict {
        return Ok(Some((cli_out_dir, overwrite)));
    }

    let folder = cli_out_dir.clone().unwrap_or_else(|| {
        inputs
            .first()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."))
    });
    let jobs: Vec<Vec<String>> = inputs.iter().map(|path| convert::candidate_names(path, to)).collect();
    if ask_conflict && !ask_folder && !jobs.iter().flatten().any(|name| folder.join(name).exists()) {
        return Ok(Some((cli_out_dir, false)));
    }

    let Some(choice) = ui::place_dialog(&ui::PlaceRequest { ask_folder, ask_conflict, folder, jobs })? else {
        return Ok(None);
    };
    let out_dir = if ask_folder { Some(choice.folder) } else { cli_out_dir };
    let overwrite = if ask_conflict { choice.overwrite } else { overwrite };
    Ok(Some((out_dir, overwrite)))
}

fn view_cmd(no_open: bool, gui: bool, inputs: Vec<PathBuf>) -> Result<()> {
    let tools = Tools::detect();
    let mut log = ui::Log::open(!gui)?;
    // A preview is still worth showing when the options file can't be read.
    let overrides = options::load().map(|s| s.overrides).unwrap_or_else(|e| {
        log.line(&format!("ignoring options: {e:#}"));
        Default::default()
    });
    let mut failures = Vec::new();
    for input in &inputs {
        let name = input.file_name().unwrap_or_default().to_string_lossy().to_string();
        if gui && preview::needs_conversion(input) {
            ui::notify("Preparing preview…", &name);
        }
        log.line(&format!("== preview {}", input.display()));
        let page = preview::view(input, &tools, &overrides, &mut log)
            .and_then(|page| if no_open { Ok(page) } else { ui::open_path(&page).map(|_| page) });
        match page {
            Ok(page) => {
                log.line(&format!("wrote {}", page.display()));
                if !gui {
                    println!("{}", page.display());
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
            "Preview 3D failed",
            &format!("{}\n\nLog: {}\n\n{}", failures.join("\n"), log.path.display(), log.tail(15)),
        );
    }
    bail!("{} of {} previews failed", failures.len(), inputs.len())
}

#[cfg(windows)]
fn attach_console() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

#[cfg(not(windows))]
fn attach_console() {}
