//! Hızlı uygulama (Laragon "Quick app"): usr\sites.conf şablonlarından proje oluşturur.
//! URL ise indirip açar, komut ise terminalde çalıştırır ve bitince `--post-create` ile .env + veritabanı ayarlar.

use crate::kcore::{log, paths, settings};
use crate::i18n::{t, tf};
use crate::services::{is_running, Svc};
use crate::util;
use crate::vhosts::{self, valid_name};
use anyhow::{bail, Result};
use regex::Regex;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize)]
pub struct Template {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Manifest {
    pub auto_create_db: bool,
    pub templates: Vec<Template>,
}

pub fn manifest_path() -> PathBuf {
    paths().usr().join("sites.conf")
}

pub fn manifest() -> Manifest {
    crate::packages::ensure_defaults();
    let text = fs::read_to_string(manifest_path()).unwrap_or_default();
    let mut m = Manifest { auto_create_db: true, templates: vec![] };
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with(';') || l.starts_with("---") {
            continue;
        }
        let Some((k, v)) = l.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "AutoCreateDatabase" => m.auto_create_db = v.eq_ignore_ascii_case("true") || v == "1",
            "Cached" => {}
            _ => m.templates.push(Template { name: k.to_string(), value: v.to_string() }),
        }
    }
    m
}

fn project_dir(name: &str) -> PathBuf {
    PathBuf::from(settings().document_root).join(name)
}

fn db_name(project: &str) -> String {
    project.replace(['-', '.'], "_")
}

fn check_new(name: &str) -> Result<PathBuf> {
    if name.trim().is_empty() {
        bail!(t("msg.specify_name"));
    }
    if !valid_name(name) {
        bail!(t("msg.invalid_project_name"));
    }
    let dir = project_dir(name);
    if dir.exists() {
        bail!("{}\n{}", t("msg.project_exists"), t("msg.project_exists_hint"));
    }
    Ok(dir)
}

fn maybe_create_db(name: &str) -> Option<String> {
    if !manifest().auto_create_db {
        return None;
    }
    if !is_running(Svc::Mysql) {
        return Some(t("msg.autodb_not_running"));
    }
    match crate::mysqlops::create_database(&db_name(name)) {
        Ok(_) => Some(tf("msg.db_created", &[&db_name(name)])),
        Err(e) => Some(tf("msg.db_create_failed", &[&db_name(name), &e.to_string()])),
    }
}

/// Proje oluşturur. Komut tabanlı şablonlarda terminal açılır ve işlem orada devam eder.
pub fn create(template: &str, name: &str) -> Result<String> {
    let dir = check_new(name)?;
    let tpl = manifest().templates.into_iter().find(|t| t.name == template).ok_or_else(|| anyhow::anyhow!("{template}?"))?;
    let value = tpl.value.trim().to_string();
    if value.starts_with("http://") || value.starts_with("https://") {
        crate::kcore::status(&tf("progress.creating", &[name]));
        crate::packages::fetch_into(&value, &dir, name, None, None)?;
        configure_wordpress(name)?;
    } else if value.is_empty() {
        fs::create_dir_all(&dir)?;
    } else {
        let cmd = value.replace("%s", name);
        let exe = std::env::current_exe()?;
        let chain = if cfg!(windows) {
            format!("echo. & echo [KaradelikDev] {} & {cmd} && \"{}\" --post-create {name}", tf("progress.creating", &[name]), exe.display())
        } else {
            let q = |s: &str| crate::platform::sh_quote(s);
            format!(
                "echo; echo {}; {cmd} && {} --post-create {name}",
                q(&format!("[KaradelikDev] {}", tf("progress.creating", &[name]))),
                q(&exe.display().to_string())
            )
        };
        crate::terminal::open(&PathBuf::from(settings().document_root), Some(&chain))?;
        return Ok(tf("msg.creating_in_terminal", &[name]));
    }
    let mut msg = vec![tf("msg.project_created", &[name])];
    if let Some(m) = maybe_create_db(name) {
        msg.push(m);
    }
    vhosts::sync_and_reload()?;
    Ok(msg.join("\n"))
}

/// WordPress: wp-config.php'yi veritabanı bilgileri ve rastgele anahtarlarla oluşturur.
fn configure_wordpress(name: &str) -> Result<()> {
    let dir = project_dir(name);
    let sample = dir.join("wp-config-sample.php");
    let target = dir.join("wp-config.php");
    if !sample.exists() || target.exists() {
        return Ok(());
    }
    let s = settings();
    let mut c = fs::read_to_string(&sample)?;
    c = c
        .replace("database_name_here", &db_name(name))
        .replace("username_here", "root")
        .replace("password_here", &crate::mysqlops::root_password().replace('\'', "\\'"));
    if s.svc("mysql").port != 3306 {
        c = c.replace("'localhost'", &format!("'127.0.0.1:{}'", s.svc("mysql").port));
    }
    while c.contains("put your unique phrase here") {
        c = c.replacen("put your unique phrase here", &util::random_password(64), 1);
    }
    fs::write(target, c)?;
    Ok(())
}

