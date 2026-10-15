//! php-cgi FastCGI havuzu (Nginx ve NTS PHP ile Apache için). Laragon ile aynı model:
//! 127.0.0.1:9001.. portlarında N ayrı php-cgi süreci; ölen süreç izleyici tarafından yeniden başlatılır.
//! Proje başına farklı PHP sürümü seçilmişse her ek sürüm için 9101.. portlarında birer php-cgi daha çalışır.

use super::{kill_tracked, service_env, track, tracked_alive, Svc};
use crate::kcore::{paths, settings};
use crate::i18n::tf;
use crate::util;
use anyhow::{anyhow, Result};

pub const BASE_PORT: u16 = 9001;

fn count() -> u16 {
    settings().php_upstream.clamp(1, 32) as u16
}

pub fn write_upstream_conf() -> Result<()> {
    let servers: Vec<String> = (0..count())
        .map(|i| format!("    server 127.0.0.1:{} weight=1 max_fails=1 fail_timeout=1;", BASE_PORT + i))
        .collect();
    let content = format!(
        "# Otomatik üretildi. PHP süreç sayısı: Tercihler > Servisler (usr\\karadelik.ini [nginx] Upstream)\nupstream php_upstream {{\n{}\n}}\n",
        servers.join("\n")
    );
    util::write_if_changed(&paths().etc().join("nginx").join("php_upstream.conf"), &content)?;
    Ok(())
}

fn spawn_cgi(exe: &std::path::Path, port: u16, ini_dir: &std::path::Path, env: &[(String, String)]) -> Result<()> {
    let key = format!("php-cgi-{port}");
    if tracked_alive(&key).is_some() || util::port_open(port) {
        // Çalışıyor (veya önceki oturumdan kalan bir php-cgi dinliyor)
        return Ok(());
    }
    let args = vec!["-b".to_string(), format!("127.0.0.1:{port}"), "-c".to_string(), ini_dir.display().to_string()];
    let child = util::spawn_bg(exe, &args, None, env, Some(&paths().logs().join("php-cgi.log")))?;
    track(&key, child);
    Ok(())
}

/// php-cgi ortamı: PHP klasörü PATH'te (eklenti DLL'leri), istek sınırı yok, php.ini klasörü PHPRC.
fn cgi_env(php_dir: &std::path::Path, ini_dir: &std::path::Path) -> Vec<(String, String)> {
    let path = format!("{};{};{}", php_dir.display(), php_dir.join("ext").display(), std::env::var("PATH").unwrap_or_default());
    vec![
        ("PATH".into(), path),
        // Windows'ta php-cgi kendi alt süreçlerini açamaz; istek sınırı olmasın ki süreç kendiliğinden kapanmasın
        ("PHP_FCGI_MAX_REQUESTS".into(), "0".into()),
        ("PHPRC".into(), ini_dir.display().to_string()),
    ]
}

pub fn ensure() -> Result<()> {
    let exe = crate::php::cgi_exe().ok_or_else(|| anyhow!(tf("msg.not_installed", &["php-cgi"])))?;
    crate::php::ensure_ini()?;
    let ini_dir = crate::php::ini_dir().unwrap();
    write_upstream_conf()?;
    let mut env = service_env(&[]);
    env.push(("PHP_FCGI_MAX_REQUESTS".into(), "0".into()));
    env.push(("PHPRC".into(), ini_dir.display().to_string()));
    for i in 0..count() {
        spawn_cgi(&exe, BASE_PORT + i, &ini_dir, &env)?;
    }
    Ok(())
}

fn extra_keys() -> Vec<String> {
    crate::kcore::core().procs.lock().keys().filter(|k| is_extra_key(k)).cloned().collect()
}

fn is_extra_key(k: &str) -> bool {
    k.strip_prefix("php-cgi-").and_then(|p| p.parse::<u16>().ok()).map(|p| p >= crate::projects::PHP_POOL_BASE).unwrap_or(false)
}

/// Proje başına seçilen ek PHP sürümlerinin php-cgi süreçleri; artık kullanılmayanları kapatır.
pub fn ensure_extra() -> Result<()> {
    let pools = crate::projects::php_pools();
    let wanted: Vec<String> = pools.iter().map(|(_, p)| format!("php-cgi-{p}")).collect();
    for key in extra_keys() {
        if !wanted.contains(&key) {
            kill_tracked(&key);
        }
    }
    for (ver, port) in pools {
        let dir = crate::php::dir_for(&ver);
        let ini = crate::php::ensure_ini_for(&ver)?;
        let ini_dir = ini.parent().unwrap().to_path_buf();
        // Port başka bir sürümün php-cgi'si tarafından tutuluyorsa (havuz sırası değişti) onu kapat
        if tracked_alive(&format!("php-cgi-{port}")).is_none() && util::port_open(port) {
            if let Some(pid) = util::listeners().get(&port).copied() {
                let ours = util::process_path(pid).map(|p| p == dir.join("php-cgi.exe")).unwrap_or(true);
                if !ours {
                    util::kill_tree(pid);
                    util::wait_port(port, false, std::time::Duration::from_secs(3));
                }
            }
        }
        spawn_cgi(&dir.join("php-cgi.exe"), port, &ini_dir, &cgi_env(&dir, &ini_dir))?;
    }
    Ok(())
}

fn stop_default() {
    for i in 0..32u16 {
        kill_tracked(&format!("php-cgi-{}", BASE_PORT + i));
    }
    if let Some(exe) = crate::php::cgi_exe() {
        super::kill_by_exe(&exe);
    }
}

pub fn stop_all() {
    stop_default();
    for key in extra_keys() {
        kill_tracked(&key);
    }
    // Ayarı sonradan kaldırılmış ek sürümler dahil, bu köke ait tüm php-cgi süreçleri
    for ver in crate::versions::installed("php") {
        super::kill_by_exe(&crate::php::dir_for(&ver).join("php-cgi.exe"));
    }
}

fn web_running() -> bool {
    super::is_running(Svc::Nginx) || super::is_running(Svc::Apache)
}

pub fn needed() -> bool {
    let nginx = super::is_running(Svc::Nginx);
    let apache_fcgi = super::is_running(Svc::Apache)
        && crate::php::dir().map(|d| crate::php::apache_dll(&d).is_none()).unwrap_or(false);
    nginx || apache_fcgi
}

pub fn release_if_unused() {
    if !web_running() {
        stop_all();
    } else if !needed() {
        stop_default();
    }
}

/// Düşen php-cgi süreçlerini yeniden başlatır.
pub fn monitor() {
    let any_tracked = crate::kcore::core().procs.lock().keys().any(|k| k.starts_with("php-cgi-") && !is_extra_key(k));
    if any_tracked && needed() {
        let _ = ensure();
    }
    if !extra_keys().is_empty() && web_running() {
        let _ = ensure_extra();
    }
}
