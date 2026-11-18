//! Explorer context menu entries, registered per-user under HKCU (no admin
//! needed): a "Convert 3D to" submenu and "Preview 3D". On Windows 11 they
//! appear under "Show more options".

use super::MenuSpec;
use anyhow::Result;
use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

const CLASSES: &str = r"Software\Classes";
const MENU_KEY: &str = "Convert3D.Menu";
const VERB: &str = "Convert3D";
const PREVIEW_VERB: &str = "Preview3D";

pub fn install(spec: &MenuSpec) -> Result<()> {
    uninstall()?;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let exe = spec.exe.display().to_string();

    // The shared submenu. Explorer sorts entries by key name, hence the index prefix.
    for (i, f) in spec.targets.iter().enumerate() {
        let (item, _) = hkcu.create_subkey(format!(r"{CLASSES}\{MENU_KEY}\shell\{i:02}_{}", f.ext))?;
        item.set_value("MUIVerb", &format!("{} (.{})", f.name, f.ext))?;
        let (cmd, _) = item.create_subkey("command")?;
        cmd.set_value("", &format!("\"{exe}\" convert --gui --to {} \"%1\"", f.ext))?;
    }

    let (item, _) = hkcu.create_subkey(format!(r"{CLASSES}\{MENU_KEY}\shell\99_options"))?;
    item.set_value("MUIVerb", &"Options")?;
    // Draw a separator above this entry.
    item.set_value("CommandFlags", &0x20u32)?;
    let (cmd, _) = item.create_subkey("command")?;
    cmd.set_value("", &format!("\"{exe}\" options"))?;

    for ext in &spec.inputs {
        let (verb, _) =
            hkcu.create_subkey(format!(r"{CLASSES}\SystemFileAssociations\.{ext}\shell\{VERB}"))?;
        verb.set_value("MUIVerb", &"Convert 3D to")?;
        verb.set_value("Icon", &format!("\"{exe}\",0"))?;
        verb.set_value("ExtendedSubCommandsKey", &MENU_KEY)?;
        // Run once per selected file, even when many are selected.
        verb.set_value("MultiSelectModel", &"Player")?;

        let (preview, _) =
            hkcu.create_subkey(format!(r"{CLASSES}\SystemFileAssociations\.{ext}\shell\{PREVIEW_VERB}"))?;
        preview.set_value("MUIVerb", &"Preview 3D")?;
        preview.set_value("Icon", &format!("\"{exe}\",0"))?;
        preview.set_value("MultiSelectModel", &"Player")?;
        let (cmd, _) = preview.create_subkey("command")?;
        cmd.set_value("", &format!("\"{exe}\" view --gui \"%1\""))?;
    }
    refresh_shell();
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey_all(format!(r"{CLASSES}\{MENU_KEY}"));
    if let Ok(assoc) = hkcu.open_subkey(format!(r"{CLASSES}\SystemFileAssociations")) {
        for ext in assoc.enum_keys().flatten() {
            for verb in [VERB, PREVIEW_VERB] {
                let _ = assoc.delete_subkey_all(format!(r"{ext}\shell\{verb}"));
            }
        }
    }
    refresh_shell();
    Ok(())
}

fn refresh_shell() {
    use windows_sys::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED as i32, SHCNF_IDLIST, std::ptr::null(), std::ptr::null()) };
}
