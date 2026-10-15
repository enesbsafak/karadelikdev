//! macOS uygulaması: lsof/kill, osascript ile yönetici izni, Terminal/iTerm (`.command` dosyası), ayarlar
//! `~/Library/Application Support/KaradelikDev/config.json`, kurulum tespiti `.app/Contents/MacOS`, Homebrew.

use super::sh_quote;
use crate::util::run_capture;
use anyhow::{anyhow, bail, Context, Result};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

pub const EXE_SUFFIX: &str = "";
/// PATH ortam değişkeninde dizin ayırıcı.
pub const PATH_SEP: &str = ":";
/// Ürettiğimiz metin dosyalarının satır sonu.
pub const LINE_END: &str = "\n";

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Kurulumlu sürümde veri klasörünün varsayılanı (boşluksuz; Apache/MySQL boşluklu yollarla sorun çıkarır).
pub fn default_data_root() -> PathBuf {
    home().join("KaradelikDev")
}

/// Arka plan komutu. macOS'ta pencere açmaz; her süreç kendi süreç grubunda başlar: Apache (prefork) kapanırken
/// `killpg(getpgrp())` ile kendi grubuna SIGTERM gönderir, aynı grupta kalsaydı KaradelikDev'i ve diğer servisleri
/// de kapatırdı. Ayrı grup ayrıca terminalden açılan uygulamaya gelen Ctrl+C'nin servislere yayılmasını önler.
pub fn hidden(program: impl AsRef<OsStr>) -> Command {
    use std::os::unix::process::CommandExt;
    let mut c = Command::new(program);
    c.process_group(0);
    c
}

pub fn shell(cmdline: &str) -> Command {
    let mut c = hidden("/bin/sh");
    c.arg("-c").arg(cmdline);
    c
}

/// AppleScript dizgesi için kaçış.
fn osa_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Finder'dan açılan uygulamanın PATH'i yalnız `/usr/bin:/bin:/usr/sbin:/sbin` olur; Homebrew ve kullanıcı
/// araçları (`which cloudflared`, `code`) bulunabilsin diye yaygın dizinler eklenir.
pub fn extend_gui_path() {
    let cur = std::env::var("PATH").unwrap_or_default();
    let mut parts: Vec<String> = cur.split(':').filter(|s| !s.is_empty()).map(str::to_string).collect();
    let extra = ["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin", "/usr/local/sbin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"];
    for d in extra {
        if !parts.iter().any(|p| p == d) && Path::new(d).is_dir() {
            parts.push(d.to_string());
        }
    }
    std::env::set_var("PATH", parts.join(":"));
}

// ---------------------------------------------------------------- Homebrew

/// Homebrew öneki: Apple Silicon'da /opt/homebrew, Intel'de /usr/local.
pub fn brew_prefix() -> Option<PathBuf> {
    ["/opt/homebrew", "/usr/local"].iter().map(PathBuf::from).find(|p| p.join("bin").join("brew").is_file())
}

pub fn brew() -> Option<PathBuf> {
    brew_prefix().map(|p| p.join("bin").join("brew"))
}

/// Homebrew'un resmi kurulum komutu (Terminal'de çalışır; parola ister).
pub const BREW_INSTALL: &str =
    "/bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"";

/// Xcode komut satırı araçları kurulu mu?
pub fn clt_installed() -> bool {
    let mut c = hidden("/usr/bin/xcode-select");
    c.arg("-p");
    run_capture(c).map(|(code, out)| code == 0 && Path::new(out.trim()).is_dir()).unwrap_or(false)
}

/// Gerçek (bağlantıları çözülmüş) yolun Homebrew Cellar'ı içindeki önek kısmı: `/opt/homebrew/Cellar/x/1.0/bin/y` -> `/opt/homebrew`.
pub fn cellar_prefix(path: &Path) -> Option<PathBuf> {
    let real = std::fs::canonicalize(path).ok()?;
    let s = real.to_string_lossy().to_string();
    let i = s.find("/Cellar/")?;
    Some(PathBuf::from(&s[..i]))
}

// ---------------------------------------------------------------- terminal

/// `tmp/terminal` altındaki eski betikleri siler (Terminal okuduktan sonra gereksizdir).
fn cleanup_scripts(dir: &Path) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let old = e.metadata().and_then(|m| m.modified()).map(|t| t.elapsed().unwrap_or_default().as_secs() > 3600).unwrap_or(false);
            if old {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

fn scripts_dir() -> PathBuf {
    crate::kcore::paths().tmp().join("terminal")
}

/// Seçili terminal uygulaması: "terminal" (Terminal.app) veya "iterm".
pub fn terminal_app() -> &'static str {
    let t = crate::kcore::settings().terminal;
    let iterm = Path::new("/Applications/iTerm.app").exists();
    match t.as_str() {
        "iterm" if iterm => "iTerm",
        "terminal" | "iterm" => "Terminal",
        _ if iterm => "iTerm",
        _ => "Terminal",
    }
}

