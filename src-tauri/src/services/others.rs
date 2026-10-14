//! PostgreSQL, MongoDB, Redis, Memcached, Mailpit ve PHP yerleşik sunucusu.

use super::{ensure_port_free, kill_tracked, service_env, track, Svc};
use crate::kcore::{paths, settings};
use crate::i18n::{t, tf};
use crate::paths::fwd;
use crate::util::{self, hidden};
use crate::versions;
use anyhow::{anyhow, bail, Result};
use std::path::PathBuf;
use std::time::Duration;

fn data_root() -> PathBuf {
    PathBuf::from(settings().data_dir)
}

fn wait_or_fail(svc: Svc, port: u16, secs: u64, log: Option<PathBuf>) -> Result<()> {
    if util::wait_port(port, true, Duration::from_secs(secs)) {
        return Ok(());
    }
    kill_tracked(svc.id());
    let tail = log.map(|l| util::tail(&l, 8)).unwrap_or_default();
    bail!("{}\n{}", tf("msg.start_failed", &[svc.title()]), tail)
}

// ---------------------------------------------------------------- PostgreSQL

pub fn pg_bin() -> Option<PathBuf> {
    Svc::Postgresql.exe().and_then(|e| e.parent().map(|p| p.to_path_buf()))
}

pub fn pg_data() -> Option<PathBuf> {
    versions::current("postgresql").map(|v| data_root().join(versions::major_minor_key(&v, "postgresql")))
}

pub fn pg_log() -> PathBuf {
    paths().logs().join("postgresql.log")
}

pub fn postgres_start() -> Result<()> {
    let port = settings().svc("postgresql").port;
    ensure_port_free(Svc::Postgresql, port)?;
    let bin = pg_bin().ok_or_else(|| anyhow!(tf("msg.not_installed", &["PostgreSQL"])))?;
    let data = pg_data().unwrap();
    if !data.join("PG_VERSION").exists() {
        crate::kcore::status(&t("msg.initializing"));
        std::fs::create_dir_all(&data)?;
        let mut c = hidden(bin.join("initdb.exe"));
        c.arg("-D").arg(&data).args(["-U", "postgres", "-A", "trust", "-E", "UTF8", "--locale=C"]);
        let (code, out) = util::run_logged(c, &pg_log(), Duration::from_secs(180))?;
        if code != 0 {
            bail!("initdb: {}", out.lines().rev().take(6).collect::<Vec<_>>().join("\n"));
        }
    }
    let mut c = hidden(bin.join("pg_ctl.exe"));
    c.arg("-D").arg(&data).arg("-l").arg(pg_log()).arg("-o").arg(format!("-p {port}")).args(["-w", "start"]);
    // pg_ctl, postgres'i arka planda başlatır; çıktısı boruya bağlanırsa okuma hiç bitmez
    let (code, out) = util::run_logged(c, &paths().logs().join("pg_ctl.log"), Duration::from_secs(90))?;
    if code != 0 {
        bail!("pg_ctl: {}\n{}", out.trim(), util::tail(&pg_log(), 6));
    }
    wait_or_fail(Svc::Postgresql, port, 10, Some(pg_log()))
}

pub fn postgres_stop() -> Result<()> {
    if let (Some(bin), Some(data)) = (pg_bin(), pg_data()) {
        let mut c = hidden(bin.join("pg_ctl.exe"));
        c.arg("-D").arg(&data).args(["-m", "fast", "-w", "stop"]);
        let _ = util::run_logged(c, &paths().logs().join("pg_ctl.log"), Duration::from_secs(60));
    }
    Ok(())
}

// ---------------------------------------------------------------- MongoDB

pub fn mongo_conf() -> PathBuf {
    paths().etc().join("mongodb").join("mongod.conf")
}

pub fn mongo_start() -> Result<()> {
    let port = settings().svc("mongodb").port;
    ensure_port_free(Svc::Mongodb, port)?;
    let exe = Svc::Mongodb.exe().ok_or_else(|| anyhow!(tf("msg.not_installed", &["MongoDB"])))?;
    let data = data_root().join("mongodb");
    std::fs::create_dir_all(&data)?;
    let conf = crate::templates::render_named(
        "mongod.conf.tpl",
        &[("DATADIR", fwd(&data)), ("LOG_DIR", fwd(&paths().logs())), ("PORT", port.to_string())],
    );
    util::write_if_changed(&mongo_conf(), &conf)?;
    let args = vec!["-f".to_string(), mongo_conf().display().to_string()];
    let child = util::spawn_bg(&exe, &args, None, &[], None)?;
    track("mongodb", child);
    wait_or_fail(Svc::Mongodb, port, 20, Some(paths().logs().join("mongod.log")))
}

// ---------------------------------------------------------------- Redis

pub fn redis_conf() -> PathBuf {
    paths().etc().join("redis").join("redis.conf")
}

