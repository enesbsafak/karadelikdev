//! Süreç, dosya ve ağ yardımcıları.

use anyhow::{anyhow, bail, Context, Result};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
pub const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

/// Konsol penceresi açmayan bir komut.
pub fn hidden(program: impl AsRef<OsStr>) -> Command {
    let mut c = Command::new(program);
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

/// Komutu çalıştırır, çıkış kodunu ve birleşik stdout+stderr çıktısını döndürür.
pub fn run_capture(mut cmd: Command) -> Result<(i32, String)> {
    let out = cmd
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("çalıştırılamadı: {:?}", cmd.get_program()))?;
    let mut text = decode(&out.stdout);
    let err = decode(&out.stderr);
    if !err.trim().is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&err);
    }
    Ok((out.status.code().unwrap_or(-1), text))
}

/// Windows konsol çıktısı UTF-8 veya OEM kod sayfasında olabilir.
pub fn decode(b: &[u8]) -> String {
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => b.iter().map(|&c| if c < 128 { c as char } else { '?' }).collect(),
    }
}

/// Komutu çıktısı log dosyasına gidecek şekilde çalıştırır ve en fazla `timeout` bekler.
/// Arka planda kalıcı süreç başlatan komutlar (ör. `pg_ctl start`) için: çıktı boruya bağlansaydı
/// başlatılan sunucu boruyu miras alır ve okuma hiç bitmezdi. Dönüş: çıkış kodu ve logun son satırları.
pub fn run_logged(mut cmd: Command, log: &Path, timeout: Duration) -> Result<(i32, String)> {
    if let Some(p) = log.parent() {
        fs::create_dir_all(p)?;
    }
    let start_len = fs::metadata(log).map(|m| m.len()).unwrap_or(0);
    let f = OpenOptions::new().create(true).append(true).open(log)?;
    let f2 = f.try_clone()?;
    let mut child = cmd.stdin(Stdio::null()).stdout(f).stderr(f2).spawn().with_context(|| format!("{:?}", cmd.get_program()))?;
    let started = Instant::now();
    let code = loop {
        if let Some(st) = child.try_wait()? {
            break st.code().unwrap_or(-1);
        }
        if started.elapsed() > timeout {
            kill_tree(child.id());
            bail!("{:?}: {}s içinde bitmedi", cmd.get_program(), timeout.as_secs());
        }
        std::thread::sleep(Duration::from_millis(150));
    };
    let all = fs::read(log).unwrap_or_default();
    let new = decode(&all[(start_len as usize).min(all.len())..]);
    Ok((code, new.lines().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n")))
}

/// Arka plan servisi başlatır; çıktısı log dosyasına eklenir.
pub fn spawn_bg(
    program: &Path,
    args: &[String],
    cwd: Option<&Path>,
    env: &[(String, String)],
    log: Option<&Path>,
) -> Result<Child> {
    let mut c = hidden(program);
    c.args(args).stdin(Stdio::null());
    if let Some(d) = cwd {
        c.current_dir(d);
    } else if let Some(p) = program.parent() {
        c.current_dir(p);
    }
    for (k, v) in env {
        c.env(k, v);
    }
    match log {
        Some(l) => {
            if let Some(p) = l.parent() {
                let _ = fs::create_dir_all(p);
            }
            let f = OpenOptions::new().create(true).append(true).open(l)?;
            let f2 = f.try_clone()?;
            c.stdout(f).stderr(f2);
        }
        None => {
            c.stdout(Stdio::null()).stderr(Stdio::null());
        }
    }
    c.spawn().with_context(|| format!("başlatılamadı: {}", program.display()))
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

pub fn port_open(port: u16) -> bool {
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok()
}

pub fn wait_port(port: u16, up: bool, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if port_open(port) == up {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
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

pub fn process_path(pid: u32) -> Option<PathBuf> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    let p = Pid::from_u32(pid);
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[p]),
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
    );
    sys.process(p).and_then(|pr| pr.exe().map(Path::to_path_buf))
}

/// Belirtilen exe'den çalışan ve KaradelikDev köküne ait (komut satırında veya çalışma dizininde
/// kök yolu geçen) süreçlerin PID'leri. sysinfo dizin bağlantılarını çözülmüş yol olarak döndürdüğü için
/// iki biçim de karşılaştırılır; kök koşulu aynı binary'yi kullanan Laragon süreçlerini dışarıda bırakır.
pub fn pids_by_exe(exe: &Path, root: &Path) -> Vec<u32> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::Always)
            .with_cmd(UpdateKind::Always)
            .with_cwd(UpdateKind::Always),
    );
    let low = |p: &Path| p.to_string_lossy().to_lowercase();
    let mut wanted = vec![low(exe)];
    if let Ok(real) = std::fs::canonicalize(exe) {
        wanted.push(low(&crate::paths::strip_unc(real)));
    }
    let root_l = low(root);
    let root_f = root_l.replace('\\', "/");
    sys.processes()
        .iter()
        .filter(|(_, p)| p.exe().map(|e| wanted.contains(&low(e))).unwrap_or(false))
        .filter(|(_, p)| {
            let cmd = p.cmd().iter().map(|a| a.to_string_lossy().to_lowercase()).collect::<Vec<_>>().join(" ");
            let cwd = p.cwd().map(low).unwrap_or_default();
            cmd.contains(&root_l) || cmd.contains(&root_f) || cwd.starts_with(&root_l)
        })
        .map(|(pid, _)| pid.as_u32())
        .collect()
}

