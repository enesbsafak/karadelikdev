//! MySQL menü işlemleri. Parola komut satırına yazılmaz; MYSQL_PWD ortam değişkeniyle geçilir.

use crate::kcore::{paths, settings, update_settings};
use crate::i18n::{t, tf};
use crate::services::{mysql, Svc};
use crate::util::{self, hidden, run_capture};
use anyhow::{anyhow, bail, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Seçili MySQL sürümünün veri dizinine ait root parolası.
pub fn root_password() -> String {
    settings().mysql_password(&mysql::key().unwrap_or_default())
}

pub fn set_root_password(pw: &str) {
    let key = mysql::key().unwrap_or_default();
    update_settings(|s| {
        s.mysql_passwords.insert(key, pw.to_string());
    });
}

fn client(name: &str) -> Result<Command> {
    client_with(name, &root_password())
}

fn client_with(name: &str, password: &str) -> Result<Command> {
    let exe = mysql::tool(name).ok_or_else(|| anyhow!(tf("msg.not_installed", &[name])))?;
    let mut c = hidden(exe);
    c.args(["-u", "root", "-h", "127.0.0.1", "-P", &settings().svc("mysql").port.to_string()]);
    if name != "mysqladmin" {
        // Konsol kod sayfası (ör. cp857) uyarısını önler
        c.arg("--default-character-set=utf8mb4");
    }
    c.env("MYSQL_PWD", password);
    Ok(c)
}

fn require_running() -> Result<()> {
    if !crate::services::is_running(Svc::Mysql) {
        bail!(tf("msg.start_first", &["MySQL"]));
    }
    Ok(())
}

pub fn valid_db_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '$')
}

/// Sorguyu çalıştırır; yalnızca stdout döner (uyarılar sonuçlara karışmasın).
pub fn sql(query: &str) -> Result<String> {
    require_running()?;
    let run = |pw: &str| -> Result<std::process::Output> {
        let mut c = client_with("mysql", pw)?;
        c.args(["--batch", "--skip-column-names", "-e", query]).stdin(Stdio::null());
        Ok(c.output()?)
    };
    let pw = root_password();
    let mut out = run(&pw)?;
    // Kayıtlı parola reddedildi ama sunucu parolasızsa (ör. yeni oluşturulmuş veri dizini) kaydı düzelt
    if !out.status.success() && !pw.is_empty() && util::decode(&out.stderr).contains("1045") {
        let retry = run("")?;
        if retry.status.success() {
            crate::kcore::log("MySQL root parolası bu veri dizininde boş; kayıt düzeltildi");
            set_root_password("");
            out = retry;
        }
    }
    if !out.status.success() {
        bail!("{}", util::decode(&out.stderr).trim());
    }
    Ok(util::decode(&out.stdout))
}

/// Sekmeyle ayrılmış satırlar.
fn rows(query: &str) -> Result<Vec<Vec<String>>> {
    Ok(sql(query)?.lines().filter(|l| !l.is_empty()).map(|l| l.split('\t').map(str::to_string).collect()).collect())
}

