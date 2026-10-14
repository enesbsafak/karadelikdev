//! Kullanıcı PATH'i (HKCU\Environment) ve Explorer sağ tık menüsü (HKCU\Software\Classes).
//! İkisi de yönetici izni gerektirmez.

use crate::kcore::paths;
use crate::i18n::t;
use anyhow::Result;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_EXPAND_SZ};
use winreg::{RegKey, RegValue};

fn env_key() -> Result<RegKey> {
    Ok(RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)?)
}

pub fn user_path() -> Vec<String> {
    env_key()
        .and_then(|k| Ok(k.get_value::<String, _>("Path")?))
        .unwrap_or_default()
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn set_user_path(parts: &[String]) -> Result<()> {
    let joined = parts.join(";");
    let mut bytes: Vec<u8> = joined.encode_utf16().chain(std::iter::once(0)).flat_map(|u| u.to_le_bytes()).collect();
    bytes.shrink_to_fit();
    env_key()?.set_raw_value("Path", &RegValue { bytes: bytes.into(), vtype: REG_EXPAND_SZ })?;
    broadcast_env_change();
    Ok(())
}

fn ours(entry: &str) -> bool {
    let root = paths().root.to_string_lossy().to_lowercase();
    entry.to_lowercase().starts_with(&root)
}

/// KaradelikDev dizinlerini kullanıcı PATH'ine ekler (eski KaradelikDev girdileri değiştirilir).
pub fn add_to_path() -> Result<()> {
    let mut parts: Vec<String> = user_path().into_iter().filter(|p| !ours(p)).collect();
    let mut add: Vec<String> = crate::terminal::path_dirs().iter().map(|d| d.display().to_string()).collect();
    add.append(&mut parts);
    set_user_path(&add)
}

pub fn remove_from_path() -> Result<()> {
    let parts: Vec<String> = user_path().into_iter().filter(|p| !ours(p)).collect();
    set_user_path(&parts)
}

pub fn path_report() -> String {
    let mut s = String::new();
    for p in user_path() {
        let mark = if ours(&p) { "★ " } else { "  " };
        s.push_str(&format!("{mark}{p}\n"));
    }
    s
}

fn broadcast_env_change() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE};
    let env: Vec<u16> = "Environment".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut res = 0usize;
        SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, 0, env.as_ptr() as isize, SMTO_ABORTIFHUNG, 3000, &mut res);
    }
}

const SHELL_KEYS: &[&str] = &[r"Software\Classes\Directory\shell", r"Software\Classes\Directory\Background\shell"];

/// "KaradelikDev Terminal" ve (editör ayarlıysa) "Editörde aç" sağ tık girdileri.
pub fn add_context_menu() -> Result<()> {
    let exe = std::env::current_exe()?;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    for base in SHELL_KEYS {
        let arg = if base.contains("Background") { "%V" } else { "%1" };
        let (k, _) = hkcu.create_subkey(format!(r"{base}\KaradelikTerminal"))?;
        k.set_value("", &t("ctx.terminal"))?;
        k.set_value("Icon", &exe.display().to_string())?;
        let (cmd, _) = k.create_subkey("command")?;
        cmd.set_value("", &format!("\"{}\" --terminal \"{arg}\"", exe.display()))?;
        let editor = crate::kcore::settings().editor;
        let editor = if editor.trim().is_empty() {
            crate::versions::tool("notepad++", "notepad++.exe").map(|p| p.display().to_string()).unwrap_or_default()
        } else {
            editor
        };
        if !editor.is_empty() && !base.contains("Background") {
            let (k, _) = hkcu.create_subkey(format!(r"{base}\KaradelikEditor"))?;
            k.set_value("", &t("ctx.editor"))?;
            k.set_value("Icon", &editor)?;
            let (cmd, _) = k.create_subkey("command")?;
            cmd.set_value("", &format!("\"{editor}\" \"{arg}\""))?;
        }
    }
    Ok(())
}

pub fn remove_context_menu() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    for base in SHELL_KEYS {
        for name in ["KaradelikTerminal", "KaradelikEditor"] {
            let _ = hkcu.delete_subkey_all(format!(r"{base}\{name}"));
        }
    }
    Ok(())
}

pub fn context_menu_installed() -> bool {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey(format!(r"{}\KaradelikTerminal", SHELL_KEYS[0])).is_ok()
}
