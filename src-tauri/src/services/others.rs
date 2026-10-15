//! PostgreSQL, MongoDB, Redis, Memcached, Mailpit ve PHP yerleşik sunucusu.

use super::{ensure_port_free, kill_tracked, service_env, track, Svc};
use crate::kcore::{paths, settings};
use crate::i18n::{t, tf};
use crate::paths::fwd;
use crate::platform::exe;
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
        let mut c = hidden(bin.join(exe("initdb")));
        c.arg("-D").arg(&data).args(["-U", "postgres", "-A", "trust", "-E", "UTF8", "--locale=C"]);
        let (code, out) = util::run_logged(c, &pg_log(), Duration::from_secs(180))?;
        if code != 0 {
            bail!("initdb: {}", out.lines().rev().take(6).collect::<Vec<_>>().join("\n"));
        }
    }
    let mut c = hidden(bin.join(exe("pg_ctl")));
    c.arg("-D").arg(&data).arg("-l").arg(pg_log()).arg("-o").arg(pg_server_opts(port)).args(["-w", "start"]);
    // pg_ctl, postgres'i arka planda başlatır; çıktısı boruya bağlanırsa okuma hiç bitmez
    let (code, out) = util::run_logged(c, &paths().logs().join("pg_ctl.log"), Duration::from_secs(90))?;
    if code != 0 {
        bail!("pg_ctl: {}\n{}", out.trim(), util::tail(&pg_log(), 6));
    }
    wait_or_fail(Svc::Postgresql, port, 10, Some(pg_log()))
}

/// postgres sunucu seçenekleri. macOS'ta Unix soketi /tmp yerine kökte: aynı portta başka bir PostgreSQL'in
/// (ör. Homebrew) kilit dosyasıyla çakışmaz.
pub fn pg_server_opts(port: u16) -> String {
    if cfg!(windows) {
        format!("-p {port}")
    } else {
        format!("-p {port} -k \"{}\"", pg_socket_dir().display())
    }
}

/// Unix soketinin klasörü: kökteki tmp; soket yolu (`<klasör>/.s.PGSQL.<port>`) 103 karakteri aşacaksa /tmp.
pub fn pg_socket_dir() -> PathBuf {
    let t = paths().tmp();
    if t.as_os_str().len() + 18 < 100 { t } else { PathBuf::from("/tmp") }
}

/// Veri dizini anahtarı (ör. "postgresql-18.2"); parola bu anahtarla saklanır.
pub fn pg_key() -> Option<String> {
    versions::current("postgresql").map(|v| versions::major_minor_key(&v, "postgresql"))
}

/// Seçili sürümün veri dizinindeki `postgres` kullanıcısının parolası (boş = trust, parolasız).
pub fn pg_password() -> String {
    pg_key().and_then(|k| settings().pg_passwords.get(&k).cloned()).unwrap_or_default()
}

/// pg_hba.conf'taki tüm `local`/`host*` kurallarının kimlik doğrulama yöntemini değiştirir.
pub fn set_hba_method(content: &str, method: &str) -> String {
    const METHODS: &[&str] = &["trust", "md5", "scram-sha-256", "password"];
    let mut out = String::with_capacity(content.len());
    for line in content.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        let first = body.split_whitespace().next().unwrap_or("");
        let last = body.split_whitespace().last().unwrap_or("");
        if (first == "local" || first.starts_with("host")) && METHODS.contains(&last) {
            let cut = body.rfind(last).unwrap();
            out.push_str(&body[..cut]);
            out.push_str(method);
            out.push_str(&line[body.len()..]);
        } else {
            out.push_str(line);
        }
    }
    out
}

/// `postgres` parolasını değiştirir. Boş parola: kimlik doğrulama `trust` (varsayılan, parolasız MySQL root ile aynı);
/// parola verilirse yerel bağlantılar `scram-sha-256` ile parola ister.
pub fn pg_set_password(new: &str) -> Result<()> {
    let port = settings().svc("postgresql").port;
    if !util::port_open(port) {
        bail!(tf("msg.start_first", &["PostgreSQL"]));
    }
    let bin = pg_bin().ok_or_else(|| anyhow!(tf("msg.not_installed", &["PostgreSQL"])))?;
    let data = pg_data().unwrap();
    let lit = if new.is_empty() { "NULL".to_string() } else { format!("'{}'", new.replace('\'', "''")) };
    let mut c = hidden(bin.join(exe("psql")));
    c.args(["-h", "127.0.0.1", "-p", &port.to_string(), "-U", "postgres", "-d", "postgres", "-w", "-v", "ON_ERROR_STOP=1"])
        .arg("-c")
        .arg(format!("ALTER USER postgres PASSWORD {lit}"))
        .env("PGPASSWORD", pg_password());
    let (code, out) = util::run_capture(c)?;
    if code != 0 {
        bail!("psql: {}", out.trim());
    }
    let hba = data.join("pg_hba.conf");
    let content = std::fs::read_to_string(&hba)?;
    std::fs::write(&hba, set_hba_method(&content, if new.is_empty() { "trust" } else { "scram-sha-256" }))?;
    let mut c = hidden(bin.join(exe("pg_ctl")));
    c.arg("-D").arg(&data).arg("reload");
    let (code, out) = util::run_logged(c, &paths().logs().join("pg_ctl.log"), Duration::from_secs(30))?;
    if code != 0 {
        bail!("pg_ctl reload: {}", out.trim());
    }
    let key = pg_key().unwrap_or_default();
    crate::kcore::update_settings(|s| {
        s.pg_passwords.insert(key, new.to_string());
    });
    Ok(())
}

pub fn postgres_stop() -> Result<()> {
    if let (Some(bin), Some(data)) = (pg_bin(), pg_data()) {
        let mut c = hidden(bin.join(exe("pg_ctl")));
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

#[cfg(test)]
mod tests {
    use super::set_hba_method;

    #[test]
    fn hba_methods() {
        let c = "# TYPE  DATABASE USER ADDRESS METHOD\r\nlocal   all  all                trust\r\nhost    all  all  127.0.0.1/32  trust\r\nhost    replication all ::1/128 trust\r\n# host all all 0.0.0.0/0 trust\r\n";
        let s = set_hba_method(c, "scram-sha-256");
        assert_eq!(s.matches("scram-sha-256").count(), 3);
        assert!(s.contains("# host all all 0.0.0.0/0 trust\r\n"));
        assert!(s.contains("host    all  all  127.0.0.1/32  scram-sha-256\r\n"));
        assert_eq!(set_hba_method(&s, "trust"), c);
    }
}