pub fn kill_tree(pid: u32) {
    let mut c = hidden("taskkill");
    c.args(["/T", "/F", "/PID", &pid.to_string()]);
    let _ = run_capture(c);
}

/// Dizinde en fazla `depth` seviye aşağıda dosya arar (büyük/küçük harf duyarsız).
pub fn find_file(dir: &Path, name: &str, depth: usize) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    if depth == 0 {
        return None;
    }
    let mut subs: Vec<PathBuf> = fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    // "bin" klasörünü önce dene
    subs.sort_by_key(|p| if p.file_name().map(|n| n.eq_ignore_ascii_case("bin")).unwrap_or(false) { 0 } else { 1 });
    for s in subs {
        if let Some(f) = find_file(&s, name, depth - 1) {
            return Some(f);
        }
    }
    None
}

pub fn subdirs(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| natural_cmp(b, a));
    v
}

/// "php-8.10" > "php-8.9" sıralaması.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn parts(s: &str) -> Vec<(bool, String)> {
        let mut out = Vec::new();
        let mut cur = String::new();
        let mut digit = false;
        for ch in s.chars() {
            let d = ch.is_ascii_digit();
            if !cur.is_empty() && d != digit {
                out.push((digit, std::mem::take(&mut cur)));
            }
            digit = d;
            cur.push(ch.to_ascii_lowercase());
        }
        if !cur.is_empty() {
            out.push((digit, cur));
        }
        out
    }
    let (pa, pb) = (parts(a), parts(b));
    for (x, y) in pa.iter().zip(pb.iter()) {
        let o = if x.0 && y.0 {
            x.1.parse::<u64>().unwrap_or(0).cmp(&y.1.parse::<u64>().unwrap_or(0))
        } else {
            x.1.cmp(&y.1)
        };
        if o != std::cmp::Ordering::Equal {
            return o;
        }
    }
    pa.len().cmp(&pb.len())
}

/// İçerik değiştiyse yazar; yazıldıysa true.
pub fn write_if_changed(path: &Path, content: &str) -> Result<bool> {
    if fs::read_to_string(path).map(|c| c == content).unwrap_or(false) {
        return Ok(false);
    }
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(path, content).with_context(|| format!("yazılamadı: {}", path.display()))?;
    Ok(true)
}

/// Salt okunur dosyaları da (ör. .git içi) silebilen dizin silme.
pub fn remove_dir_force(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if fs::remove_dir_all(path).is_ok() {
        return Ok(());
    }
    clear_readonly(path);
    fs::remove_dir_all(path).with_context(|| format!("silinemedi: {}", path.display()))
}

fn clear_readonly(path: &Path) {
    if let Ok(md) = fs::symlink_metadata(path) {
        let mut perm = md.permissions();
        if perm.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            perm.set_readonly(false);
            let _ = fs::set_permissions(path, perm);
        }
        if md.is_dir() {
            if let Ok(rd) = fs::read_dir(path) {
                for e in rd.flatten() {
                    clear_readonly(&e.path());
                }
            }
        }
    }
}

pub fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for e in fs::read_dir(src)? {
        let e = e?;
        let to = dst.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &to)?;
        } else {
            fs::copy(e.path(), &to)?;
        }
    }
    Ok(())
}

/// Dosya/dizini taşır; farklı sürücüler arasında kopyala+sil yapar.
pub fn move_path(src: &Path, dst: &Path) -> Result<()> {
    if let Some(p) = dst.parent() {
        fs::create_dir_all(p)?;
    }
    if fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    if src.is_dir() {
        copy_dir(src, dst)?;
        remove_dir_force(src)
    } else {
        fs::copy(src, dst)?;
        fs::remove_file(src)?;
        Ok(())
    }
}

pub fn tail(path: &Path, lines: usize) -> String {
    let s = fs::read_to_string(path).unwrap_or_default();
    let v: Vec<&str> = s.lines().collect();
    v[v.len().saturating_sub(lines)..].join("\n")
}

pub fn now_stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d_%H%M%S").to_string()
}

pub fn random_password(len: usize) -> String {
    use rand::Rng;
    const CH: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut r = rand::rng();
    (0..len).map(|_| CH[r.random_range(0..CH.len())] as char).collect()
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

pub fn append_line(path: &Path, line: &str) {
    if let Some(p) = path.parent() {
        let _ = fs::create_dir_all(p);
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

/// Tırnaklı argümanları da tanıyan basit ayrıştırıcı (Procfile için).
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut has = false;
    for ch in s.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                has = true;
            }
            c if c.is_whitespace() && !quoted => {
                if has {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            c => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_sort() {
        let mut v = vec!["php-8.9.1", "php-8.10.0", "php-7.4.33"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, vec!["php-7.4.33", "php-8.9.1", "php-8.10.0"]);
    }

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

    #[test]
    fn args_split() {
        assert_eq!(split_args(r#"npm run dev PWD=www/a DB="a b""#), vec!["npm", "run", "dev", "PWD=www/a", "DB=a b"]);
    }
}

