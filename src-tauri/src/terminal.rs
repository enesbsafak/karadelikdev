//! Terminal: seçili tüm sürümleri PATH'e ekleyen ortam dosyası üretilir (Laragon'daki laragon.cmd).
//! Windows: `usr\kdenv.cmd`; Windows Terminal / Cmder / cmd / PowerShell bu dosyayla açılır.
//! macOS: `usr/kdenv.sh`; Terminal.app veya iTerm kullanıcının kabuğunu bu dosyayı yükleyerek açar.

use crate::kcore::{paths, settings};
use crate::platform::exe as x;
use crate::util;
#[cfg(windows)]
use crate::util::which;
use crate::versions;
use anyhow::{Context, Result};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// PATH'e eklenecek dizinler (öncelik sırasıyla).
pub fn path_dirs() -> Vec<PathBuf> {
    let p = paths();
    let mut v: Vec<PathBuf> = vec![p.usr().join("bin")];
    let mut push = |d: PathBuf| {
        if d.is_dir() && !v.contains(&d) {
            v.push(d);
        }
    };
    if let Some(d) = versions::current_dir("php") {
        // Homebrew PHP: php-8.4/bin
        push(d.join("bin"));
        if cfg!(windows) {
            push(d);
        }
    }
    push(p.bin().join("composer"));
    for (cat, exe) in [
        ("mysql", "mysql"),
        ("postgresql", "psql"),
        ("mongodb", "mongod"),
        ("apache", "httpd"),
        ("nginx", "nginx"),
        ("redis", "redis-cli"),
        ("memcached", "memcached"),
        ("mailpit", "mailpit"),
        ("nodejs", "node"),
        ("go", "go"),
        ("ruby", "ruby"),
        ("java", "java"),
    ] {
        if let Some(f) = versions::exe(cat, &x(exe)) {
            push(f.parent().unwrap().to_path_buf());
        }
    }
    if let Some(d) = versions::current_dir("python") {
        if cfg!(windows) {
            push(d.join("Scripts"));
            push(d);
        } else {
            // Homebrew python@3.x: libexec/bin'de sürüm eksiz `python`/`pip` bağlantıları
            push(d.join("libexec").join("bin"));
            push(d.join("bin"));
        }
    }
    // Sürümsüz araçlar
    for tool in ["git", "ngrok", "cloudflared", "heidisql", "notepad++", "dbeaver", "vscode", "pocketbase"] {
        let d = p.bin().join(tool);
        if d.is_dir() {
            for sub in ["cmd", "bin", "mingw64\\bin", "usr\\bin"] {
                let sd = d.join(sub);
                if sd.is_dir() {
                    push(sd);
                }
            }
            push(d);
        }
    }
    v
}

pub fn env_vars() -> Vec<(String, String)> {
    let p = paths();
    let s = settings();
    let mut v = vec![
        ("KD_ROOT".to_string(), p.root.display().to_string()),
        ("DOCUMENT_ROOT".to_string(), s.document_root.clone()),
    ];
    if let Some(d) = crate::php::ini_dir() {
        v.push(("PHPRC".into(), d.display().to_string()));
    }
    if let Some(d) = versions::current_dir("go") {
        let root = util::find_file(&d, &x("go"), 2).and_then(|f| f.parent().and_then(|b| b.parent()).map(Path::to_path_buf));
        // Homebrew go: GOROOT libexec'tir; go ikilisi kendi kökünü bildiği için ayarlanmaz
        if let Some(r) = root.filter(|r| r.join("src").is_dir()) {
            v.push(("GOROOT".into(), r.display().to_string()));
        }
    }
    if let Some(java) = versions::exe("java", &x("java")) {
        if let Some(home) = java.parent().and_then(|b| b.parent()) {
            // Homebrew openjdk: libexec/openjdk.jdk/Contents/Home
            let jdk = home.join("libexec").join("openjdk.jdk").join("Contents").join("Home");
            let home = if jdk.is_dir() { jdk } else { home.to_path_buf() };
            v.push(("JAVA_HOME".into(), home.display().to_string()));
        }
    }
    if crate::services::is_running(crate::services::Svc::Mysql) {
        v.push(("MYSQL_PWD".into(), crate::mysqlops::root_password()));
    }
    let pg = crate::services::others::pg_password();
    if !pg.is_empty() {
        v.push(("PGPASSWORD".into(), pg));
    }
    v
}

