//! Apache: etc\apache2\httpd.conf üretilir, `httpd -f` ile çalıştırılır. bin içine yazılmaz.

use super::{ensure_port_free, kill_by_exe, kill_tracked, service_env, track, Svc};
use crate::kcore::{paths, settings};
use crate::i18n::tf;
use crate::paths::fwd;
use crate::templates::render_named;
use crate::util::{self, hidden, run_capture};
use anyhow::{anyhow, bail, Result};
use std::path::PathBuf;
use std::time::Duration;

pub fn exe() -> Option<PathBuf> {
    Svc::Apache.exe()
}

pub fn conf_path() -> PathBuf {
    paths().etc().join("apache2").join("httpd.conf")
}

pub fn error_log() -> PathBuf {
    paths().logs().join("apache_error.log")
}

pub fn access_log() -> PathBuf {
    paths().logs().join("apache_access.log")
}

/// Tüm Apache konfigürasyonunu yazar. php-cgi havuzu gerekiyorsa true döner.
pub fn write_conf() -> Result<bool> {
    let exe = exe().ok_or_else(|| anyhow!(tf("msg.not_installed", &["Apache"])))?;
    let server_root = exe.parent().and_then(|p| p.parent()).ok_or_else(|| anyhow!("ServerRoot"))?.to_path_buf();
    let p = paths();
    let s = settings();
    let cfg = s.svc("apache");
    let a2 = p.etc().join("apache2");

    let fcgi = crate::php::write_apache_php_conf()?;

    let ssl_conf = a2.join("ssl.conf");
    let default_conf = a2.join("sites-enabled").join("00-default.conf");
    let doc_root = fwd(std::path::Path::new(&s.document_root));
    let common = vec![
        ("KD_ROOT", fwd(&p.root)),
        ("DOC_ROOT", doc_root.clone()),
        ("PORT", cfg.port.to_string()),
        ("SSL_PORT", cfg.ssl_port.to_string()),
        ("SSL_DIR", fwd(&p.ssl())),
        ("LOG_DIR", fwd(&p.logs())),
        ("SERVER_ROOT", fwd(&server_root)),
    ];
    if cfg.ssl {
        crate::ssl::ensure_ca()?;
        crate::ssl::ensure_host_cert("localhost")?;
        util::write_if_changed(&ssl_conf, &render_named("apache.ssl.tpl", &common))?;
        util::write_if_changed(&default_conf, &render_named("apache.default.ssl.tpl", &common))?;
    } else {
        let _ = std::fs::remove_file(&ssl_conf);
        util::write_if_changed(&default_conf, &render_named("apache.default.tpl", &common))?;
    }
    util::write_if_changed(&conf_path(), &render_named("httpd.conf.tpl", &common))?;
    Ok(fcgi)
}

/// `httpd -t`: konfigürasyon sözdizimi denetimi.
pub fn test_conf() -> Result<()> {
    let exe = exe().ok_or_else(|| anyhow!(tf("msg.not_installed", &["Apache"])))?;
    let mut c = hidden(&exe);
    c.args(["-t", "-f"]).arg(conf_path()).envs(env());
    let (code, out) = run_capture(c)?;
    if code != 0 {
        bail!("{}", out.trim());
    }
    Ok(())
}

fn env() -> Vec<(String, String)> {
    let bin = exe().and_then(|e| e.parent().map(|p| p.to_path_buf())).unwrap_or_default();
    service_env(&[bin])
}

pub fn start() -> Result<()> {
    let s = settings();
    let cfg = s.svc("apache");
    ensure_port_free(Svc::Apache, cfg.port)?;
    if cfg.ssl {
        ensure_port_free(Svc::Apache, cfg.ssl_port)?;
    }
    let fcgi = write_conf()?;
    if fcgi {
        super::phpcgi::ensure()?;
    }
    test_conf()?;
    let exe = exe().unwrap();
    let args = vec!["-f".to_string(), conf_path().display().to_string()];
    let child = util::spawn_bg(&exe, &args, exe.parent().and_then(|p| p.parent()), &env(), Some(&paths().logs().join("apache_stdout.log")))?;
    track("apache", child);
    if !util::wait_port(cfg.port, true, Duration::from_secs(10)) {
        let tail = util::tail(&error_log(), 8);
        kill_tracked("apache");
        bail!("{}\n{}", tf("msg.start_failed", &["Apache"]), tail);
    }
    Ok(())
}

pub fn stop() -> Result<()> {
    kill_tracked("apache");
    if let Some(exe) = exe() {
        kill_by_exe(&exe);
    }
    util::wait_port(settings().svc("apache").port, false, Duration::from_secs(5));
    let _ = std::fs::remove_file(paths().logs().join("httpd.pid"));
    Ok(())
}
