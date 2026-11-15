//! Logging plus the small bits of desktop UI (notifications, error dialogs,
//! format picker) used when launched from a file manager.

use anyhow::{Context, Result, bail};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn cache_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("convert3d")
}

pub struct Log {
    file: File,
    pub path: PathBuf,
    echo: bool,
}

impl Log {
    /// Opens a fresh log file and prunes old ones. With `echo`, log lines are
    /// also printed to stderr.
    pub fn open(echo: bool) -> Result<Log> {
        let dir = cache_dir().join("logs");
        fs::create_dir_all(&dir)?;
        let mut old: Vec<_> = fs::read_dir(&dir)?.flatten().map(|e| e.path()).collect();
        old.sort();
        for p in old.iter().rev().skip(30) {
            let _ = fs::remove_file(p);
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("{stamp}-{}.log", std::process::id()));
        Ok(Log { file: File::create(&path)?, path, echo })
    }

    pub fn line(&mut self, msg: &str) {
        let _ = writeln!(self.file, "{msg}");
        if self.echo {
            eprintln!("{msg}");
        }
    }

    /// A handle that child processes write their output into.
    pub fn stdio(&self) -> Result<Stdio> {
        if self.echo {
            return Ok(Stdio::inherit());
        }
        Ok(self.file.try_clone()?.into())
    }

    pub fn tail(&self, lines: usize) -> String {
        let text = fs::read_to_string(&self.path).unwrap_or_default();
        let all: Vec<_> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        all[all.len().saturating_sub(lines)..].join("\n")
    }
}

fn spawn_quiet(cmd: &mut Command) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_ok()
}

fn run_quiet(cmd: &mut Command) -> Option<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn applescript_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Shows a non-blocking desktop notification.
pub fn notify(title: &str, body: &str) {
    if cfg!(target_os = "macos") {
        let script =
            format!("display notification {} with title {}", applescript_str(body), applescript_str(title));
        spawn_quiet(Command::new("osascript").args(["-e", &script]));
    } else if cfg!(windows) {
        let ps = format!(
            "Add-Type -AssemblyName System.Windows.Forms; \
             $n = New-Object System.Windows.Forms.NotifyIcon; \
             $n.Icon = [System.Drawing.SystemIcons]::Information; $n.Visible = $true; \
             $n.ShowBalloonTip(5000, '{}', '{}', 'Info'); Start-Sleep 6; $n.Dispose()",
            title.replace('\'', "''"),
            body.replace('\'', "''")
        );
        spawn_quiet(Command::new("powershell").args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &ps,
        ]));
    } else {
        spawn_quiet(Command::new("notify-send").args([
            "-a",
            "Convert 3D",
            "-i",
            "document-export",
            title,
            body,
        ]));
    }
}

/// Shows a blocking error dialog.
pub fn error_dialog(title: &str, body: &str) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
        let w = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let (t, b) = (w(title), w(body));
        unsafe { MessageBoxW(std::ptr::null_mut(), b.as_ptr(), t.as_ptr(), MB_OK | MB_ICONERROR) };
    }
    #[cfg(target_os = "macos")]
    {
        let script =
            format!("display alert {} message {} as critical", applescript_str(title), applescript_str(body));
        let _ = Command::new("osascript").args(["-e", &script]).status();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let shown = Command::new("kdialog")
            .args(["--title", title, "--error", body])
            .status()
            .or_else(|_| Command::new("zenity").args(["--error", "--title", title, "--text", body]).status())
            .is_ok();
        if !shown {
            notify(title, body);
        }
    }
}

/// Asks the user to pick an output format. Returns the chosen extension.
pub fn pick_format(choices: &[(String, String)]) -> Option<String> {
    let labels: Vec<String> = choices.iter().map(|(ext, name)| format!("{name} (.{ext})")).collect();
    let picked = if cfg!(target_os = "macos") {
        let list = labels.iter().map(|l| applescript_str(l)).collect::<Vec<_>>().join(", ");
        let script = format!(
            "set r to choose from list {{{list}}} with title \"Convert 3D\" with prompt \"Convert to:\"\n\
             if r is false then return \"\"\nreturn item 1 of r"
        );
        run_quiet(Command::new("osascript").args(["-e", &script]))
    } else if cfg!(windows) {
        None
    } else {
        let mut kd = Command::new("kdialog");
        kd.args(["--title", "Convert 3D", "--combobox", "Convert to:"]).args(&labels);
        run_quiet(&mut kd).or_else(|| {
            let mut z = Command::new("zenity");
            z.args(["--list", "--title=Convert 3D", "--column=Convert to"]).args(&labels);
            run_quiet(&mut z)
        })
    }?;
    let i = labels.iter().position(|l| *l == picked)?;
    Some(choices[i].0.clone())
}

pub struct PlaceRequest {
    pub ask_folder: bool,
    pub ask_conflict: bool,
    pub folder: PathBuf,
    pub jobs: Vec<Vec<String>>,
}