/// Proje başına PHP: `usr/bin/php` (Windows `php.cmd`) çalışma dizinine bakar; proje klasörünün içindeyse
/// o projenin sürümünü (`usr/projects.ini` `Php=`) ve php.ini'sini (PHPRC), değilse profildeki sürümü çalıştırır.
/// `usr/bin` PATH'in ilk dizini olduğundan `php artisan`, `composer` ve Procfile/cron komutları da bu sürümü kullanır.
/// Sürümü farklı proje yoksa dosya silinir; `php` doğrudan profildeki ikiliye çözülür.
pub fn write_php_shim() -> Result<()> {
    let file = paths().usr().join("bin").join(if cfg!(windows) { "php.cmd" } else { "php" });
    let current = crate::php::version_name().unwrap_or_default();
    let root = PathBuf::from(settings().document_root);
    let mut entries = vec![];
    for (name, cfg) in crate::projects::all() {
        if cfg.php.is_empty() || cfg.php == current {
            continue;
        }
        if let Some(php) = crate::php::php_exe_for(&cfg.php) {
            // Proje sürümünün php.ini'si yalnız php-cgi havuzu başlarken oluşuyordu; terminalde de gerekli
            if let Err(e) = crate::php::ensure_ini_for(&cfg.php) {
                crate::kcore::log(&format!("php.ini {}: {e:#}", cfg.php));
            }
            entries.push(ShimEntry { dir: root.join(&name), php, ini: crate::php::ini_dir_for(&cfg.php) });
        }
    }
    let default = crate::php::php_exe();
    let (Some(default), false) = (default, entries.is_empty()) else {
        if file.exists() {
            std::fs::remove_file(&file)?;
        }
        return Ok(());
    };
    if util::write_if_changed(&file, &php_shim(&default, &entries))? {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(())
}

pub struct ShimEntry {
    pub dir: PathBuf,
    pub php: PathBuf,
    pub ini: PathBuf,
}

/// cmd.exe: önek karşılaştırması `if /i` ile (büyük/küçük harf duyarsız). Toplu iş dosyası OEM kod sayfasıyla okunduğu
/// için ASCII dışı ya da `!`, `%`, `"` içeren yollar atlanır (o projede profildeki sürüm çalışır). Değişkenler
/// gecikmeli genişletme kapalıyken taşınır; böylece `%*` argümanlarındaki `!` bozulmaz.
#[cfg(windows)]
pub fn php_shim(default: &Path, entries: &[ShimEntry]) -> String {
    let ok = |p: &Path| {
        let s = p.display().to_string();
        s.is_ascii() && !s.contains(['!', '%', '"'])
    };
    let mut l = vec![
        "@echo off".to_string(),
        ":: KaradelikDev tarafından üretilir (usr\\projects.ini'deki proje başına PHP sürümleri). Elle düzenlemeyin.".into(),
        "setlocal DisableDelayedExpansion".into(),
        "set \"KD_D=%CD%\\\"".into(),
        format!("set \"KD_PHP={}\"", default.display()),
        "setlocal EnableDelayedExpansion".into(),
    ];
    for e in entries.iter().filter(|e| ok(&e.dir) && ok(&e.php) && ok(&e.ini)) {
        // %CD% ters eğik çizgilidir; ayarlardaki kök `C:/...` biçiminde olabilir
        let prefix = format!("{}\\", e.dir.display().to_string().replace('/', "\\").trim_end_matches('\\'));
        l.push(format!(
            "if /i \"!KD_D:~0,{}!\"==\"{prefix}\" (set \"KD_PHP={}\" & set \"PHPRC={}\")",
            prefix.chars().count(),
            e.php.display(),
            e.ini.display()
        ));
    }
    l.push("endlocal & set \"KD_PHP=%KD_PHP%\" & set \"PHPRC=%PHPRC%\"".into());
    l.push("\"%KD_PHP%\" %*".into());
    l.push("exit /b %ERRORLEVEL%".into());
    l.join("\r\n") + "\r\n"
}

#[cfg(not(windows))]
pub fn php_shim(default: &Path, entries: &[ShimEntry]) -> String {
    let q = |p: &Path| crate::platform::sh_quote(&p.display().to_string());
    let mut l = vec![
        "#!/bin/sh".to_string(),
        "# KaradelikDev tarafından üretilir (usr/projects.ini'deki proje başına PHP sürümleri). Elle düzenlemeyin.".into(),
        "case \"$PWD/\" in".into(),
    ];
    for e in entries {
        let prefix = format!("{}/", e.dir.display().to_string().trim_end_matches('/'));
        l.push(format!(
            "  {}*) PHPRC={}; export PHPRC; exec {} \"$@\" ;;",
            crate::platform::sh_quote(&prefix),
            q(&e.ini),
            q(&e.php)
        ));
    }
    l.push("esac".into());
    l.push(format!("exec {} \"$@\"", q(default)));
    l.join("\n") + "\n"
}

pub fn env_cmd_path() -> PathBuf {
    paths().usr().join(if cfg!(windows) { "kdenv.cmd" } else { "kdenv.sh" })
}

/// Kullanıcının kendi başlangıç komutları (`usr\user.cmd` / `usr/user.sh`).
pub fn user_script_path() -> PathBuf {
    paths().usr().join(if cfg!(windows) { "user.cmd" } else { "user.sh" })
}

/// Ortam dosyasını günceller (`usr/kdenv.sh`): seçili sürümler PATH'in başında.
#[cfg(not(windows))]
pub fn write_env_cmd() -> Result<PathBuf> {
    if let Err(e) = write_php_shim() {
        crate::kcore::log(&format!("usr/bin/php: {e:#}"));
    }
    let q = |s: &str| crate::platform::sh_quote(s);
    let mut lines = vec!["# KaradelikDev tarafından üretilir. Elle düzenlemeyin; kendi komutlarınız için usr/user.sh".to_string()];
    for (k, v) in env_vars() {
        if k == "MYSQL_PWD" || k == "PGPASSWORD" {
            continue;
        }
        lines.push(format!("export {k}={}", q(&v)));
    }
    let dirs: Vec<String> = path_dirs().iter().map(|d| d.display().to_string()).collect();
    let composer = crate::platform::home().join(".composer").join("vendor").join("bin");
    lines.push(format!("export PATH={}\"$PATH\"", q(&format!("{}:{}:", dirs.join(":"), composer.display()))));
    lines.push("[ -f \"$KD_ROOT/usr/user.sh\" ] && . \"$KD_ROOT/usr/user.sh\"".into());
    let file = env_cmd_path();
    util::write_if_changed(&file, &(lines.join("\n") + "\n"))?;
    Ok(file)
}

/// usr\kdenv.cmd dosyasını günceller.
#[cfg(windows)]
pub fn write_env_cmd() -> Result<PathBuf> {
    if let Err(e) = write_php_shim() {
        crate::kcore::log(&format!("usr\\bin\\php.cmd: {e:#}"));
    }
    let mut lines = vec!["@echo off".to_string(), ":: KaradelikDev tarafından üretilir. Elle düzenlemeyin; kendi komutlarınız için usr\\user.cmd".to_string()];
    for (k, v) in env_vars() {
        if k == "MYSQL_PWD" || k == "PGPASSWORD" {
            continue;
        }
        lines.push(format!("set \"{k}={v}\""));
    }
    let dirs: Vec<String> = path_dirs().iter().map(|d| d.display().to_string()).collect();
    lines.push(format!("set \"PATH={};%APPDATA%\\npm;%APPDATA%\\Composer\\vendor\\bin;%PATH%\"", dirs.join(";")));
    lines.push("if exist \"%KD_ROOT%\\usr\\user.cmd\" call \"%KD_ROOT%\\usr\\user.cmd\"".into());
    let file = env_cmd_path();
    util::write_if_changed(&file, &(lines.join("\r\n") + "\r\n"))?;
    Ok(file)
}

#[cfg(windows)]
fn terminal_kind() -> String {
    let t = settings().terminal;
    if t != "auto" {
        return t;
    }
    if which("wt.exe").is_some() {
        "wt".into()
    } else if paths().bin().join("cmder").join("Cmder.exe").exists() {
        "cmder".into()
    } else {
        "cmd".into()
    }
}

/// Terminali verilen dizinde açar. `run` verilirse ortam yüklendikten sonra çalıştırılır; sonra kabuk açık kalır.
#[cfg(not(windows))]
pub fn open(dir: &Path, run: Option<&str>) -> Result<()> {
    let env = write_env_cmd()?;
    crate::platform::terminal_script("KaradelikDev", dir, Some(&env), run.unwrap_or(""), true, None)
}

/// Terminali verilen dizinde açar. `run` verilirse ortam yüklendikten sonra çalıştırılır.
#[cfg(windows)]
pub fn open(dir: &Path, run: Option<&str>) -> Result<()> {
    let env_cmd = write_env_cmd()?;
    let init = match run {
        Some(cmd) => format!("call \"{}\" && {cmd}", env_cmd.display()),
        None => format!("call \"{}\"", env_cmd.display()),
    };
    match terminal_kind().as_str() {
        "wt" => {
            // wt.exe bir Store "uygulama yürütme takma adı"dır; CreateProcess onu açamaz
            // (os error 1920), ShellExecute açar. wt ';' karakterini komut ayırıcı sayar.
            // `start` takma adı kabuk üzerinden çözer (ShellExecute'un COM gerektiren yolunu kendisi yapar)
            let mut c = util::hidden("cmd.exe");
            c.raw_arg(format!("/c start \"\" wt.exe -d \"{}\" cmd.exe /k \"{}\"", dir.display(), init.replace(';', "\\;")));
            c.spawn().context("wt.exe")?;
        }
        "powershell" => {
            let mut c = Command::new("cmd.exe");
            c.creation_flags(crate::platform::CREATE_NEW_CONSOLE).current_dir(dir);
            c.raw_arg(format!("/s /k \"{init} && powershell -NoLogo\""));
            c.spawn()?;
        }
        "cmder" => {
            let cmder = paths().bin().join("cmder");
            let profile = cmder.join("config").join("user_profile.cmd");
            let line = format!("call \"{}\"", env_cmd.display());
            let cur = std::fs::read_to_string(&profile).unwrap_or_default();
            if !cur.contains(&line) {
                util::append_line(&profile, &line);
            }
            Command::new(cmder.join("Cmder.exe")).arg("/START").arg(dir).spawn()?;
        }
        _ => {
            util::open_console("KaradelikDev", dir, &init, true)?;
        }
    }
    Ok(())
}

/// Dosyayı açar: Tercihler'deki editör (`.app` paketi veya çalıştırılabilir dosya), yoksa VS Code, yoksa
/// sistemin varsayılan metin düzenleyicisi (`open -t`).
#[cfg(not(windows))]
pub fn edit(file: &Path) -> Result<()> {
    if !file.exists() {
        if let Some(p) = file.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let _ = std::fs::write(file, "");
    }
    let editor = settings().editor.trim().to_string();
    let code = Path::new("/Applications/Visual Studio Code.app");
    let mut c = Command::new("/usr/bin/open");
    if editor.ends_with(".app") {
        c.arg("-a").arg(&editor);
    } else if !editor.is_empty() {
        Command::new(&editor).arg(file).spawn().with_context(|| editor.clone())?;
        return Ok(());
    } else if code.exists() {
        c.arg("-a").arg(code);
    } else {
        c.arg("-t");
    }
    c.arg(file).spawn().context("open")?;
    Ok(())
}

/// Dosyayı seçili editörle (yoksa Notepad++ / Not Defteri) açar.
#[cfg(windows)]
pub fn edit(file: &Path) -> Result<()> {
    if !file.exists() {
        if let Some(p) = file.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let _ = std::fs::write(file, "");
    }
    let s = settings();
    let editor = if !s.editor.trim().is_empty() {
        PathBuf::from(s.editor.trim())
    } else if let Some(npp) = versions::tool("notepad++", "notepad++.exe") {
        npp
    } else if let Some(code) = which("code.cmd") {
        code
    } else {
        PathBuf::from("notepad.exe")
    };
    let mut c = Command::new(&editor);
    if editor.extension().map(|e| e == "cmd" || e == "bat").unwrap_or(false) {
        c.creation_flags(crate::platform::CREATE_NO_WINDOW);
    }
    c.arg(file).spawn().with_context(|| editor.display().to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn php_shim_cmd() {
        let e = |d: &str, v: &str| ShimEntry {
            dir: PathBuf::from(d),
            php: PathBuf::from(format!(r"C:\kd\bin\php\{v}\php.exe")),
            ini: PathBuf::from(format!(r"C:\kd\etc\php\{v}")),
        };
        let s = php_shim(Path::new(r"C:\kd\bin\php\8.4\php.exe"), &[e(r"C:/kd/www\blog", "8.2"), e(r"C:\kd\www\çay", "8.1")]);
        assert!(s.contains(r#"set "KD_PHP=C:\kd\bin\php\8.4\php.exe""#));
        assert!(s.contains(r#"if /i "!KD_D:~0,15!"=="C:\kd\www\blog\" (set "KD_PHP=C:\kd\bin\php\8.2\php.exe" & set "PHPRC=C:\kd\etc\php\8.2")"#));
        // ASCII dışı yol atlanır
        assert!(!s.contains("8.1"));
        assert!(s.ends_with("\"%KD_PHP%\" %*\r\nexit /b %ERRORLEVEL%\r\n"));
    }

    #[cfg(not(windows))]
    #[test]
    fn php_shim_sh() {
        let e = ShimEntry { dir: "/kd/www/blog".into(), php: "/opt/php@8.2/bin/php".into(), ini: "/kd/etc/php/8.2".into() };
        let s = php_shim(Path::new("/opt/php/bin/php"), &[e]);
        assert!(s.contains("  '/kd/www/blog/'*) PHPRC='/kd/etc/php/8.2'; export PHPRC; exec '/opt/php@8.2/bin/php' \"$@\" ;;"));
        assert!(s.ends_with("esac\nexec '/opt/php/bin/php' \"$@\"\n"));
    }
}
