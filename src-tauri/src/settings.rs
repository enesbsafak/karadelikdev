//! usr\karadelik.ini - Laragon'un laragon.ini dosyasının karşılığı.

use crate::paths::Paths;
use ini::Ini;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SvcCfg {
    pub enabled: bool,
    pub port: u16,
    #[serde(default)]
    pub ssl: bool,
    #[serde(default)]
    pub ssl_port: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub language: String,
    pub run_at_startup: bool,
    pub run_minimized: bool,
    pub auto_start: bool,
    pub document_root: String,
    pub data_dir: String,
    pub auto_vhosts: bool,
    #[serde(default = "yes")]
    pub manage_hosts: bool,
    pub hostname_format: String,
    pub vhost_ignore: String,
    pub auto_backup: bool,
    pub backup_interval: u32,
    pub backup_keep: u32,
    pub auto_update: bool,
    pub profile: String,
    pub terminal: String,
    pub editor: String,
    pub php_upstream: u32,
    pub mailpit_ui_port: u16,
    /// Eski (sürümden bağımsız) parola; veri dizinine özel kayıt yoksa yedek olarak kullanılır.
    pub mysql_root_password: String,
    /// Veri dizini anahtarı (ör. "mysql-8.4") -> root parolası. Her sürümün veri dizini ayrı olduğu için
    /// parolası da ayrıdır; Laragon tek parola tuttuğu için sürüm değişince "Access denied" verir.
    #[serde(default)]
    pub mysql_passwords: BTreeMap<String, String>,
    /// Veri dizini anahtarı (ör. "postgresql-18.2") -> `postgres` kullanıcısının parolası.
    #[serde(default)]
    pub pg_passwords: BTreeMap<String, String>,
    /// Apache/Nginx yerel ağdan erişilebilsin mi (varsayılan: yalnız bu bilgisayar).
    #[serde(default)]
    pub lan_access: bool,
    pub tunnel: String,
    pub ngrok_token: String,
    pub services: BTreeMap<String, SvcCfg>,
    #[serde(default)]
    pub last_backup: i64,
    #[serde(default)]
    pub last_update_check: i64,
    #[serde(default)]
    pub in_path: bool,
    #[serde(default)]
    pub first_run: bool,
}

fn yes() -> bool {
    true
}

fn default_services() -> BTreeMap<String, SvcCfg> {
    let mut m = BTreeMap::new();
    let mut add = |id: &str, enabled: bool, port: u16, ssl_port: u16| {
        m.insert(id.to_string(), SvcCfg { enabled, port, ssl: false, ssl_port });
    };
    add("apache", true, 80, 443);
    add("nginx", false, 8080, 8443);
    add("mysql", true, 3306, 0);
    add("postgresql", false, 5432, 0);
    add("mongodb", false, 27017, 0);
    add("redis", false, 6379, 0);
    add("memcached", false, 11211, 0);
    add("mailpit", true, 1025, 0);
    add("phpserver", false, 8000, 0);
    m
}

impl Settings {
    pub fn defaults(paths: &Paths) -> Settings {
        Settings {
            language: "tr".into(),
            run_at_startup: false,
            run_minimized: false,
            auto_start: false,
            document_root: paths.default_www().to_string_lossy().into(),
            data_dir: paths.default_data().to_string_lossy().into(),
            auto_vhosts: true,
            manage_hosts: true,
            hostname_format: "{name}.test".into(),
            vhost_ignore: String::new(),
            auto_backup: true,
            backup_interval: 8,
            backup_keep: 5,
            auto_update: false,
            profile: "default".into(),
            terminal: "auto".into(),
            editor: String::new(),
            php_upstream: 4,
            mailpit_ui_port: 8025,
            mysql_root_password: String::new(),
            mysql_passwords: BTreeMap::new(),
            pg_passwords: BTreeMap::new(),
            lan_access: false,
            tunnel: "cloudflared".into(),
            ngrok_token: String::new(),
            services: default_services(),
            last_backup: 0,
            last_update_check: 0,
            in_path: false,
            first_run: true,
        }
    }

