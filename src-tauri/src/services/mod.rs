//! Servis yönetimi. Her servis bin içindeki resmi binary'yi, etc altında üretilen
//! konfigürasyonla çalıştırır. Durum: port dinleniyor mu + dinleyen sürecin yolu.

pub mod apache;
pub mod mysql;
pub mod nginx;
pub mod others;
pub mod phpcgi;
pub mod winservice;

use crate::kcore::{core, emit, log, paths, settings};
use crate::i18n::{t, tf};
use crate::util;
use anyhow::{bail, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Child;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub enum Svc {
    Apache,
    Nginx,
    Mysql,
    Postgresql,
    Mongodb,
    Redis,
    Memcached,
    Mailpit,
    PhpServer,
}

impl Svc {
    pub const ALL: [Svc; 9] = [
        Svc::Apache,
        Svc::Nginx,
        Svc::Mysql,
        Svc::Postgresql,
        Svc::Mongodb,
        Svc::Redis,
        Svc::Memcached,
        Svc::Mailpit,
        Svc::PhpServer,
    ];

    /// Başlatma sırası: önce veri servisleri, sonra web sunucuları.
    pub const START_ORDER: [Svc; 9] = [
        Svc::Mysql,
        Svc::Postgresql,
        Svc::Mongodb,
        Svc::Redis,
        Svc::Memcached,
        Svc::Mailpit,
        Svc::Apache,
        Svc::Nginx,
        Svc::PhpServer,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Svc::Apache => "apache",
            Svc::Nginx => "nginx",
            Svc::Mysql => "mysql",
            Svc::Postgresql => "postgresql",
            Svc::Mongodb => "mongodb",
            Svc::Redis => "redis",
            Svc::Memcached => "memcached",
            Svc::Mailpit => "mailpit",
            Svc::PhpServer => "phpserver",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Svc::Apache => "Apache",
            Svc::Nginx => "Nginx",
            Svc::Mysql => "MySQL",
            Svc::Postgresql => "PostgreSQL",
            Svc::Mongodb => "MongoDB",
            Svc::Redis => "Redis",
            Svc::Memcached => "Memcached",
            Svc::Mailpit => "Mailpit",
            Svc::PhpServer => "PHP Server",
        }
    }

    /// bin altındaki kategori klasörü.
    pub fn category(self) -> &'static str {
        match self {
            Svc::PhpServer => "php",
            s => s.id(),
        }
    }

    pub fn from_id(id: &str) -> Option<Svc> {
        Svc::ALL.into_iter().find(|s| s.id() == id)
    }

    pub fn exe_name(self) -> &'static str {
        match self {
            Svc::Apache => "httpd.exe",
            Svc::Nginx => "nginx.exe",
            Svc::Mysql => "mysqld.exe",
            Svc::Postgresql => "postgres.exe",
            Svc::Mongodb => "mongod.exe",
            Svc::Redis => "redis-server.exe",
            Svc::Memcached => "memcached.exe",
            Svc::Mailpit => "mailpit.exe",
            Svc::PhpServer => "php.exe",
        }
    }

    pub fn exe(self) -> Option<PathBuf> {
        crate::versions::exe(self.category(), self.exe_name())
    }

    pub fn port(self) -> u16 {
        settings().svc(self.id()).port
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SvcStatus {
    pub id: String,
    pub title: String,
    pub installed: bool,
    pub version: Option<String>,
    pub enabled: bool,
    pub running: bool,
    pub port: u16,
    pub ssl: bool,
    pub ssl_port: u16,
    pub pid: Option<u32>,
    /// Port başka bir uygulama tarafından tutuluyorsa onun yolu.
    pub foreign: Option<String>,
}

pub fn statuses() -> Vec<SvcStatus> {
    let ls = util::listeners();
    let s = settings();
    Svc::ALL
        .iter()
        .map(|&svc| {
            let cfg = s.svc(svc.id());
            let exe = svc.exe();
            let pid = ls.get(&cfg.port).copied();
            let mut running = pid.is_some();
            let mut foreign = None;
            if let Some(pid) = pid {
                if let Some(path) = util::process_path(pid) {
                    if !is_ours(&path, exe.as_deref()) {
                        running = false;
                        foreign = Some(path.display().to_string());
                    }
                }
            }
            SvcStatus {
                id: svc.id().into(),
                title: svc.title().into(),
                installed: exe.is_some(),
                version: crate::versions::current(svc.category()),
                enabled: cfg.enabled,
                running,
                port: cfg.port,
                ssl: cfg.ssl,
                ssl_port: cfg.ssl_port,
                pid,
                foreign,
            }
        })
        .collect()
}

/// Dinleyen süreç bizim bin dizinimizden mi (veya bağlantı verilen hedefinden mi)?
fn is_ours(proc_path: &Path, expected: Option<&Path>) -> bool {
    let pp = proc_path.to_string_lossy().to_lowercase();
    let bin = paths().bin().to_string_lossy().to_lowercase();
    if pp.starts_with(&bin) {
        return true;
    }
    if let Some(e) = expected {
        if let Ok(real) = std::fs::canonicalize(e) {
            let real = crate::paths::strip_unc(real).to_string_lossy().to_lowercase();
            if real == pp {
                return true;
            }
            // Aynı sürüm klasöründen başka bir exe (ör. mysqld yerine mariadbd)
            if let Some(dir) = Path::new(&real).parent().and_then(|p| p.parent()) {
                if pp.starts_with(&dir.to_string_lossy().to_string()) {
                    return true;
                }
            }
        }
    }
    false
}

pub fn is_running(svc: Svc) -> bool {
    util::port_open(svc.port())
}

/// Servisin portunu dinleyen süreç bizim bin dizinimizden mi?
pub fn owned_by_us(svc: Svc) -> bool {
    let Some(pid) = util::listeners().get(&svc.port()).copied() else { return false };
    util::process_path(pid).map(|p| is_ours(&p, svc.exe().as_deref())).unwrap_or(true)
}

pub fn track(key: &str, child: Child) {
    core().procs.lock().insert(key.to_string(), child);
}

/// Takip edilen süreç hâlâ çalışıyorsa PID'i.
pub fn tracked_alive(key: &str) -> Option<u32> {
    let mut procs = core().procs.lock();
    let child = procs.get_mut(key)?;
    match child.try_wait() {
        Ok(None) => Some(child.id()),
        _ => {
            procs.remove(key);
            None
        }
    }
}

pub fn kill_tracked(key: &str) {
    if let Some(mut child) = core().procs.lock().remove(key) {
        util::kill_tree(child.id());
        let _ = child.wait();
    }
}

/// Servise ait (aynı exe yolundan çalışan) tüm süreçleri sonlandırır.
pub fn kill_by_exe(exe: &Path) {
    for pid in util::pids_by_exe(exe, &paths().root) {
        util::kill_tree(pid);
    }
}

/// Servislerin ortak PATH'i: seçili PHP + servisin kendi bin dizini + sistem PATH.
pub fn service_env(extra_dirs: &[PathBuf]) -> Vec<(String, String)> {
    let mut parts: Vec<String> = extra_dirs.iter().map(|d| d.display().to_string()).collect();
    if let Some(p) = crate::php::env_path() {
        parts.push(p);
    }
    parts.push(std::env::var("PATH").unwrap_or_default());
    vec![("PATH".into(), parts.join(";"))]
}

/// Web sunucularının dinleyeceği adresler. Varsayılan yalnız bu bilgisayar (127.0.0.1 ve varsa ::1);
/// Tercihler'de "yerel ağdan erişim" açıksa tüm arabirimler (yalnız port).
pub fn listen_addrs(port: u16) -> Vec<String> {
    if settings().lan_access {
        return vec![port.to_string()];
    }
    let mut v = vec![format!("127.0.0.1:{port}")];
    if ipv6_loopback() {
        v.push(format!("[::1]:{port}"));
    }
    v
}

/// IPv6 devre dışıysa [::1] dinlemek sunucunun açılmasını engeller.
fn ipv6_loopback() -> bool {
    static V6: once_cell::sync::Lazy<bool> = once_cell::sync::Lazy::new(|| std::net::TcpListener::bind("[::1]:0").is_ok());
    *V6
}

/// Nginx `listen` satırları (ör. `ssl default_server` ekiyle).
pub fn nginx_listen(port: u16, suffix: &str) -> String {
    listen_addrs(port).iter().map(|a| format!("    listen {a}{suffix};")).collect::<Vec<_>>().join("\n")
}

/// Portun boş olduğunu doğrular; doluysa sahibini söyleyen hata.
pub fn ensure_port_free(svc: Svc, port: u16) -> Result<()> {
    if !util::port_open(port) {
        return Ok(());
    }
    let owner = util::listeners()
        .get(&port)
        .and_then(|pid| util::process_path(*pid).map(|p| format!("{} (PID {pid})", p.display())))
        .unwrap_or_else(|| "?".into());
    bail!(tf("msg.port_in_use", &[svc.title(), &port.to_string(), &owner]))
}

/// Servis işlem kilidi. Takılan bir işlem diğerlerini sonsuza kadar bekletmesin.
fn op_guard() -> Result<parking_lot::ReentrantMutexGuard<'static, ()>> {
    core()
        .op_lock
        .try_lock_for(std::time::Duration::from_secs(120))
        .ok_or_else(|| anyhow::anyhow!(t("msg.op_busy")))
}

