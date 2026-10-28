//! KDE Dolphin service menu, Nautilus scripts, and MIME types for 3D
//! extensions the shared-mime-info database doesn't know.

use super::{MenuSpec, write_file};
use anyhow::Result;
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const MIME_FILE: &str = "mime/packages/convert3d.xml";
const KDE_FILE: &str = "kio/servicemenus/convert3d.desktop";
const NAUTILUS_DIR: &str = "nautilus/scripts/Convert 3D to";

fn data_dir() -> PathBuf {
    dirs::data_dir().expect("no XDG data dir")
}

/// MIME types (from the system and user databases) whose glob matches `*.ext`.
fn known_mimes(ext: &str) -> Vec<String> {
    let mut dirs = vec![data_dir().join("mime")];
    let xdg = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    dirs.extend(xdg.split(':').map(|d| Path::new(d).join("mime")));
    let pattern = format!("*.{ext}");
    let mut out = BTreeSet::new();
    for d in dirs {
        for line in fs::read_to_string(d.join("globs2")).unwrap_or_default().lines() {
            let mut parts = line.split(':');
            if let (Some(_weight), Some(mime), Some(glob)) = (parts.next(), parts.next(), parts.next())
                && glob.eq_ignore_ascii_case(&pattern)
                && !mime.starts_with("model/x-convert3d")
            {
                out.insert(mime.to_string());
            }
        }
    }
    out.into_iter().collect()
}

fn is_3d_mime(mime: &str) -> bool {
    mime.starts_with("model/")
        || [
            "application/x-blender",
            "application/zip",
            "image/x-3ds",
            "image/x-lwo",
            "image/x-lws",
            "image/vnd.dxf",
        ]
        .contains(&mime)
}

/// Our own MIME definition for an extension that has no 3D type yet. When
/// the extension belongs to something else (`.abc` music notation, `.smd`
/// Genesis ROMs) a higher glob weight makes the 3D meaning win.
fn own_mime(ext: &str, known: &[String]) -> Option<(String, u32)> {
    if known.iter().any(|m| is_3d_mime(m)) {
        return None;
    }
    let weight = if known.is_empty() { 50 } else { 80 };
    Some((format!("model/x-convert3d-{ext}"), weight))
}

pub fn install(spec: &MenuSpec) -> Result<()> {
    let exe = spec.exe.display().to_string();

    let mut mimes = BTreeSet::new();
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <mime-info xmlns=\"http://www.freedesktop.org/standards/shared-mime-info\">\n",
    );
    for &ext in &spec.inputs {
        let known = known_mimes(ext);
        if let Some((mime, weight)) = own_mime(ext, &known) {
            xml += &format!(
                "  <mime-type type=\"{mime}\">\n    <comment>{} file</comment>\n    \
                 <glob pattern=\"*.{ext}\" weight=\"{weight}\"/>\n  </mime-type>\n",
                crate::formats::get(ext).map_or(ext, |f| f.name)
            );
            mimes.insert(mime);
        }
        mimes.extend(known.into_iter().filter(|m| is_3d_mime(m)));
    }
    xml += "</mime-info>\n";
    write_file(&data_dir().join(MIME_FILE), &xml)?;
    let _ = Command::new("update-mime-database").arg(data_dir().join("mime")).status();

    // KDE / Dolphin
    let actions: Vec<String> = spec.targets.iter().map(|f| format!("to_{}", f.ext)).collect();
    let mut desktop = format!(
        "[Desktop Entry]\nType=Service\nServiceTypes=KonqPopupMenu/Plugin\n\
         MimeType={};\nActions={};\nX-KDE-Submenu=Convert 3D to\nX-KDE-Priority=TopLevel\n\
         Icon=document-export\n",
        mimes.iter().cloned().collect::<Vec<_>>().join(";"),
        actions.join(";")
    );
    for f in &spec.targets {
        desktop += &format!(
            "\n[Desktop Action to_{ext}]\nName={name} (.{ext})\nIcon=document-export\n\
             Exec=\"{exe}\" convert --gui --to {ext} %F\n",
            ext = f.ext,
            name = f.name
        );
    }
    let kde = data_dir().join(KDE_FILE);
    write_file(&kde, &desktop)?;
    fs::set_permissions(&kde, fs::Permissions::from_mode(0o755))?;
    // Refresh KDE's cache of MIME types and service menus.
    for tool in ["kbuildsycoca6", "kbuildsycoca5"] {
        if Command::new(tool).status().is_ok() {
            break;
        }
    }

    // GNOME Files only shows scripts for every file type, so only add them
    // where Nautilus is in use.
    if which_nautilus() {
        let dir = data_dir().join(NAUTILUS_DIR);
        let _ = fs::remove_dir_all(&dir);
        for f in &spec.targets {
            let script = dir.join(format!("{} (.{})", f.name, f.ext));
            write_file(&script, &format!("#!/bin/sh\nexec \"{exe}\" convert --gui --to {} \"$@\"\n", f.ext))?;
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(())
}

fn which_nautilus() -> bool {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).any(|d| d.join("nautilus").is_file())
}

pub fn uninstall() -> Result<()> {
    let _ = fs::remove_file(data_dir().join(KDE_FILE));
    let _ = fs::remove_dir_all(data_dir().join(NAUTILUS_DIR));
    if fs::remove_file(data_dir().join(MIME_FILE)).is_ok() {
        let _ = Command::new("update-mime-database").arg(data_dir().join("mime")).status();
    }
    Ok(())
}
