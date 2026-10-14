//! php-cgi FastCGI havuzu (Nginx ve NTS PHP ile Apache için). Laragon ile aynı model:
//! 127.0.0.1:9001.. portlarında N ayrı php-cgi süreci; ölen süreç izleyici tarafından yeniden başlatılır.

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

pub fn ensure() -> Result<()> {
    let exe = crate::php::cgi_exe().ok_or_else(|| anyhow!(tf("msg.not_installed", &["php-cgi"])))?;
    crate::php::ensure_ini()?;
    let ini_dir = crate::php::ini_dir().unwrap();
    write_upstream_conf()?;
    let mut env = service_env(&[]);
    // Windows'ta php-cgi kendi alt süreçlerini açamaz; istek sınırı olmasın ki süreç kendiliğinden kapanmasın
    env.push(("PHP_FCGI_MAX_REQUESTS".into(), "0".into()));
    env.push(("PHPRC".into(), ini_dir.display().to_string()));
    for i in 0..count() {
        let port = BASE_PORT + i;
        let key = format!("php-cgi-{port}");
        if tracked_alive(&key).is_some() {
            continue;
        }
        if util::port_open(port) {
            // Başka bir php-cgi (ör. önceki oturumdan) zaten dinliyor
            continue;
        }
        let args = vec!["-b".to_string(), format!("127.0.0.1:{port}"), "-c".to_string(), ini_dir.display().to_string()];
        let child = util::spawn_bg(&exe, &args, None, &env, Some(&paths().logs().join("php-cgi.log")))?;
        track(&key, child);
    }
    Ok(())
}

pub fn stop_all() {
    for i in 0..32u16 {
        kill_tracked(&format!("php-cgi-{}", BASE_PORT + i));
    }
    if let Some(exe) = crate::php::cgi_exe() {
        super::kill_by_exe(&exe);
    }
}

pub fn needed() -> bool {
    let nginx = super::is_running(Svc::Nginx);
    let apache_fcgi = super::is_running(Svc::Apache)
        && crate::php::dir().map(|d| crate::php::apache_dll(&d).is_none()).unwrap_or(false);
    nginx || apache_fcgi
}

pub fn release_if_unused() {
    if !needed() {
        stop_all();
    }
}

/// Düşen php-cgi süreçlerini yeniden başlatır.
pub fn monitor() {
    let any_tracked = crate::kcore::core().procs.lock().keys().any(|k| k.starts_with("php-cgi-"));
    if any_tracked && needed() {
        let _ = ensure();
    }
}