pub fn start(svc: Svc) -> Result<()> {
    let _g = op_guard()?;
    if svc.exe().is_none() {
        bail!(tf("msg.not_installed", &[svc.title()]));
    }
    if is_running(svc) {
        // Port açık: bizim sürecimizse zaten çalışıyor, değilse sahibini bildir
        if owned_by_us(svc) {
            return Ok(());
        }
        ensure_port_free(svc, svc.port())?;
    }
    crate::kcore::status(&tf("msg.starting", &[svc.title()]));
    if winservice::installed(svc) {
        return winservice::start(svc);
    }
    match svc {
        Svc::Apache => apache::start(),
        Svc::Nginx => nginx::start(),
        Svc::Mysql => mysql::start(),
        Svc::PhpServer => others::phpserver_start(),
        Svc::Postgresql => others::postgres_start(),
        Svc::Mongodb => others::mongo_start(),
        Svc::Redis => others::redis_start(),
        Svc::Memcached => others::memcached_start(),
        Svc::Mailpit => others::mailpit_start(),
    }?;
    log(&format!("{} {}", svc.title(), t("msg.started")));
    Ok(())
}

pub fn stop(svc: Svc) -> Result<()> {
    let _g = op_guard()?;
    crate::kcore::status(&tf("msg.stopping", &[svc.title()]));
    if winservice::installed(svc) {
        return winservice::stop(svc);
    }
    match svc {
        Svc::Apache => apache::stop(),
        Svc::Nginx => nginx::stop(),
        Svc::Mysql => mysql::stop(),
        Svc::Postgresql => others::postgres_stop(),
        other => {
            kill_tracked(other.id());
            if let Some(exe) = other.exe() {
                if util::port_open(other.port()) {
                    kill_by_exe(&exe);
                }
            }
            Ok(())
        }
    }?;
    if matches!(svc, Svc::Apache | Svc::Nginx) {
        phpcgi::release_if_unused();
    }
    Ok(())
}