    pub fn load(paths: &Paths) -> Settings {
        let mut s = Settings::defaults(paths);
        let Ok(ini) = Ini::load_from_file(paths.ini()) else {
            return s;
        };
        let get = |sec: &str, key: &str| ini.get_from(Some(sec), key).map(|v| v.trim().to_string());
        let b = |v: Option<String>, d: bool| v.map(|x| matches!(x.as_str(), "1" | "-1" | "true" | "True")).unwrap_or(d);
        let p = "preferences";
        if let Some(v) = get(p, "Language") { s.language = v; }
        s.run_at_startup = b(get(p, "RunAtStartup"), s.run_at_startup);
        s.run_minimized = b(get(p, "RunMinimized"), s.run_minimized);
        s.auto_start = b(get(p, "AutoStart"), s.auto_start);
        if let Some(v) = get(p, "DocumentRoot").filter(|v| !v.is_empty()) { s.document_root = v; }
        if let Some(v) = get(p, "DataDir").filter(|v| !v.is_empty()) { s.data_dir = v; }
        s.auto_vhosts = b(get(p, "AutoVirtualHosts"), s.auto_vhosts);
        s.manage_hosts = b(get(p, "ManageHostsFile"), s.manage_hosts);
        if let Some(v) = get(p, "HostnameFormat").filter(|v| v.contains("{name}")) { s.hostname_format = v; }
        if let Some(v) = get(p, "IgnoreList") { s.vhost_ignore = v; }
        s.auto_backup = b(get(p, "AutoBackup"), s.auto_backup);
        if let Some(v) = get(p, "BackupInterval").and_then(|v| v.parse().ok()) { s.backup_interval = v; }
        if let Some(v) = get(p, "BackupKeep").and_then(|v| v.parse().ok()) { s.backup_keep = v; }
        s.auto_update = b(get(p, "AutoUpdate"), s.auto_update);
        if let Some(v) = get(p, "Profile").filter(|v| !v.is_empty()) { s.profile = v; }
        if let Some(v) = get(p, "Terminal") { s.terminal = v; }
        if let Some(v) = get(p, "Editor") { s.editor = v; }
        s.in_path = b(get(p, "InPath"), s.in_path);
        s.first_run = b(get(p, "FirstRun"), s.first_run);
        if let Some(v) = get("nginx", "Upstream").and_then(|v| v.parse().ok()) { s.php_upstream = v; }
        if let Some(v) = get("mailpit", "UIPort").and_then(|v| v.parse().ok()) { s.mailpit_ui_port = v; }
        if let Some(v) = get("mysql", "RootPassword") { s.mysql_root_password = v; }
        s.lan_access = b(get(p, "LanAccess"), s.lan_access);
        if let Some(sec) = ini.section(Some("postgresql")) {
            for (k, v) in sec.iter() {
                if let Some(key) = k.strip_prefix("Password.") {
                    s.pg_passwords.insert(key.to_string(), v.to_string());
                }
            }
        }
        if let Some(sec) = ini.section(Some("mysql")) {
            for (k, v) in sec.iter() {
                if let Some(key) = k.strip_prefix("RootPassword.") {
                    s.mysql_passwords.insert(key.to_string(), v.to_string());
                }
            }
        }
        if let Some(v) = get("share", "Tunnel") { s.tunnel = v; }
        if let Some(v) = get("share", "NgrokAuthToken") { s.ngrok_token = v; }
        if let Some(v) = get("state", "LastBackup").and_then(|v| v.parse().ok()) { s.last_backup = v; }
        if let Some(v) = get("state", "LastUpdateCheck").and_then(|v| v.parse().ok()) { s.last_update_check = v; }
        for (id, cfg) in s.services.iter_mut() {
            let sec = format!("service.{id}");
            cfg.enabled = b(get(&sec, "Enabled"), cfg.enabled);
            if let Some(v) = get(&sec, "Port").and_then(|v| v.parse().ok()) { cfg.port = v; }
            cfg.ssl = b(get(&sec, "SSL"), cfg.ssl);
            if let Some(v) = get(&sec, "SSLPort").and_then(|v| v.parse().ok()) { cfg.ssl_port = v; }
        }
        s
    }

    pub fn save(&self, paths: &Paths) -> anyhow::Result<()> {
        let bs = |b: bool| if b { "1" } else { "0" }.to_string();
        let mut ini = Ini::new();
        ini.with_section(Some("preferences"))
            .set("Language", &self.language)
            .set("RunAtStartup", bs(self.run_at_startup))
            .set("RunMinimized", bs(self.run_minimized))
            .set("AutoStart", bs(self.auto_start))
            .set("DocumentRoot", &self.document_root)
            .set("DataDir", &self.data_dir)
            .set("AutoVirtualHosts", bs(self.auto_vhosts))
            .set("ManageHostsFile", bs(self.manage_hosts))
            .set("HostnameFormat", &self.hostname_format)
            .set("IgnoreList", &self.vhost_ignore)
            .set("AutoBackup", bs(self.auto_backup))
            .set("BackupInterval", self.backup_interval.to_string())
            .set("BackupKeep", self.backup_keep.to_string())
            .set("AutoUpdate", bs(self.auto_update))
            .set("Profile", &self.profile)
            .set("Terminal", &self.terminal)
            .set("Editor", &self.editor)
            .set("InPath", bs(self.in_path))
            .set("FirstRun", bs(self.first_run))
            .set("LanAccess", bs(self.lan_access));
        ini.with_section(Some("nginx")).set("Upstream", self.php_upstream.to_string());
        ini.with_section(Some("mailpit")).set("UIPort", self.mailpit_ui_port.to_string());
        ini.with_section(Some("mysql")).set("RootPassword", &self.mysql_root_password);
        for (k, v) in &self.pg_passwords {
            ini.with_section(Some("postgresql")).set(format!("Password.{k}"), v);
        }
        for (k, v) in &self.mysql_passwords {
            ini.with_section(Some("mysql")).set(format!("RootPassword.{k}"), v);
        }
        ini.with_section(Some("share"))
            .set("Tunnel", &self.tunnel)
            .set("NgrokAuthToken", &self.ngrok_token);
        for (id, c) in &self.services {
            ini.with_section(Some(format!("service.{id}")))
                .set("Enabled", bs(c.enabled))
                .set("Port", c.port.to_string())
                .set("SSL", bs(c.ssl))
                .set("SSLPort", c.ssl_port.to_string());
        }
        ini.with_section(Some("state"))
            .set("LastBackup", self.last_backup.to_string())
            .set("LastUpdateCheck", self.last_update_check.to_string());
        std::fs::create_dir_all(paths.usr())?;
        ini.write_to_file(paths.ini())?;
        Ok(())
    }

    pub fn mysql_password(&self, key: &str) -> String {
        self.mysql_passwords.get(key).cloned().unwrap_or_else(|| self.mysql_root_password.clone())
    }

    pub fn svc(&self, id: &str) -> SvcCfg {
        self.services.get(id).cloned().unwrap_or(SvcCfg { enabled: false, port: 0, ssl: false, ssl_port: 0 })
    }

    pub fn hostname(&self, name: &str) -> String {
        self.hostname_format.replace("{name}", &name.to_lowercase())
    }

    pub fn ignore_list(&self) -> Vec<String> {
        self.vhost_ignore
            .split([',', ';'])
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    }
}
