//! Windows uygulaması: konsolsuz süreçler, netstat/taskkill, UAC, kayıt defteri, konsol tutamaçları.

use crate::util::run_capture;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

pub const EXE_SUFFIX: &str = ".exe";
/// PATH ortam değişkeninde dizin ayırıcı.
pub const PATH_SEP: &str = ";";
/// Ürettiğimiz metin dosyalarının satır sonu.
pub const LINE_END: &str = "\r\n";
/// Kurulumlu sürümde veri klasörünün varsayılanı: kısa ve boşluksuz (Apache/MySQL boşluklu yollarla sorun çıkarır).
pub fn default_data_root() -> PathBuf {
    PathBuf::from(r"C:\karadelik")
}

/// Komut kabuğunda bir satır çalıştıran gizli komut (`cmd /s /c "..."`). `/s`: cmd yalnızca en dıştaki
/// tırnak çiftini kaldırır, içteki tırnaklar korunur.
pub fn shell(cmdline: &str) -> Command {
    let mut c = hidden("cmd.exe");
    c.raw_arg(format!("/s /c \"{cmdline}\""));
    c
}

// ---------------------------------------------------------------- yapılandırma (HKCU/HKLM\Software\KaradelikDev)

/// Setup'ın yazdığı ve uygulamanın okuduğu kayıt anahtarı.
const REG_KEY: &str = r"Software\KaradelikDev";

fn reg_get_in(hive: winreg::HKEY, name: &str) -> Option<String> {
    winreg::RegKey::predef(hive).open_subkey(REG_KEY).ok()?.get_value::<String, _>(name).ok().filter(|v| !v.trim().is_empty())
}

/// Kullanıcıya ait ayar (ör. `DataRoot`).
pub fn config_get(name: &str) -> Option<String> {
    reg_get_in(winreg::enums::HKEY_CURRENT_USER, name)
}

pub fn config_set(name: &str, value: &str) -> Result<()> {
    let (key, _) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).create_subkey(REG_KEY)?;
    key.set_value(name, &value.to_string())?;
    Ok(())
}

/// Setup'ın kurduğu klasörler (NSIS kancası yazar): kurulumu yapan kullanıcıda HKCU, bilgisayar genelinde HKLM.
pub fn install_dirs() -> Vec<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE].into_iter().filter_map(|h| reg_get_in(h, "InstallDir")).map(PathBuf::from).collect()
}

pub fn hosts_path() -> PathBuf {
    let win = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    Path::new(&win).join(r"System32\drivers\etc\hosts")
}

// ---------------------------------------------------------------- konsol tutamaçları (cli)