pub fn reload(svc: Svc) -> Result<()> {
    let _g = op_guard()?;
    match svc {
        Svc::Nginx if is_running(svc) => nginx::reload(),
        _ => {
            if is_running(svc) {
                stop(svc)?;
            }
            start(svc)
        }
    }
}

/// Web sunucularının ihtiyaç duyduğu ortak hazırlık (vhost + php.ini + SSL).
pub fn prepare_web() {
    if crate::php::dir().is_some() {
        if let Err(e) = crate::php::ensure_ini() {
            crate::kcore::error("php.ini", &e);
        }
    }
    if let Err(e) = crate::vhosts::sync() {
        crate::kcore::error(&t("menu.vhosts"), &e);
    }
}

pub fn start_all() -> Vec<String> {
    let Ok(_g) = op_guard() else { return vec![t("msg.op_busy")] };
    let s = settings();
    let mut errors = Vec::new();
    let wants_web = Svc::ALL.iter().any(|v| matches!(v, Svc::Apache | Svc::Nginx) && s.svc(v.id()).enabled);
    if wants_web {
        prepare_web();
    }
    for svc in Svc::START_ORDER {
        if !s.svc(svc.id()).enabled || svc.exe().is_none() {
            continue;
        }
        if let Err(e) = start(svc) {
            let msg = format!("{}: {e:#}", svc.title());
            log(&msg);
            errors.push(msg);
        }
    }
    broadcast();
    errors
}

pub fn stop_all() {
    let _g = op_guard();
    for svc in Svc::START_ORDER.iter().rev() {
        if is_running(*svc) || core().procs.lock().contains_key(svc.id()) {
            if let Err(e) = stop(*svc) {
                log(&format!("{}: {e:#}", svc.title()));
            }
        }
    }
    phpcgi::stop_all();
    crate::procfile::stop_all();
    broadcast();
}

/// Arayüze ve menüye güncel durumu bildirir.
pub fn broadcast() {
    let st = statuses();
    emit("services", st.clone());
    crate::menu::refresh(&st);
}
