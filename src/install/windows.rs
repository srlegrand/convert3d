//! Explorer cascading context menu, registered per-user under HKCU (no admin
//! needed). On Windows 11 it appears under "Show more options".

use super::MenuSpec;
use anyhow::Result;
use winreg::RegKey;
use winreg::enums::HKEY_CURRENT_USER;

const CLASSES: &str = r"Software\Classes";
const MENU_KEY: &str = "Convert3D.Menu";
const VERB: &str = "Convert3D";

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

    for ext in &spec.inputs {
        let (verb, _) =
            hkcu.create_subkey(format!(r"{CLASSES}\SystemFileAssociations\.{ext}\shell\{VERB}"))?;
        verb.set_value("MUIVerb", &"Convert 3D to")?;
        verb.set_value("Icon", &format!("\"{exe}\",0"))?;
        verb.set_value("ExtendedSubCommandsKey", &MENU_KEY)?;
        // Run once per selected file, even when many are selected.
        verb.set_value("MultiSelectModel", &"Player")?;
    }
    refresh_shell();
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey_all(format!(r"{CLASSES}\{MENU_KEY}"));
    if let Ok(assoc) = hkcu.open_subkey(format!(r"{CLASSES}\SystemFileAssociations")) {
        for ext in assoc.enum_keys().flatten() {
            let _ = assoc.delete_subkey_all(format!(r"{ext}\shell\{VERB}"));
        }
    }
    refresh_shell();
    Ok(())
}

fn refresh_shell() {
    use windows_sys::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED as i32, SHCNF_IDLIST, std::ptr::null(), std::ptr::null()) };
}