/// Kullanıcının kabuğunu KaradelikDev ortamıyla etkileşimli açan satır. Kabuk kendi başlangıç dosyalarını okur,
/// sonra ortam dosyası yüklenir; böylece PATH'te önce seçili sürümler gelir (ör. Herd/Homebrew PHP'sinden önce).
fn interactive_shell(env_file: Option<&Path>, id: &str) -> Result<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let Some(env) = env_file else { return Ok(format!("exec {}", sh_quote(&shell))) };
    let src = format!(". {}", sh_quote(&env.display().to_string()));
    let dir = scripts_dir();
    if shell.ends_with("zsh") {
        let z = dir.join(format!("{id}.zdot"));
        std::fs::create_dir_all(&z)?;
        std::fs::write(z.join(".zshenv"), "[ -f \"$HOME/.zshenv\" ] && . \"$HOME/.zshenv\"\n")?;
        std::fs::write(
            z.join(".zshrc"),
            format!("ZDOTDIR=\"$HOME\"\n[ -f \"$HOME/.zshrc\" ] && . \"$HOME/.zshrc\"\n{src}\n"),
        )?;
        Ok(format!("ZDOTDIR={} exec {} -i", sh_quote(&z.display().to_string()), sh_quote(&shell)))
    } else if shell.ends_with("bash") {
        let rc = dir.join(format!("{id}.bashrc"));
        std::fs::write(&rc, format!("[ -f \"$HOME/.bashrc\" ] && . \"$HOME/.bashrc\"\n{src}\n"))?;
        Ok(format!("exec {} --rcfile {} -i", sh_quote(&shell), sh_quote(&rc.display().to_string())))
    } else {
        Ok(format!("{src}; exec {}", sh_quote(&shell)))
    }
}

/// Terminal'de (Terminal.app veya iTerm) bir betik çalıştırır. `.command` dosyası `open -a` ile açılır;
/// AppleScript kullanılmadığı için "otomasyon izni" sorulmaz. `pid_file` verilirse betiğin PID'i yazılır
/// (Procfile'ı kapatmak için). `keep_open`: komut bitince kullanıcının kabuğu açık kalır.
pub fn terminal_script(title: &str, cwd: &Path, env_file: Option<&Path>, body: &str, keep_open: bool, pid_file: Option<&Path>) -> Result<()> {
    let dir = scripts_dir();
    std::fs::create_dir_all(&dir)?;
    cleanup_scripts(&dir);
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let id = format!("kd-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed));
    let file = dir.join(format!("{id}.command"));
    let mut s = String::from("#!/bin/sh\n");
    s.push_str(&format!("printf '\\033]0;%s\\007' {}\n", sh_quote(title)));
    if let Some(p) = pid_file {
        s.push_str(&format!("echo $$ > {}\n", sh_quote(&p.display().to_string())));
    }
    s.push_str(&format!("cd {} || exit 1\n", sh_quote(&cwd.display().to_string())));
    if let Some(env) = env_file {
        s.push_str(&format!(". {}\n", sh_quote(&env.display().to_string())));
    }
    s.push_str("clear\n");
    if !body.trim().is_empty() {
        s.push_str(body);
        s.push('\n');
    }
    if keep_open {
        s.push_str(&interactive_shell(env_file, &id)?);
        s.push('\n');
    }
    std::fs::write(&file, s)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700))?;
    let (code, out) = run_capture({
        let mut c = hidden("/usr/bin/open");
        c.arg("-a").arg(terminal_app()).arg(&file);
        c
    })?;
    if code != 0 {
        bail!("open -a {}: {}", terminal_app(), out.trim());
    }
    Ok(())
}

/// Görünür bir terminal penceresinde komut satırı çalıştırır. Dönen süreç pencereyi açan `true`'dur
/// (pencere ayrı yaşar); Procfile kendi PID dosyasını kullanır.
pub fn open_console(title: &str, cwd: &Path, cmdline: &str, keep_open: bool) -> Result<Child> {
    terminal_script(title, cwd, None, cmdline, keep_open, None)?;
    Command::new("/usr/bin/true").spawn().context("true")
}

// ---------------------------------------------------------------- süreçler