/// Cygwin/MSYS2 derlemeleri POSIX yol bekler.
fn redis_path(p: &std::path::Path, exe_dir: &std::path::Path) -> String {
    let f = fwd(p);
    let posix = |prefix: &str| {
        let mut ch = f.chars();
        match (ch.next(), ch.next()) {
            (Some(d), Some(':')) => format!("{prefix}/{}{}", d.to_ascii_lowercase(), ch.as_str()),
            _ => f.clone(),
        }
    };
    if exe_dir.join("cygwin1.dll").exists() {
        posix("/cygdrive")
    } else if exe_dir.join("msys-2.0.dll").exists() {
        posix("")
    } else {
        f.clone()
    }
}

pub fn redis_start() -> Result<()> {
    let port = settings().svc("redis").port;
    ensure_port_free(Svc::Redis, port)?;
    let exe = Svc::Redis.exe().ok_or_else(|| anyhow!(tf("msg.not_installed", &["Redis"])))?;
    let exe_dir = exe.parent().unwrap().to_path_buf();
    let data = data_root().join("redis");
    std::fs::create_dir_all(&data)?;
    if !redis_conf().exists() {
        util::write_if_changed(
            &redis_conf(),
            "# KaradelikDev Redis ayarları. port/bind/dir komut satırından verilir.\nmaxmemory 256mb\nmaxmemory-policy allkeys-lru\nsave 900 1\nsave 300 10\n",
        )?;
    }
    let args = vec![
        redis_path(&redis_conf(), &exe_dir),
        "--port".into(),
        port.to_string(),
        "--bind".into(),
        "127.0.0.1".into(),
        "--dir".into(),
        ".".into(),
    ];
    let child = util::spawn_bg(&exe, &args, Some(&data), &[], Some(&paths().logs().join("redis.log")))?;
    track("redis", child);
    wait_or_fail(Svc::Redis, port, 8, Some(paths().logs().join("redis.log")))
}

// ---------------------------------------------------------------- Memcached

pub fn memcached_start() -> Result<()> {
    let port = settings().svc("memcached").port;
    ensure_port_free(Svc::Memcached, port)?;
    let exe = Svc::Memcached.exe().ok_or_else(|| anyhow!(tf("msg.not_installed", &["Memcached"])))?;
    let args: Vec<String> = ["-p", &port.to_string(), "-l", "127.0.0.1", "-m", "64"].iter().map(|s| s.to_string()).collect();
    let child = util::spawn_bg(&exe, &args, None, &[], Some(&paths().logs().join("memcached.log")))?;
    track("memcached", child);
    wait_or_fail(Svc::Memcached, port, 5, None)
}

// ---------------------------------------------------------------- Mailpit

pub fn mailpit_ui_url() -> String {
    format!("http://127.0.0.1:{}", settings().mailpit_ui_port)
}

pub fn mailpit_start() -> Result<()> {
    let s = settings();
    let port = s.svc("mailpit").port;
    ensure_port_free(Svc::Mailpit, port)?;
    ensure_port_free(Svc::Mailpit, s.mailpit_ui_port)?;
    let exe = Svc::Mailpit.exe().ok_or_else(|| anyhow!(tf("msg.not_installed", &["Mailpit"])))?;
    let data = data_root().join("mailpit");
    std::fs::create_dir_all(&data)?;
    let args = vec![
        "--smtp".into(),
        format!("127.0.0.1:{port}"),
        "--listen".into(),
        format!("127.0.0.1:{}", s.mailpit_ui_port),
        "--database".into(),
        data.join("mailpit.db").display().to_string(),
        "--quiet".into(),
    ];
    let child = util::spawn_bg(&exe, &args, None, &[], Some(&paths().logs().join("mailpit.log")))?;
    track("mailpit", child);
    wait_or_fail(Svc::Mailpit, port, 8, Some(paths().logs().join("mailpit.log")))
}

// ---------------------------------------------------------------- PHP yerleşik sunucu

pub fn phpserver_start() -> Result<()> {
    let port = settings().svc("phpserver").port;
    ensure_port_free(Svc::PhpServer, port)?;
    let php = crate::php::php_exe().ok_or_else(|| anyhow!(tf("msg.not_installed", &["PHP"])))?;
    let ini = crate::php::ensure_ini()?;
    let root = settings().document_root;
    let args = vec![
        "-S".into(),
        format!("127.0.0.1:{port}"),
        "-t".into(),
        root.clone(),
        "-c".into(),
        ini.display().to_string(),
    ];
    let child = util::spawn_bg(&php, &args, Some(std::path::Path::new(&root)), &service_env(&[]), Some(&paths().logs().join("php-server.log")))?;
    track("phpserver", child);
    wait_or_fail(Svc::PhpServer, port, 5, Some(paths().logs().join("php-server.log")))
}
