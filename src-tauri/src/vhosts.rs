//! Otomatik sanal host: Document Root altındaki her klasör -> {name}.test
//! Apache ve Nginx için `auto.<host>.conf`, yönetim panelleri için alias, hosts dosyası bloğu.

use crate::kcore::{log, paths, settings};
use crate::paths::fwd;
use crate::templates::render_named;
use crate::util::write_if_changed;
use anyhow::Result;
use regex::Regex;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize)]
pub struct Project {
    pub name: String,
    pub dir: PathBuf,
    /// Laravel/Symfony gibi projelerde `public` alt dizini.
    pub web_root: PathBuf,
    pub host: String,
}

pub fn valid_name(name: &str) -> bool {
    // Laragon: boşluk ve "_" geçersiz; hostname kuralı
    Regex::new(r"^[A-Za-z0-9]([A-Za-z0-9-]*[A-Za-z0-9])?(\.[A-Za-z0-9]([A-Za-z0-9-]*[A-Za-z0-9])?)*$")
        .unwrap()
        .is_match(name)
}

pub fn web_root(dir: &Path) -> PathBuf {
    let public = dir.join("public");
    if public.join("index.php").exists() || public.join("index.html").exists() {
        return public;
    }
    let web = dir.join("web");
    if web.join("app.php").exists() || web.join("index.php").exists() && dir.join("bin").join("console").exists() {
        return web;
    }
    dir.to_path_buf()
}

/// Document Root'taki geçerli projeler. Geçersiz adlar loglanır (Laragon ile aynı mesaj).
pub fn projects() -> Vec<Project> {
    let s = settings();
    let ignore = s.ignore_list();
    let root = PathBuf::from(&s.document_root);
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(&root) else { return out };
    for e in rd.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if ignore.contains(&name.to_lowercase()) {
            continue;
        }
        if !valid_name(&name) {
            continue;
        }
        out.push(Project { host: s.hostname(&name), web_root: web_root(&path), dir: path, name });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn invalid_folders() -> Vec<String> {
    let root = PathBuf::from(settings().document_root);
    fs::read_dir(root)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| !n.starts_with('.') && !valid_name(n))
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Default, Debug, Serialize, Clone)]
pub struct SyncReport {
    pub created: Vec<String>,
    pub removed: Vec<String>,
    pub hosts_changed: bool,
    pub changed: bool,
}

fn user_owned(dir: &Path, host: &str) -> bool {
    // "auto." öneki kaldırılmış dosya kullanıcıya aittir, dokunulmaz
    dir.join(format!("{host}.conf")).exists()
}

