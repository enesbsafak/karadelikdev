//! Proje başına ayarlar (usr\projects.ini): PHP sürümü ve Node/Vite gibi bir geliştirme sunucusuna ters vekil (proxy).
//!
//! ```ini
//! [blog]
//! Php=php-8.3.30-Win32-vs16-x64   ; boş: profildeki sürüm
//! [shop-ui]
//! Proxy=5173                      ; http://shop-ui.test -> http://127.0.0.1:5173 (WebSocket/HMR dahil)
//! ```

use crate::kcore::paths;
use anyhow::{bail, Result};
use ini::Ini;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Profildeki sürümden farklı PHP sürümleri bu porttan itibaren birer php-cgi ile çalışır.
pub const PHP_POOL_BASE: u16 = 9101;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ProjectCfg {
    /// bin\php altındaki sürüm klasörü; boş = profildeki sürüm.
    #[serde(default)]
    pub php: String,
    /// 0 = yok; aksi halde proje bu porta yönlendirilir.
    #[serde(default)]
    pub proxy: u16,
}

fn file() -> PathBuf {
    paths().usr().join("projects.ini")
}

pub fn all() -> BTreeMap<String, ProjectCfg> {
    let mut out = BTreeMap::new();
    let Ok(ini) = Ini::load_from_file(file()) else { return out };
    for (sec, props) in ini.iter() {
        let Some(name) = sec else { continue };
        let cfg = ProjectCfg {
            php: props.get("Php").unwrap_or("").trim().to_string(),
            proxy: props.get("Proxy").and_then(|v| v.trim().parse().ok()).unwrap_or(0),
        };
        out.insert(name.to_lowercase(), cfg);
    }
    out
}

pub fn set(name: &str, cfg: &ProjectCfg) -> Result<()> {
    if !cfg.php.is_empty() && !crate::versions::installed("php").contains(&cfg.php) {
        bail!(crate::i18n::tf("msg.not_installed", &[&cfg.php]));
    }
    if !cfg.php.is_empty() && !has_cgi(&cfg.php) {
        bail!(crate::i18n::tf("msg.not_installed", &[&format!("{} php-cgi", cfg.php)]));
    }
    let mut ini = Ini::load_from_file(file()).unwrap_or_default();
    let key = name.to_lowercase();
    ini.delete(Some(key.as_str()));
    if *cfg != ProjectCfg::default() {
        let mut s = ini.with_section(Some(key.as_str()));
        if !cfg.php.is_empty() {
            s.set("Php", cfg.php.as_str());
        }
        if cfg.proxy > 0 {
            s.set("Proxy", cfg.proxy.to_string());
        }
    }
    ini.write_to_file(file())?;
    // Terminaldeki `php` da proje sürümünü kullansın (usr/bin/php)
    let _ = crate::terminal::write_env_cmd();
    Ok(())
}

pub fn has_cgi(ver: &str) -> bool {
    crate::php::cgi_exe_for(&crate::php::dir_for(ver)).is_some()
}

/// Profildekinden farklı PHP isteyen (ve proxy kullanmayan) mevcut projelerin sürümleri -> php-cgi portu.
pub fn php_pools() -> Vec<(String, u16)> {
    let current = crate::php::version_name().unwrap_or_default();
    let existing: Vec<String> = crate::vhosts::projects().into_iter().map(|p| p.name.to_lowercase()).collect();
    let mut vers: Vec<String> = all()
        .into_iter()
        .filter(|(name, c)| existing.contains(name) && c.proxy == 0 && !c.php.is_empty() && c.php != current && has_cgi(&c.php))
        .map(|(_, c)| c.php)
        .collect();
    vers.sort();
    vers.dedup();
    vers.into_iter().enumerate().map(|(i, v)| (v, PHP_POOL_BASE + i as u16)).collect()
}

/// Arayüz için kaba proje türü.
pub fn kind(dir: &Path) -> &'static str {
    if dir.join("artisan").exists() {
        "Laravel"
    } else if dir.join("wp-config.php").exists() || dir.join("wp-login.php").exists() {
        "WordPress"
    } else if dir.join("bin").join("console").exists() && dir.join("composer.json").exists() {
        "Symfony"
    } else if dir.join("package.json").exists() && !dir.join("composer.json").exists() && !dir.join("index.php").exists() {
        "Node"
    } else if dir.join("composer.json").exists() || dir.join("index.php").exists() || dir.join("public").join("index.php").exists() {
        "PHP"
    } else {
        "HTML"
    }
}
