//! macOS uygulaması.
//!
//! DURUM: Windows'ta yazıldı, macOS'ta henüz derlenmedi ve test edilmedi. Mac'teki oturum ilk iş olarak
//! `cargo check` ile derleyip her işlevi gerçek ortamda doğrulamalı (bkz. docs/macos-port.md).

use crate::util::run_capture;
use anyhow::{anyhow, bail, Context, Result};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

pub const EXE_SUFFIX: &str = "";

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Kurulumlu sürümde veri klasörünün varsayılanı (boşluksuz; Apache/MySQL boşluklu yollarla sorun çıkarır).
pub fn default_data_root() -> PathBuf {
    home().join("KaradelikDev")
}

/// macOS'ta arka plan süreçleri zaten pencere açmaz.
pub fn hidden(program: impl AsRef<OsStr>) -> Command {
    Command::new(program)
}

pub fn shell(cmdline: &str) -> Command {
    let mut c = Command::new("/bin/sh");
    c.arg("-c").arg(cmdline);
    c
}

/// AppleScript dizgesi için kaçış.
fn osa_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Terminal.app'te yeni pencerede komut çalıştırır. Dönen süreç osascript'tir (pencere ayrı yaşar).
pub fn open_console(title: &str, cwd: &Path, cmdline: &str, keep_open: bool) -> Result<Child> {
    let q = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let mut script = format!("printf '\\033]0;%s\\007' {}; cd {} && {cmdline}", q(title), q(&cwd.display().to_string()));
    if !keep_open {
        script.push_str("; exit");
    }
    Command::new("osascript")
        .args(["-e", &format!("tell application \"Terminal\" to do script {}", osa_quote(&script))])
        .args(["-e", "tell application \"Terminal\" to activate"])
        .spawn()
        .context("osascript")
}

/// `lsof -nP -iTCP -sTCP:LISTEN -F pn` çıktısından port -> PID.
pub fn listeners() -> HashMap<u16, u32> {
    let mut map = HashMap::new();
    let mut c = hidden("lsof");
    c.args(["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "pn"]);
    if let Ok((_, out)) = run_capture(c) {
        parse_lsof(&out, &mut map);
    }
    map
}

fn parse_lsof(out: &str, map: &mut HashMap<u16, u32>) {
    let mut pid = 0u32;
    for line in out.lines() {
        if let Some(p) = line.strip_prefix('p') {
            pid = p.parse().unwrap_or(0);
        } else if let Some(n) = line.strip_prefix('n') {
            // n127.0.0.1:80  n*:3306  n[::1]:80
            if let Some(port) = n.rsplit(':').next().and_then(|p| p.parse().ok()) {
                if pid != 0 {
                    map.entry(port).or_insert(pid);
                }
            }
        }
    }
}

/// Önce alt süreçleri, sonra kendisini sonlandırır (SIGTERM, ardından SIGKILL).
pub fn kill_tree(pid: u32) {
    let p = pid.to_string();
    let _ = run_capture({
        let mut c = hidden("pkill");
        c.args(["-TERM", "-P", &p]);
        c
    });
    let _ = run_capture({
        let mut c = hidden("kill");
        c.args(["-TERM", &p]);
        c
    });
    std::thread::sleep(std::time::Duration::from_millis(500));
    let _ = run_capture({
        let mut c = hidden("kill");
        c.args(["-KILL", &p]);
        c
    });
}

pub fn which(name: &str) -> Option<PathBuf> {
    let mut c = hidden("/usr/bin/which");
    c.arg(name);
    run_capture(c).ok().and_then(|(code, out)| (code == 0).then(|| PathBuf::from(out.lines().next()?.trim())))
}

/// Yönetici olarak çalıştırır: macOS parola penceresi (`do shell script ... with administrator privileges`).
pub fn run_elevated(program: &Path, args: &str, wait: bool) -> Result<()> {
    let code = run_elevated_code(program, args)?;
    if wait && code != 0 {
        bail!("Yönetici işlemi {code} koduyla bitti");
    }
    Ok(())
}

pub fn run_elevated_code(program: &Path, args: &str) -> Result<u32> {
    let cmd = format!("'{}' {args}", program.display().to_string().replace('\'', "'\\''"));
    let script = format!("do shell script {} with administrator privileges", osa_quote(&cmd));
    let (code, out) = run_capture({
        let mut c = hidden("osascript");
        c.args(["-e", &script]);
        c
    })?;
    // -128: kullanıcı parola penceresini iptal etti
    if out.contains("-128") {
        return Err(anyhow!("Yönetici izni verilmedi"));
    }
    Ok(code as u32)
}

// ---------------------------------------------------------------- yapılandırma (~/Library/Application Support/KaradelikDev/config.json)

fn config_file() -> PathBuf {
    home().join("Library").join("Application Support").join("KaradelikDev").join("config.json")
}

fn config_all() -> serde_json::Map<String, serde_json::Value> {
    std::fs::read_to_string(config_file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn config_get(name: &str) -> Option<String> {
    config_all().get(name).and_then(|v| v.as_str()).map(str::to_string).filter(|v| !v.trim().is_empty())
}

pub fn config_set(name: &str, value: &str) -> Result<()> {
    let mut all = config_all();
    all.insert(name.to_string(), serde_json::Value::String(value.to_string()));
    let f = config_file();
    std::fs::create_dir_all(f.parent().unwrap())?;
    std::fs::write(f, serde_json::to_string_pretty(&all)?)?;
    Ok(())
}

/// `.app` paketi içinden çalışıyorsa (ör. /Applications/KaradelikDev.app/Contents/MacOS) o klasör kurulumdur.
pub fn install_dirs() -> Vec<PathBuf> {
    let exe = std::env::current_exe().unwrap_or_default();
    let dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    if dir.ends_with("Contents/MacOS") && dir.to_string_lossy().contains(".app/") {
        vec![dir]
    } else {
        vec![]
    }
}

pub fn hosts_path() -> PathBuf {
    PathBuf::from("/etc/hosts")
}

pub fn attach_console() {}

pub fn protect_std_handles() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsof_parse() {
        let out = "p101\nn*:80\nn[::1]:80\np202\nn127.0.0.1:3306\n";
        let mut m = HashMap::new();
        parse_lsof(out, &mut m);
        assert_eq!(m.get(&80), Some(&101));
        assert_eq!(m.get(&3306), Some(&202));
    }
}