/// `lsof -nP -iTCP -sTCP:LISTEN -F pn` çıktısından port -> PID.
pub fn listeners() -> HashMap<u16, u32> {
    let mut map = HashMap::new();
    let mut c = hidden("/usr/sbin/lsof");
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

/// `kill -<sinyal> <pid>`.
fn signal(pid: u32, sig: &str) -> bool {
    let mut c = hidden("/bin/kill");
    c.arg(format!("-{sig}")).arg(pid.to_string());
    run_capture(c).map(|(code, _)| code == 0).unwrap_or(false)
}

/// Alt süreçleriyle birlikte sonlandırır: önce SIGTERM, bitmezse SIGKILL.
/// 0 (`kill 0` tüm süreç grubunu öldürür) ve kendi PID'imiz asla hedef alınmaz.
pub fn kill_tree(pid: u32) {
    if pid == 0 || pid == std::process::id() {
        crate::kcore::log(&format!("kill_tree: geçersiz PID {pid} atlandı"));
        return;
    }

    for c in child_pids(pid) {
        kill_tree(c);
    }
    signal(pid, "TERM");
    let start = std::time::Instant::now();
    while alive(pid) && start.elapsed() < std::time::Duration::from_secs(3) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    if alive(pid) {
        signal(pid, "KILL");
    }
}

/// Süreç yaşıyor mu? Bitmiş ama henüz beklenmemiş (zombi) süreç ölü sayılır.
fn alive(pid: u32) -> bool {
    let mut c = hidden("/bin/ps");
    c.args(["-o", "stat=", "-p", &pid.to_string()]);
    run_capture(c).map(|(_, out)| {
        let st = out.trim();
        !st.is_empty() && !st.starts_with('Z')
    }).unwrap_or(false)
}

fn child_pids(pid: u32) -> Vec<u32> {
    let mut c = hidden("/usr/bin/pgrep");
    c.args(["-P", &pid.to_string()]);
    run_capture(c).map(|(_, out)| out.lines().filter_map(|l| l.trim().parse().ok()).collect()).unwrap_or_default()
}

pub fn which(name: &str) -> Option<PathBuf> {
    let mut c = hidden("/usr/bin/which");
    c.arg(name);
    let (code, out) = run_capture(c).ok()?;
    let first = out.lines().next()?.trim();
    (code == 0 && !first.is_empty()).then(|| PathBuf::from(first))
}

// ---------------------------------------------------------------- yönetici izni

/// Kabuk betiğini yönetici olarak çalıştırır: macOS parola penceresi (`do shell script ... with administrator privileges`).
pub fn run_elevated_shell(script: &str, prompt: &str) -> Result<()> {
    let osa = format!("do shell script {} with prompt {} with administrator privileges", osa_quote(script), osa_quote(prompt));
    let (code, out) = run_capture({
        let mut c = hidden("/usr/bin/osascript");
        c.args(["-e", &osa]);
        c
    })?;
    // -128: kullanıcı parola penceresini iptal etti
    if out.contains("-128") {
        return Err(anyhow!("Yönetici izni verilmedi"));
    }
    if code != 0 {
        bail!("{}", out.trim());
    }
    Ok(())
}

/// Windows ile aynı imza: programı yönetici olarak çalıştırır (`args` kabuk sözdizimiyle eklenir).
#[allow(dead_code)]
pub fn run_elevated(program: &Path, args: &str, wait: bool) -> Result<()> {
    let code = run_elevated_code(program, args)?;
    if wait && code != 0 {
        bail!("Yönetici işlemi {code} koduyla bitti");
    }
    Ok(())
}

#[allow(dead_code)]
pub fn run_elevated_code(program: &Path, args: &str) -> Result<u32> {
    let cmd = format!("{} {args}", sh_quote(&program.display().to_string()));
    match run_elevated_shell(&cmd, "KaradelikDev") {
        Ok(_) => Ok(0),
        Err(e) if e.to_string().contains("izni verilmedi") => Err(e),
        Err(_) => Ok(1),
    }
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

/// Çalışan exe'nin içinde bulunduğu `.app` paketi (ör. /Applications/KaradelikDev.app).
pub fn app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    (macos.ends_with("MacOS") && contents.ends_with("Contents") && bundle.extension().map(|e| e == "app").unwrap_or(false))
        .then(|| bundle.to_path_buf())
}

/// `.app` paketi içinden çalışıyorsa (ör. /Applications/KaradelikDev.app/Contents/MacOS) o klasör kurulumdur.
pub fn install_dirs() -> Vec<PathBuf> {
    match (app_bundle(), std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf))) {
        (Some(_), Some(dir)) => vec![dir],
        _ => vec![],
    }
}

pub fn hosts_path() -> PathBuf {
    // Geliştirme derlemesinde testler gerçek hosts dosyasına (ve parola penceresine) dokunmasın diye
    if cfg!(debug_assertions) {
        if let Ok(p) = std::env::var("KARADELIK_HOSTS") {
            return PathBuf::from(p);
        }
    }
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

    #[test]
    fn quoting() {
        assert_eq!(sh_quote("a'b"), "'a'\\''b'");
        assert_eq!(osa_quote("x \"y\" \\"), "\"x \\\"y\\\" \\\\\"");
    }
}