pub fn databases() -> Vec<String> {
    sql("SHOW DATABASES")
        .map(|o| {
            o.lines()
                .map(|l| l.trim().to_string())
                .filter(|d| !["information_schema", "performance_schema", "mysql", "sys"].contains(&d.as_str()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn create_database(name: &str) -> Result<()> {
    if !valid_db_name(name) {
        bail!(t("msg.invalid_name"));
    }
    if databases().iter().any(|d| d.eq_ignore_ascii_case(name)) {
        bail!(t("msg.db_exists"));
    }
    sql(&format!("CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"))?;
    Ok(())
}

pub fn drop_database(name: &str) -> Result<()> {
    if !valid_db_name(name) {
        bail!(t("msg.invalid_name"));
    }
    sql(&format!("DROP DATABASE IF EXISTS `{name}`"))?;
    Ok(())
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

pub fn change_root_password(new: &str) -> Result<()> {
    if new.contains('"') || new.contains('\'') {
        bail!(t("msg.password_quotes"));
    }
    let q = if mysql::is_mariadb() {
        format!("ALTER USER 'root'@'localhost' IDENTIFIED BY '{}'; FLUSH PRIVILEGES;", escape(new))
    } else {
        // Mevcut kimlik doğrulama eklentisini koru (Laragon da önce eklentiyi sorgular)
        let plugin = sql("SELECT plugin FROM mysql.user WHERE user='root' AND host='localhost'")
            .ok()
            .and_then(|o| o.lines().next().map(|l| l.trim().to_string()))
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "caching_sha2_password".into());
        format!("ALTER USER 'root'@'localhost' IDENTIFIED WITH {plugin} BY '{}'; FLUSH PRIVILEGES;", escape(new))
    };
    sql(&q)?;
    set_root_password(new);
    let _ = crate::terminal::write_env_cmd();
    Ok(())
}

/// Parolayı bilmeden sıfırlar ve kaldırır (varsayılan: parolasız root, Laragon ile aynı).
/// MySQL `--init-file` ile başlatılır (MySQL resmi yöntemi, 5.7+/8/9/MariaDB).
pub fn reset_root_password() -> Result<String> {
    let new = String::new();
    let was_running = crate::services::is_running(Svc::Mysql);
    if was_running {
        crate::kcore::status(&t("msg.stopping_mysql"));
        mysql::stop()?;
    }
    if crate::services::is_running(Svc::Mysql) {
        bail!(t("msg.mysql_must_stop"));
    }
    let file = paths().tmp().join("kd-reset-root.sql");
    std::fs::write(&file, format!("ALTER USER 'root'@'localhost' IDENTIFIED BY '{new}';\n"))?;
    let res = mysql::start_with(&[format!("--init-file={}", file.display())]);
    let _ = std::fs::remove_file(&file);
    res?;
    set_root_password(&new);
    // init-file her başlatmada çalışmasın diye normal modda yeniden başlat
    mysql::stop()?;
    std::thread::sleep(Duration::from_millis(500));
    if was_running {
        mysql::start()?;
    }
    Ok(new)
}

pub fn backup_dir() -> PathBuf {
    paths().backup().join("mysql")
}

/// Tüm veritabanlarını tek SQL dosyasına döker: backup\mysql\<anahtar>-<zaman>.sql
pub fn backup_all() -> Result<PathBuf> {
    require_running()?;
    let key = mysql::key().unwrap_or_else(|| "mysql".into());
    std::fs::create_dir_all(backup_dir())?;
    let file = backup_dir().join(format!("{key}-{}.sql", util::now_stamp()));
    let mut c = client("mysqldump")?;
    c.args(["--all-databases", "--routines", "--events", "--triggers", "--single-transaction"])
        .arg(format!("--result-file={}", file.display()));
    let (code, out) = run_capture(c)?;
    if code != 0 {
        let _ = std::fs::remove_file(&file);
        bail!("mysqldump: {}", out.trim());
    }
    Ok(file)
}

/// Son `keep` yedeği tutar.
pub fn prune_backups(keep: usize) {
    let mut files: Vec<PathBuf> = std::fs::read_dir(backup_dir())
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| x == "sql").unwrap_or(false)).collect())
        .unwrap_or_default();
    files.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    while files.len() > keep {
        let _ = std::fs::remove_file(files.remove(0));
    }
}

pub fn dump_database(name: &str, file: &Path) -> Result<()> {
    let mut c = client("mysqldump")?;
    c.args(["--routines", "--events", "--triggers", name]).arg(format!("--result-file={}", file.display()));
    let (code, out) = run_capture(c)?;
    if code != 0 {
        bail!("mysqldump: {}", out.trim());
    }
    Ok(())
}

pub fn import_file(db: &str, file: &Path) -> Result<()> {
    let mut c = client("mysql")?;
    if !db.is_empty() {
        c.arg(db);
    }
    c.stdin(Stdio::from(std::fs::File::open(file)?)).stdout(Stdio::piped()).stderr(Stdio::piped());
    let out = c.output()?;
    if !out.status.success() {
        bail!("mysql: {}", util::decode(&out.stderr).trim());
    }
    Ok(())
}

/// "Clone": veritabanını yeni adla kopyalar.
pub fn clone_database(src: &str, dst: &str) -> Result<()> {
    require_running()?;
    if !databases().iter().any(|d| d == src) {
        return Ok(());
    }
    let tmp = paths().tmp().join(format!("clone-{src}.sql"));
    dump_database(src, &tmp)?;
    create_database(dst)?;
    let r = import_file(dst, &tmp);
    let _ = std::fs::remove_file(&tmp);
    r
}

// ---------------------------------------------------------------- Veritabanı yöneticisi

#[derive(serde::Serialize)]
pub struct DbInfo {
    pub name: String,
    pub tables: u64,
    pub size: u64,
}

const SYSTEM_DBS: &[&str] = &["information_schema", "performance_schema", "mysql", "sys"];

pub fn database_infos() -> Result<Vec<DbInfo>> {
    let stats = rows("SELECT table_schema, COUNT(*), COALESCE(SUM(data_length + index_length), 0) FROM information_schema.tables GROUP BY table_schema")?;
    let mut out: Vec<DbInfo> = sql("SHOW DATABASES")?
        .lines()
        .map(str::trim)
        .filter(|d| !d.is_empty() && !SYSTEM_DBS.contains(d))
        .map(|d| {
            let st = stats.iter().find(|r| r.first().map(|x| x == d).unwrap_or(false));
            DbInfo {
                name: d.to_string(),
                tables: st.and_then(|r| r.get(1)?.parse().ok()).unwrap_or(0),
                size: st.and_then(|r| r.get(2)?.parse().ok()).unwrap_or(0),
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

#[derive(serde::Serialize)]
pub struct UserInfo {
    pub user: String,
    pub host: String,
    pub plugin: String,
    pub databases: Vec<String>,
}

pub fn users() -> Result<Vec<UserInfo>> {
    let grants = rows("SELECT User, Host, Db FROM mysql.db").unwrap_or_default();
    Ok(rows("SELECT User, Host, plugin FROM mysql.user WHERE User NOT IN ('mysql.sys','mysql.session','mysql.infoschema','') ORDER BY User, Host")?
        .into_iter()
        .map(|r| {
            let (user, host) = (r.first().cloned().unwrap_or_default(), r.get(1).cloned().unwrap_or_default());
            let databases = grants
                .iter()
                .filter(|g| g.first() == Some(&user) && g.get(1) == Some(&host))
                .filter_map(|g| g.get(2).cloned())
                .collect();
            UserInfo { plugin: r.get(2).cloned().unwrap_or_default(), user, host, databases }
        })
        .collect())
}

fn valid_user(name: &str) -> bool {
    !name.is_empty() && name.len() <= 32 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

/// Kullanıcı oluşturur; `database` verilirse yalnız o veritabanında tüm yetkiler, "*" ise her yerde.
pub fn create_user(name: &str, password: &str, database: &str) -> Result<()> {
    if !valid_user(name) || name == "root" {
        bail!(t("msg.invalid_name"));
    }
    if password.is_empty() {
        bail!(t("msg.password_empty"));
    }
    if password.contains('\'') || password.contains('"') {
        bail!(t("msg.password_quotes"));
    }
    let scope = if database == "*" || database.is_empty() {
        "*.*".to_string()
    } else {
        if !valid_db_name(database) {
            bail!(t("msg.invalid_name"));
        }
        format!("`{database}`.*")
    };
    let pw = escape(password);
    sql(&format!(
        "CREATE USER '{name}'@'localhost' IDENTIFIED BY '{pw}'; CREATE USER '{name}'@'127.0.0.1' IDENTIFIED BY '{pw}'; \
         GRANT ALL PRIVILEGES ON {scope} TO '{name}'@'localhost'; GRANT ALL PRIVILEGES ON {scope} TO '{name}'@'127.0.0.1'; FLUSH PRIVILEGES;"
    ))?;
    Ok(())
}

pub fn drop_user(name: &str, host: &str) -> Result<()> {
    if !valid_user(name) || name == "root" || !host.chars().all(|c| c.is_ascii_alphanumeric() || ".:%-_".contains(c)) {
        bail!(t("msg.invalid_name"));
    }
    sql(&format!("DROP USER IF EXISTS '{name}'@'{host}'; FLUSH PRIVILEGES;"))?;
    Ok(())
}

/// Tek veritabanını backup\mysql\db-<ad>-<zaman>.sql olarak yedekler.
pub fn backup_database(name: &str) -> Result<PathBuf> {
    require_running()?;
    if !valid_db_name(name) {
        bail!(t("msg.invalid_name"));
    }
    std::fs::create_dir_all(backup_dir())?;
    let file = backup_dir().join(format!("db-{name}-{}.sql", util::now_stamp()));
    dump_database(name, &file)?;
    Ok(file)
}

#[derive(serde::Serialize)]
pub struct BackupInfo {
    pub file: String,
    pub path: String,
    pub size: u64,
    pub modified: i64,
    /// "" = tüm sunucu dökümü, aksi halde tek veritabanının adı
    pub database: String,
}

/// "db-blog-2026-10-14_221500.sql" -> "blog"
fn backup_db_name(file: &str) -> String {
    let Some(rest) = file.strip_prefix("db-").and_then(|r| r.strip_suffix(".sql")) else { return String::new() };
    // Sondaki "-YYYY-MM-DD_HHMMSS" kısmını at
    match rest.len().checked_sub(18) {
        Some(i) if rest.as_bytes().get(i) == Some(&b'-') => rest[..i].to_string(),
        _ => String::new(),
    }
}

pub fn backups() -> Vec<BackupInfo> {
    let mut v: Vec<BackupInfo> = std::fs::read_dir(backup_dir())
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().extension().map(|x| x == "sql").unwrap_or(false))
                .filter_map(|e| {
                    let md = e.metadata().ok()?;
                    let file = e.file_name().to_string_lossy().to_string();
                    let modified = md.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
                    Some(BackupInfo { database: backup_db_name(&file), path: e.path().display().to_string(), file, size: md.len(), modified })
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| b.modified.cmp(&a.modified));
    v
}

/// .sql dosyasını geri yükler. `database` boşsa dosya tam sunucu dökümü kabul edilir.
pub fn restore(file: &Path, database: &str) -> Result<()> {
    require_running()?;
    if !database.is_empty() {
        if !valid_db_name(database) {
            bail!(t("msg.invalid_name"));
        }
        sql(&format!("CREATE DATABASE IF NOT EXISTS `{database}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"))?;
    }
    import_file(database, file)
}

#[cfg(test)]
mod tests {
    use super::backup_db_name;

    #[test]
    fn backup_names() {
        assert_eq!(backup_db_name("db-blog-2026-10-14_221500.sql"), "blog");
        assert_eq!(backup_db_name("db-my-shop-2026-10-14_221500.sql"), "my-shop");
        assert_eq!(backup_db_name("mysql-8.4-2026-10-14_221500.sql"), "");
    }
}

/// Veritabanı yöneticisinin işlemleri (Tauri `db_call` komutu ve `karadelikdev do dbcall:<işlem>` ortak).
pub fn call(op: &str, args: &[String]) -> Result<String> {
    let a = |i: usize| args.get(i).cloned().unwrap_or_default();
    match op {
        "create_db" => create_database(&a(0)).map(|_| crate::i18n::tf("msg.db_created", &[&a(0)])),
        "drop_db" => drop_database(&a(0)).map(|_| crate::i18n::tf("db.dropped", &[&a(0)])),
        "backup_db" => backup_database(&a(0)).map(|f| crate::i18n::tf("db.backup_one_done", &[&a(0), &f.display().to_string()])),
        "backup_all" => backup_all().map(|f| crate::i18n::tf("msg.backup_done", &[&f.display().to_string()])),
        "restore" => restore(std::path::Path::new(&a(0)), &a(1)).map(|_| crate::i18n::t("db.restored")),
        "create_user" => create_user(&a(0), &a(1), &a(2)).map(|_| crate::i18n::tf("db.user_created", &[&a(0)])),
        "drop_user" => drop_user(&a(0), &a(1)).map(|_| crate::i18n::tf("db.user_dropped", &[&a(0)])),
        // Boş parola = parolasız root (varsayılan)
        "change_root" => change_root_password(&a(0)).map(|_| crate::i18n::t(if a(0).is_empty() { "msg.pw_removed" } else { "msg.pw_changed" })),
        "reset_root" => reset_root_password().map(|_| crate::i18n::t("msg.pw_removed")),
        "start" => crate::services::start(Svc::Mysql).map(|_| crate::i18n::tf("msg.svc_started", &["MySQL"])),
        "delete_backup" => {
            let p = std::path::PathBuf::from(a(0));
            if p.starts_with(backup_dir()) && p.extension().map(|e| e == "sql").unwrap_or(false) {
                std::fs::remove_file(&p).map(|_| crate::i18n::t("db.backup_deleted")).map_err(anyhow::Error::from)
            } else {
                Err(anyhow::anyhow!("?"))
            }
        }
        other => Err(anyhow::anyhow!("{other}?")),
    }
}
