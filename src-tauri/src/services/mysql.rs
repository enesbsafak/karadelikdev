//! MySQL / MariaDB. Konfigürasyon etc\mysql\<anahtar>.ini, veri <DataDir>\<anahtar> (ör. data\mysql-8.4).

use super::{ensure_port_free, kill_by_exe, kill_tracked, track, Svc};
use crate::kcore::{paths, settings};
use crate::i18n::{t, tf};
use crate::paths::fwd;
use crate::util::{self, hidden, run_capture};
use crate::versions;
use anyhow::{anyhow, bail, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn mysqld() -> Option<PathBuf> {
    Svc::Mysql.exe()
}

pub fn bin_dir() -> Option<PathBuf> {
    mysqld().and_then(|p| p.parent().map(Path::to_path_buf))
}

pub fn tool(name: &str) -> Option<PathBuf> {
    let d = bin_dir()?;
    [crate::platform::exe(name), crate::platform::exe(&name.replace("mysql", "mariadb"))]
        .iter()
        .map(|n| d.join(n))
        .find(|p| p.exists())
}

pub fn key() -> Option<String> {
    versions::current("mysql").map(|v| versions::major_minor_key(&v, "mysql"))
}

pub fn is_mariadb() -> bool {
    versions::current("mysql").map(|v| v.to_lowercase().contains("mariadb")).unwrap_or(false)
        || bin_dir().map(|d| d.join(crate::platform::exe("mariadbd")).exists()).unwrap_or(false)
}

pub fn data_dir() -> Option<PathBuf> {
    key().map(|k| PathBuf::from(settings().data_dir).join(k))
}

pub fn ini_path() -> Option<PathBuf> {
    key().map(|k| paths().etc().join("mysql").join(format!("{k}.ini")))
}

pub fn log_path() -> PathBuf {
    paths().logs().join("mysqld.log")
}

/// Unix soket dosyası (macOS şablonu kullanır). Homebrew'un /tmp/mysql.sock'u ile çakışmasın diye kökte;
/// yol 104 karakteri geçerse soket açılamaz, o durumda /tmp altında kök yoluna özgü bir ad kullanılır.
pub fn socket_path() -> PathBuf {
    let p = paths().tmp().join("mysql.sock");
    if p.as_os_str().len() < 100 {
        return p;
    }
    use sha2::{Digest, Sha256};
    let h = hex::encode(Sha256::digest(paths().root.to_string_lossy().as_bytes()));
    PathBuf::from(format!("/tmp/karadelikdev-{}.sock", &h[..12]))
}

pub fn write_ini() -> Result<PathBuf> {
    let exe = mysqld().ok_or_else(|| anyhow!(tf("msg.not_installed", &["MySQL"])))?;
    let basedir = exe.parent().and_then(|p| p.parent()).unwrap().to_path_buf();
    let ini = ini_path().unwrap();
    // LOAD DATA INFILE / SELECT ... INTO OUTFILE yalnız bu klasörde; boş değer tüm diske erişim verir
    let files = PathBuf::from(settings().data_dir).join("mysql-files");
    std::fs::create_dir_all(&files)?;
    let content = crate::templates::render_named(
        "my.ini.tpl",
        &[
            ("PORT", settings().svc("mysql").port.to_string()),
            ("BASEDIR", fwd(&basedir)),
            ("DATADIR", fwd(&data_dir().unwrap())),
            ("LOG_DIR", fwd(&paths().logs())),
            ("SECURE_FILE_PRIV", fwd(&files)),
            ("SOCKET", fwd(&socket_path())),
        ],
    );
    util::write_if_changed(&ini, &content)?;
    Ok(ini)
}

/// İlk çalıştırmada veri dizinini oluşturur (Laragon: "Init DataDir at: ...").
pub fn init_data_dir() -> Result<()> {
    let data = data_dir().unwrap();
    if data.join("mysql").exists() {
        return Ok(());
    }
    if data.exists() && std::fs::read_dir(&data).map(|mut r| r.next().is_some()).unwrap_or(false) {
        bail!(tf("msg.datadir_invalid", &[&data.display().to_string()]));
    }
    crate::kcore::status(&t("msg.initializing"));
    std::fs::create_dir_all(data.parent().unwrap())?;
    let ini = write_ini()?;
    let (code, out) = if is_mariadb() {
        let installer = tool("mysql_install_db")
            .or_else(|| tool("mariadb-install-db"))
            .ok_or_else(|| anyhow!("mariadb-install-db bulunamadı"))?;
        let mut c = hidden(installer);
        if cfg!(windows) {
            c.arg(format!("--datadir={}", data.display()));
        } else {
            // Unix betiği: varsayılan root yalnız unix_socket ile girer; TCP'den parolasız girişe izin ver
            let basedir = mysqld().and_then(|e| e.parent().and_then(|p| p.parent()).map(Path::to_path_buf)).unwrap();
            c.arg(format!("--defaults-file={}", ini.display()))
                .arg(format!("--basedir={}", basedir.display()))
                .arg(format!("--datadir={}", data.display()))
                .args(["--auth-root-authentication-method=normal", "--skip-test-db"]);
        }
        run_capture(c)?
    } else {
        let mut c = hidden(mysqld().unwrap());
        c.arg(format!("--defaults-file={}", ini.display())).args(["--initialize-insecure", "--console"]);
        run_capture(c)?
    };
    if code != 0 || !data.join("mysql").exists() {
        bail!("{}\n{}", t("msg.init_failed"), out.lines().rev().take(6).collect::<Vec<_>>().join("\n"));
    }
    crate::kcore::log(&format!("Init DataDir at: {}", data.display()));
    // --initialize-insecure / mariadb-install-db root parolasını boş bırakır
    crate::mysqlops::set_root_password("");
    Ok(())
}

pub fn start_with(extra: &[String]) -> Result<()> {
    let port = settings().svc("mysql").port;
    ensure_port_free(Svc::Mysql, port)?;
    let ini = write_ini()?;
    init_data_dir()?;
    let exe = mysqld().unwrap();
    // --defaults-file ilk argüman olmalı
    let mut args = vec![format!("--defaults-file={}", ini.display())];
    args.extend_from_slice(extra);
    let child = util::spawn_bg(&exe, &args, exe.parent(), &[], None)?;
    track("mysql", child);
    if !util::wait_port(port, true, Duration::from_secs(30)) {
        let tail = util::tail(&log_path(), 8);
        kill_tracked("mysql");
        bail!("{}\n{}", tf("msg.start_failed", &["MySQL"]), tail);
    }
    Ok(())
}

pub fn start() -> Result<()> {
    start_with(&[])
}

pub fn stop() -> Result<()> {
    let port = settings().svc("mysql").port;
    if util::port_open(port) {
        if let Some(admin) = tool("mysqladmin") {
            let mut c = hidden(admin);
            c.args(["-u", "root", "-h", "127.0.0.1", "-P", &port.to_string(), "shutdown"])
                .env("MYSQL_PWD", crate::mysqlops::root_password());
            let _ = run_capture(c);
        }
        if !util::wait_port(port, false, Duration::from_secs(15)) {
            crate::kcore::log("Graceful shutdown failed - zorla kapatılıyor");
            if let Some(exe) = mysqld() {
                kill_by_exe(&exe);
            }
        }
        // Port kapandıktan sonra mysqld veri dosyalarını bir süre daha tutar; hemen yeniden başlatılırsa
        // "ibdata1 must be writable" hatası verir
        if let Some(exe) = mysqld() {
            let started = std::time::Instant::now();
            while started.elapsed() < Duration::from_secs(10) && !util::pids_by_exe(&exe, &paths().root).is_empty() {
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
    kill_tracked("mysql");
    Ok(())
}