/// .env içinde `KEY=...` (yorumlu olsa da) satırını ayarlar.
pub fn set_env(content: &str, key: &str, value: &str) -> String {
    let re = Regex::new(&format!(r"(?m)^[ \t]*#?[ \t]*{}=.*$", regex::escape(key))).unwrap();
    if re.is_match(content) {
        re.replace(content, format!("{key}={value}").as_str()).to_string()
    } else {
        format!("{}\n{key}={value}\n", content.trim_end())
    }
}

/// Komutla oluşturulan projeden sonra (ayrı süreçte) çalışır: .env + veritabanı + vhost.
pub fn post_create(name: &str) -> Result<()> {
    let dir = project_dir(name);
    if !dir.exists() {
        bail!(tf("msg.create_failed", &[name, "?"]));
    }
    let s = settings();
    let host = s.hostname(name);
    let url = if s.svc("apache").ssl { format!("https://{host}") } else { format!("http://{host}") };
    let env = dir.join(".env");
    if env.exists() {
        let mut c = fs::read_to_string(&env)?;
        c = set_env(&c, "APP_URL", &url);
        if manifest().auto_create_db {
            for (k, v) in [
                ("DB_CONNECTION", "mysql".to_string()),
                ("DB_HOST", "127.0.0.1".to_string()),
                ("DB_PORT", s.svc("mysql").port.to_string()),
                ("DB_DATABASE", db_name(name)),
                ("DB_USERNAME", "root".to_string()),
                ("DB_PASSWORD", crate::mysqlops::root_password()),
            ] {
                c = set_env(&c, k, &v);
            }
            // Symfony
            if c.contains("DATABASE_URL=") {
                let re = Regex::new(r#"(?m)^[ \t]*#?[ \t]*DATABASE_URL=.*mysql.*$"#).unwrap();
                let root_pw = crate::mysqlops::root_password();
                let pw = if root_pw.is_empty() { String::new() } else { format!(":{root_pw}") };
                let line = format!(
                    "DATABASE_URL=\"mysql://root{pw}@127.0.0.1:{}/{}?serverVersion=8.0&charset=utf8mb4\"",
                    s.svc("mysql").port,
                    db_name(name)
                );
                c = re.replace(&c, line.as_str()).to_string();
            }
        }
        fs::write(&env, c)?;
        println!("[KaradelikDev] .env güncellendi");
    }
    if let Some(m) = maybe_create_db(name) {
        println!("[KaradelikDev] {m}");
    }
    println!("[KaradelikDev] {}: {}", t("msg.project_path"), dir.display());
    println!("[KaradelikDev] {}: {url}", t("msg.pretty_url"));
    log(&format!("Quick app: {name} -> {url}"));
    // Çalışan KaradelikDev örneğine vhost'ları yenilemesini söyle (single-instance ile iletilir)
    let _ = std::process::Command::new(std::env::current_exe()?).arg("reload").spawn();
    Ok(())
}

pub fn delete(name: &str) -> Result<()> {
    if !valid_name(name) {
        bail!(t("msg.invalid_project_name"));
    }
    let dir = project_dir(name);
    if !dir.starts_with(&settings().document_root) || dir == PathBuf::from(settings().document_root) {
        bail!(t("msg.invalid_project_name"));
    }
    util::remove_dir_force(&dir)?;
    if is_running(Svc::Mysql) {
        let _ = crate::mysqlops::drop_database(&db_name(name));
    }
    vhosts::sync_and_reload()?;
    Ok(())
}

pub fn clone(src: &str, dst: &str) -> Result<()> {
    let to = check_new(dst)?;
    let from = project_dir(src);
    crate::kcore::status(&tf("progress.cloning", &[src, dst]));
    util::copy_dir(&from, &to)?;
    if is_running(Svc::Mysql) {
        crate::mysqlops::clone_database(&db_name(src), &db_name(dst))?;
    }
    let env = to.join(".env");
    if let Ok(c) = fs::read_to_string(&env) {
        let c = set_env(&c, "DB_DATABASE", &db_name(dst));
        let c = set_env(&c, "APP_URL", &format!("http://{}", settings().hostname(dst)));
        fs::write(&env, c)?;
    }
    let wp = to.join("wp-config.php");
    if let Ok(c) = fs::read_to_string(&wp) {
        fs::write(&wp, c.replace(&format!("'{}'", db_name(src)), &format!("'{}'", db_name(dst))))?;
    }
    vhosts::sync_and_reload()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::set_env;

    #[test]
    fn env_edit() {
        let c = "DB_CONNECTION=sqlite\n# DB_HOST=127.0.0.1\n# DB_DATABASE=laravel\n";
        let c = set_env(c, "DB_CONNECTION", "mysql");
        let c = set_env(&c, "DB_HOST", "127.0.0.1");
        let c = set_env(&c, "DB_DATABASE", "blog");
        let c = set_env(&c, "APP_URL", "http://blog.test");
        assert!(c.contains("DB_CONNECTION=mysql"));
        assert!(c.contains("\nDB_HOST=127.0.0.1"));
        assert!(c.contains("DB_DATABASE=blog"));
        assert!(c.contains("APP_URL=http://blog.test"));
    }
}
