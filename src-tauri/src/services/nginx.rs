//! Nginx: `nginx -p tmp\nginx\ -c etc\nginx\nginx.conf`. PHP isteklerini php-cgi havuzuna iletir.

use super::{ensure_port_free, kill_by_exe, kill_tracked, track, Svc};
use crate::kcore::{paths, settings};
use crate::i18n::tf;
use crate::paths::fwd;
use crate::templates::render_named;
use crate::util::{self, hidden, run_capture};
use anyhow::{anyhow, bail, Result};
use std::path::PathBuf;
use std::time::Duration;

pub fn exe() -> Option<PathBuf> {
    Svc::Nginx.exe()
}

pub fn conf_path() -> PathBuf {
    paths().etc().join("nginx").join("nginx.conf")
}

fn prefix() -> String {
    format!("{}/", fwd(&paths().tmp().join("nginx")))
}

/// mime.types ve fastcgi.conf'un bulunduğu klasör: Windows'ta `nginx/conf`, Homebrew'da `<önek>/etc/nginx`.
pub fn bin_conf_dir() -> Option<PathBuf> {
    let exe = exe()?;
    let conf = exe.parent()?.join("conf");
    #[cfg(target_os = "macos")]
    if !conf.join("mime.types").exists() {
        return crate::platform::cellar_prefix(&exe)
            .or_else(crate::platform::brew_prefix)
            .map(|p| p.join("etc").join("nginx"))
            .filter(|d| d.join("mime.types").is_file());
    }
    Some(conf)
}

pub fn write_conf() -> Result<()> {
    let conf_dir = bin_conf_dir().ok_or_else(|| anyhow!(tf("msg.not_installed", &["Nginx"])))?;
    let p = paths();
    let s = settings();
    let cfg = s.svc("nginx");
    let ng = p.etc().join("nginx");
    let _ = std::fs::create_dir_all(p.tmp().join("nginx").join("logs"));
    let _ = std::fs::create_dir_all(p.tmp().join("nginx").join("temp"));
    let (ssl_listen, ssl_cert) = if cfg.ssl {
        crate::ssl::ensure_ca()?;
        crate::ssl::ensure_host_cert("localhost")?;
        (
            super::nginx_listen(cfg.ssl_port, " ssl default_server"),
            format!(
                "    ssl_certificate \"{0}/localhost.crt\";\n    ssl_certificate_key \"{0}/localhost.key\";\n    ssl_protocols TLSv1.2 TLSv1.3;\n",
                fwd(&p.ssl())
            ),
        )
    } else {
        (String::new(), String::new())
    };
    let vars = vec![
        ("KD_ROOT", fwd(&p.root)),
        ("DOC_ROOT", fwd(std::path::Path::new(&s.document_root))),
        ("PORT", cfg.port.to_string()),
        ("LISTEN", super::nginx_listen(cfg.port, " default_server")),
        ("SSL_PORT", cfg.ssl_port.to_string()),
        ("LOG_DIR", fwd(&p.logs())),
        ("NGINX_CONF", fwd(&conf_dir)),
        ("SSL_LISTEN", ssl_listen),
        ("SSL_CERT", ssl_cert),
        ("LOCAL_ONLY", local_only_block(&[cfg.port, cfg.ssl_port])),
    ];
    util::write_if_changed(&conf_path(), &render_named("nginx.conf.tpl", &vars))?;
    util::write_if_changed(&ng.join("fastcgi-php.conf"), &render_named("nginx.fastcgi-php.tpl", &vars))?;
    util::write_if_changed(&ng.join("sites-enabled").join("00-default.conf"), &render_named("nginx.default.tpl", &vars))?;
    super::phpcgi::write_upstream_conf()?;
    Ok(())
}

/// macOS'ta 1024 altı portlar tüm arabirimlerde dinlenir; yerel ağdan erişim kapalıysa yalnız bu bilgisayar.
fn local_only_block(ports: &[u16]) -> String {
    if !super::local_only(ports) {
        return String::new();
    }
    "\n    # Yerel ağdan erişim kapalı (Tercihler > Servisler ve Portlar): yalnız bu bilgisayar\n    allow 127.0.0.1;\n    allow ::1;\n    deny all;\n".into()
}

fn nginx_cmd(extra: &[&str]) -> Result<std::process::Command> {
    let exe = exe().ok_or_else(|| anyhow!(tf("msg.not_installed", &["Nginx"])))?;
    let mut c = hidden(&exe);
    c.arg("-p").arg(prefix()).arg("-c").arg(conf_path()).args(extra);
    c.current_dir(exe.parent().unwrap());
    Ok(c)
}

pub fn test_conf() -> Result<()> {
    let (code, out) = run_capture(nginx_cmd(&["-t"])?)?;
    if code != 0 {
        bail!("{}", out.trim());
    }
    Ok(())
}

pub fn start() -> Result<()> {
    let cfg = settings().svc("nginx");
    ensure_port_free(Svc::Nginx, cfg.port)?;
    if cfg.ssl {
        ensure_port_free(Svc::Nginx, cfg.ssl_port)?;
    }
    write_conf()?;
    if crate::php::dir().is_some() {
        super::phpcgi::ensure()?;
    }
    super::phpcgi::ensure_extra()?;
    test_conf()?;
    let exe = exe().unwrap();
    let args = vec!["-p".into(), prefix(), "-c".into(), conf_path().display().to_string()];
    let child = util::spawn_bg(&exe, &args, exe.parent(), &[], None)?;
    track("nginx", child);
    if !util::wait_port(cfg.port, true, Duration::from_secs(8)) {
        let tail = util::tail(&paths().logs().join("nginx_error.log"), 8);
        stop()?;
        bail!("{}\n{}", tf("msg.start_failed", &["Nginx"]), tail);
    }
    Ok(())
}

pub fn reload() -> Result<()> {
    write_conf()?;
    test_conf()?;
    let (code, out) = run_capture(nginx_cmd(&["-s", "reload"])?)?;
    if code != 0 {
        bail!("{}", out.trim());
    }
    Ok(())
}

pub fn stop() -> Result<()> {
    if let Ok(c) = nginx_cmd(&["-s", "stop"]) {
        let _ = run_capture(c);
    }
    let port = settings().svc("nginx").port;
    if !util::wait_port(port, false, Duration::from_secs(4)) {
        if let Some(exe) = exe() {
            kill_by_exe(&exe);
        }
    }
    kill_tracked("nginx");
    Ok(())
}