/// Tüm otomatik konfigürasyonu senkronize eder. Bir şey değiştiyse `changed` true olur.
pub fn sync() -> Result<SyncReport> {
    let s = settings();
    let p = paths();
    let mut rep = SyncReport::default();
    let a_dir = p.etc().join("apache2").join("sites-enabled");
    let n_dir = p.etc().join("nginx").join("sites-enabled");
    fs::create_dir_all(&a_dir)?;
    fs::create_dir_all(&n_dir)?;

    for name in invalid_folders() {
        log(&format!("Skip creating auto virtual host for \"{name}\" because the name is invalid."));
    }

    let projects = if s.auto_vhosts { projects() } else { vec![] };
    let apache = s.svc("apache");
    let nginx = s.svc("nginx");
    let any_ssl = apache.ssl || nginx.ssl;
    if any_ssl {
        crate::ssl::ensure_ca()?;
    }

    let cfgs = crate::projects::all();
    let pools = crate::projects::php_pools();
    let mut wanted: Vec<String> = Vec::new();
    for pr in &projects {
        let file = format!("auto.{}.conf", pr.host);
        wanted.push(file.clone());
        if any_ssl && crate::ssl::ensure_host_cert(&pr.host)? {
            rep.changed = true;
        }
        let cfg = cfgs.get(&pr.name.to_lowercase()).cloned().unwrap_or_default();
        // Proje profildekinden farklı bir PHP sürümü istiyorsa o sürümün php-cgi'sine gider
        let php_port = pools.iter().find(|(v, _)| *v == cfg.php).map(|(_, port)| *port);
        let base = vec![
            ("PROJECT_DIR", fwd(&pr.web_root)),
            ("HOSTNAME", pr.host.clone()),
            ("SSL_DIR", fwd(&p.ssl())),
            ("KD_ROOT", fwd(&p.root)),
            ("PROXY_PORT", cfg.proxy.to_string()),
        ];
        if !user_owned(&a_dir, &pr.host) {
            let mut v = base.clone();
            v.push(("PORT", apache.port.to_string()));
            v.push(("SSL_PORT", apache.ssl_port.to_string()));
            v.push((
                "PHP_HANDLER",
                php_port
                    .map(|port| {
                        // Windows'ta açıklama satırı olmadan (önceki sürümlerle aynı çıktı)
                        let fix: String = crate::templates::fcgi_script_fix("        ").lines().filter(|l| !l.trim_start().starts_with('#')).map(|l| format!("{l}\n")).collect();
                        format!("    <FilesMatch \"\\.php$\">\n        SetHandler \"proxy:balancer://kdphp{port}/\"\n{fix}    </FilesMatch>\n")
                    })
                    .unwrap_or_default(),
            ));
            let tpl = match (cfg.proxy > 0, apache.ssl) {
                (true, true) => "apache.proxy.ssl.tpl",
                (true, false) => "apache.proxy.tpl",
                (false, true) => "apache.vhost.ssl.tpl",
                (false, false) => "apache.vhost.tpl",
            };
            let target = a_dir.join(&file);
            let existed = target.exists();
            if write_if_changed(&target, &render_named(tpl, &v))? {
                rep.changed = true;
                if !existed {
                    rep.created.push(pr.host.clone());
                }
            }
        }
        if !user_owned(&n_dir, &pr.host) {
            let mut v = base.clone();
            v.push(("PORT", nginx.port.to_string()));
            v.push(("LISTEN", crate::services::nginx_listen(nginx.port, "")));
            let (listen, cert) = if nginx.ssl {
                (
                    crate::services::nginx_listen(nginx.ssl_port, " ssl"),
                    format!(
                        "    ssl_certificate \"{0}/{1}.crt\";\n    ssl_certificate_key \"{0}/{1}.key\";\n    ssl_protocols TLSv1.2 TLSv1.3;\n",
                        fwd(&p.ssl()),
                        pr.host
                    ),
                )
            } else {
                (String::new(), String::new())
            };
            v.push(("SSL_LISTEN", listen));
            v.push(("SSL_CERT", cert));
            v.push(("PHP_UPSTREAM", php_port.map(|port| format!("127.0.0.1:{port}")).unwrap_or_else(|| "php_upstream".into())));
            let tpl = if cfg.proxy > 0 { "nginx.proxy.tpl" } else { "nginx.server.tpl" };
            if write_if_changed(&n_dir.join(&file), &render_named(tpl, &v))? {
                rep.changed = true;
            }
        }
    }

    // Klasörü silinmiş projelerin auto.* dosyalarını kaldır
    for dir in [&a_dir, &n_dir] {
        for e in fs::read_dir(dir)?.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("auto.") && n.ends_with(".conf") && !wanted.contains(&n) {
                let _ = fs::remove_file(e.path());
                rep.changed = true;
                let host = n.trim_start_matches("auto.").trim_end_matches(".conf").to_string();
                if !rep.removed.contains(&host) {
                    rep.removed.push(host);
                }
            }
        }
    }

    if sync_aliases()? {
        rep.changed = true;
    }

    if s.auto_vhosts && s.manage_hosts {
        let mut hosts: Vec<String> = projects.iter().map(|p| p.host.clone()).collect();
        // Kullanıcının kendi (auto. öneksiz) vhost dosyalarındaki hostlar da hosts'a girsin
        hosts.extend(user_hosts(&a_dir));
        hosts.sort();
        hosts.dedup();
        match crate::hosts::sync(&hosts) {
            Ok(ch) => rep.hosts_changed = ch,
            Err(e) => log(&format!("hosts: {e:#}")),
        }
    }
    Ok(rep)
}

fn user_hosts(dir: &Path) -> Vec<String> {
    let re = Regex::new(r"(?mi)^\s*ServerName\s+([^\s:]+)").unwrap();
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("auto.") || n.starts_with("00-") || !n.ends_with(".conf") {
                continue;
            }
            if let Ok(c) = fs::read_to_string(e.path()) {
                for cap in re.captures_iter(&c) {
                    let h = cap[1].to_lowercase();
                    if h != "localhost" && h.contains('.') {
                        out.push(h);
                    }
                }
            }
        }
    }
    out
}