pub fn attach_console() {
    use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

/// Kendi std tutamaçlarımızı devredilemez yap: aksi halde başlattığımız httpd/mysqld bunları miras
/// alır ve bizi çağıran betiğin borusu (pipe) servisler kapanana kadar açık kalır.
pub fn protect_std_handles() {
    use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE_FLAG_INHERIT};
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
    unsafe {
        for which in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let h = GetStdHandle(which);
            if !h.is_null() && h as isize != -1 {
                SetHandleInformation(h, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
}

// ---------------------------------------------------------------- süreçler

pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
pub const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

/// Konsol penceresi açmayan bir komut.
pub fn hidden(program: impl AsRef<OsStr>) -> Command {
    let mut c = Command::new(program);
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

/// Görünür bir cmd penceresinde komut satırı çalıştırır (`cmd /k`).
pub fn open_console(title: &str, cwd: &Path, cmdline: &str, keep_open: bool) -> Result<Child> {
    let mut c = Command::new("cmd.exe");
    c.creation_flags(CREATE_NEW_CONSOLE);
    c.current_dir(cwd);
    let sw = if keep_open { "/k" } else { "/c" };
    // /s: cmd yalnızca en dıştaki tırnak çiftini kaldırır, içteki tırnaklar korunur
    c.raw_arg(format!("/s {sw} \"title {title} & {cmdline}\""));
    c.spawn().context("cmd.exe başlatılamadı")
}

/// `netstat -ano` çıktısından dinlenen TCP portu -> PID haritası.
pub fn listeners() -> HashMap<u16, u32> {
    let mut map = HashMap::new();
    let mut c = hidden("netstat");
    c.args(["-ano", "-p", "TCP"]);
    if let Ok((_, out)) = run_capture(c) {
        parse_listeners(&out, &mut map);
    }
    let mut c6 = hidden("netstat");
    c6.args(["-ano", "-p", "TCPv6"]);
    if let Ok((_, out)) = run_capture(c6) {
        parse_listeners(&out, &mut map);
    }
    map
}

fn parse_listeners(out: &str, map: &mut HashMap<u16, u32>) {
    for line in out.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        // Proto  Local  Foreign  State  PID  (durum dile göre çevrilebilir, sütun konumuna bakılır)
        if cols.len() == 5 && cols[0].starts_with("TCP") && (cols[2].ends_with(":0") || cols[2].ends_with("]:0")) {
            if let (Some(port), Ok(pid)) = (cols[1].rsplit(':').next().and_then(|p| p.parse().ok()), cols[4].parse()) {
                map.entry(port).or_insert(pid);
            }
        }
    }
}

pub fn kill_tree(pid: u32) {
    let mut c = hidden("taskkill");
    c.args(["/T", "/F", "/PID", &pid.to_string()]);
    let _ = run_capture(c);
}

pub fn which(name: &str) -> Option<PathBuf> {
    let mut c = hidden("where");
    c.arg(name);
    run_capture(c).ok().and_then(|(code, out)| {
        if code == 0 {
            out.lines().next().map(|l| PathBuf::from(l.trim()))
        } else {
            None
        }
    })
}

/// Yönetici olarak çalıştırır (UAC) ve bitmesini bekler. Kullanıcı reddederse hata döner.
pub fn run_elevated(program: &Path, args: &str, wait: bool) -> Result<()> {
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    let w = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let verb = w("runas");
    let file = w(&program.to_string_lossy());
    let params = w(args);
    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = SW_HIDE;
        if ShellExecuteExW(&mut info) == 0 {
            return Err(anyhow!("Yönetici izni verilmedi"));
        }
        if !info.hProcess.is_null() {
            if wait {
                WaitForSingleObject(info.hProcess, INFINITE);
                let mut code: u32 = 0;
                GetExitCodeProcess(info.hProcess, &mut code);
                CloseHandle(info.hProcess);
                if code != 0 {
                    return Err(anyhow!("Yönetici işlemi {code} koduyla bitti"));
                }
            } else {
                CloseHandle(info.hProcess);
            }
        }
    }
    Ok(())
}

/// Yönetici olarak çalıştırır, bitmesini bekler ve çıkış kodunu döndürür.
pub fn run_elevated_code(program: &Path, args: &str) -> Result<u32> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;
    let w = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (verb, file, params) = (w("runas"), w(&program.to_string_lossy()), w(args));
    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = SW_HIDE;
        if ShellExecuteExW(&mut info) == 0 || info.hProcess.is_null() {
            return Err(anyhow!("Yönetici izni verilmedi"));
        }
        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code: u32 = 0;
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
        Ok(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listeners_parse() {
        let out = "  TCP    0.0.0.0:80             0.0.0.0:0              LISTENING       1234\n  TCP    127.0.0.1:3306         0.0.0.0:0              DINLEME       55\n  TCP    127.0.0.1:50000        127.0.0.1:80           ESTABLISHED     9\n  TCP    [::]:443               [::]:0                 LISTENING       77";
        let mut m = HashMap::new();
        parse_listeners(out, &mut m);
        assert_eq!(m.get(&80), Some(&1234));
        assert_eq!(m.get(&3306), Some(&55));
        assert_eq!(m.get(&443), Some(&77));
        assert_eq!(m.get(&50000), None);
    }
}