pub struct PlaceChoice {
    pub folder: PathBuf,
    pub overwrite: bool,
}

/// One window before conversion. It asks for a folder, whether to overwrite a
/// name that already exists, or both. `None` means the user cancelled.
pub fn place_dialog(req: &PlaceRequest) -> Result<Option<PlaceChoice>> {
    let dir = tempfile::tempdir().context("could not create a temporary directory")?;
    let input = dir.path().join("place-in.json");
    let output = dir.path().join("place-out.json");
    let jobs: Vec<_> = req.jobs.iter().map(|names| serde_json::json!({ "names": names })).collect();
    let model = serde_json::json!({
        "ask_folder": req.ask_folder,
        "ask_conflict": req.ask_conflict,
        "folder": req.folder.display().to_string(),
        "jobs": jobs,
    });
    fs::write(&input, serde_json::to_vec_pretty(&model)?)?;

    let output_bytes = if cfg!(windows) {
        let script = dir.path().join("place_dialog.ps1");
        fs::write(&script, include_str!("../scripts/place_dialog.ps1"))?;
        let mut cmd = gui_command("powershell.exe");
        cmd.args(["-NoProfile", "-STA", "-WindowStyle", "Hidden", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&script)
            .arg("-InputPath")
            .arg(&input)
            .arg("-OutputPath")
            .arg(&output);
        cmd.output().context("could not open the conversion window")?
    } else {
        let script = dir.path().join("place_dialog.py");
        fs::write(&script, include_str!("../scripts/place_dialog.py"))?;
        let python = ["python3", "python"]
            .into_iter()
            .find(|bin| Command::new(bin).arg("--version").output().is_ok_and(|out| out.status.success()));
        let Some(python) = python else {
            bail!("python3 with tkinter is required to confirm where to save");
        };
        let mut cmd = gui_command(python);
        cmd.arg(&script).arg(&input).arg(&output);
        cmd.output().context("could not open the conversion window")?
    };

    if output.is_file() {
        let text = fs::read_to_string(&output)?;
        let value: serde_json::Value =
            serde_json::from_str(&text).context("could not read the conversion window result")?;
        let folder = PathBuf::from(value.get("folder").and_then(|v| v.as_str()).unwrap_or(""));
        if req.ask_folder && (folder.as_os_str().is_empty() || folder.is_file()) {
            bail!("choose a folder");
        }
        let overwrite = value.get("overwrite").and_then(|v| v.as_bool()).unwrap_or(false);
        return Ok(Some(PlaceChoice { folder, overwrite }));
    }
    if output_bytes.status.code() == Some(2) {
        return Ok(None);
    }
    let detail = String::from_utf8_lossy(&output_bytes.stderr);
    let detail = detail.trim();
    if detail.is_empty() {
        bail!("conversion window failed ({})", output_bytes.status);
    }
    bail!("conversion window failed ({})", detail);
}

/// Opens the options window. `None` means the user cancelled.
pub fn edit_options(model: &serde_json::Value) -> Result<Option<serde_json::Value>> {
    let dir = tempfile::tempdir().context("could not create a temporary directory")?;
    let input = dir.path().join("options-in.json");
    let output = dir.path().join("options-out.json");
    fs::write(&input, serde_json::to_vec_pretty(model)?)?;

    let output_bytes = if cfg!(windows) {
        let script = dir.path().join("options_dialog.ps1");
        fs::write(&script, include_str!("../scripts/options_dialog.ps1"))?;
        let mut cmd = gui_command("powershell.exe");
        cmd.args(["-NoProfile", "-STA", "-WindowStyle", "Hidden", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&script)
            .arg("-InputPath")
            .arg(&input)
            .arg("-OutputPath")
            .arg(&output);
        cmd.output().context("could not open the options window")?
    } else {
        let script = dir.path().join("options_dialog.py");
        fs::write(&script, include_str!("../scripts/options_dialog.py"))?;
        let python = ["python3", "python"]
            .into_iter()
            .find(|bin| Command::new(bin).arg("--version").output().is_ok_and(|out| out.status.success()));
        let Some(python) = python else {
            bail!("python3 with tkinter is required to open Convert 3D Options");
        };
        let mut cmd = gui_command(python);
        cmd.arg(&script).arg(&input).arg(&output);
        cmd.output().context("could not open the options window")?
    };

    if output.is_file() {
        let text = fs::read_to_string(&output)?;
        let value = serde_json::from_str(&text).context("could not read the options window result")?;
        return Ok(Some(value));
    }
    if output_bytes.status.code() == Some(2) {
        return Ok(None);
    }
    let detail = String::from_utf8_lossy(&output_bytes.stderr);
    let detail = detail.trim();
    if detail.is_empty() {
        bail!("options window failed ({})", output_bytes.status);
    }
    bail!("options window failed ({})", detail);
}

fn gui_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.stdin(Stdio::null());
    cmd
}