/// etc\apps altındaki her uygulama için /<ad> alias'ı (phpMyAdmin, Adminer, phpRedisAdmin...).
pub fn app_aliases() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(paths().apps()) {
        for e in rd.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let dir_name = e.file_name().to_string_lossy().to_string();
            let alias = match dir_name.to_lowercase().as_str() {
                "phpredisadmin" => "redis".to_string(),
                x if x.starts_with("phpmyadmin") && x != "phpmyadmin" && !x.contains('6') => "phpmyadmin".to_string(),
                x => x.to_string(),
            };
            if valid_name(&alias) {
                out.push((alias, e.path()));
            }
        }
    }
    out.sort();
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

fn sync_aliases() -> Result<bool> {
    let p = paths();
    let mut changed = false;
    let a_dir = p.etc().join("apache2").join("alias");
    let n_dir = p.etc().join("nginx").join("alias");
    fs::create_dir_all(&a_dir)?;
    fs::create_dir_all(&n_dir)?;
    let aliases = app_aliases();
    let nginx_conf = crate::services::nginx::bin_conf_dir().map(|d| fwd(&d)).unwrap_or_default();
    let mut wanted = Vec::new();
    for (alias, dir) in &aliases {
        let file = format!("auto.{alias}.conf");
        wanted.push(file.clone());
        let vars = vec![("ALIAS", alias.clone()), ("APP_DIR", fwd(dir)), ("NGINX_CONF", nginx_conf.clone())];
        if !a_dir.join(format!("{alias}.conf")).exists() {
            changed |= write_if_changed(&a_dir.join(&file), &render_named("apache.alias.tpl", &vars))?;
        }
        if !n_dir.join(format!("{alias}.conf")).exists() {
            changed |= write_if_changed(&n_dir.join(&file), &render_named("nginx.alias.tpl", &vars))?;
        }
    }
    for dir in [&a_dir, &n_dir] {
        for e in fs::read_dir(dir)?.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("auto.") && !wanted.contains(&n) {
                let _ = fs::remove_file(e.path());
                changed = true;
            }
        }
    }
    Ok(changed)
}

/// "Delete all [auto]" menüsü.
pub fn delete_all_auto() -> Result<usize> {
    let p = paths();
    let mut n = 0;
    for dir in [p.etc().join("apache2").join("sites-enabled"), p.etc().join("nginx").join("sites-enabled")] {
        for e in fs::read_dir(&dir)?.flatten() {
            if e.file_name().to_string_lossy().starts_with("auto.") {
                fs::remove_file(e.path())?;
                n += 1;
            }
        }
    }
    Ok(n)
}

/// Senkron sonrası çalışan web sunucularını yeniden yükler.
pub fn sync_and_reload() -> Result<SyncReport> {
    let rep = sync()?;
    if crate::services::is_running(crate::services::Svc::Apache) || crate::services::is_running(crate::services::Svc::Nginx) {
        crate::services::phpcgi::ensure_extra()?;
    }
    if rep.changed {
        use crate::services::{is_running, reload, Svc};
        for svc in [Svc::Apache, Svc::Nginx] {
            if is_running(svc) {
                reload(svc)?;
            }
        }
    }
    Ok(rep)
}

pub fn project_url(p: &Project) -> String {
    let s = settings();
    let a = s.svc("apache");
    let (port, ssl, ssl_port) = if a.enabled || !s.svc("nginx").enabled {
        (a.port, a.ssl, a.ssl_port)
    } else {
        let n = s.svc("nginx");
        (n.port, n.ssl, n.ssl_port)
    };
    if ssl {
        if ssl_port == 443 { format!("https://{}", p.host) } else { format!("https://{}:{ssl_port}", p.host) }
    } else if port == 80 {
        format!("http://{}", p.host)
    } else {
        format!("http://{}:{port}", p.host)
    }
}

pub fn localhost_url() -> String {
    let s = settings();
    let a = s.svc("apache");
    let port = if a.enabled || !s.svc("nginx").enabled { a.port } else { s.svc("nginx").port };
    if port == 80 { "http://localhost".into() } else { format!("http://localhost:{port}") }
}

#[cfg(test)]
mod tests {
    use super::valid_name;

    #[test]
    fn names() {
        assert!(valid_name("karadelik"));
        assert!(valid_name("my-site"));
        assert!(valid_name("PATTES"));
        assert!(!valid_name("Yeni klasör"));
        assert!(!valid_name("my_site"));
        assert!(!valid_name("-bad"));
    }
}
