//! `bin\<kategori>\<sürüm>` tarama ve profil (usr\profile\<ad>.ini) yönetimi.

use crate::kcore::{paths, settings};
use crate::util::{find_file, subdirs};
use ini::Ini;
use std::path::PathBuf;

/// Sürüm seçilebilen kategoriler (menüde "Sürüm" alt menüsü olanlar).
pub const VERSIONED: &[(&str, &str)] = &[
    ("apache", "Apache"),
    ("nginx", "Nginx"),
    ("php", "PHP"),
    ("mysql", "MySQL"),
    ("postgresql", "PostgreSQL"),
    ("mongodb", "MongoDB"),
    ("redis", "Redis"),
    ("memcached", "Memcached"),
    ("mailpit", "Mailpit"),
    ("nodejs", "Node.js"),
    ("python", "Python"),
    ("go", "Go"),
    ("ruby", "Ruby"),
    ("java", "Java"),
];

pub fn is_versioned(cat: &str) -> bool {
    VERSIONED.iter().any(|(c, _)| *c == cat)
}

pub fn title(cat: &str) -> String {
    VERSIONED.iter().find(|(c, _)| *c == cat).map(|(_, t)| t.to_string()).unwrap_or_else(|| cat.to_string())
}

pub fn installed(cat: &str) -> Vec<String> {
    subdirs(&paths().bin().join(cat))
}

fn profile_file(name: &str) -> PathBuf {
    paths().profiles().join(format!("{name}.ini"))
}

pub fn profiles() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(paths().profiles())
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let p = e.path();
                    (p.extension().map(|x| x == "ini").unwrap_or(false))
                        .then(|| p.file_stem().unwrap().to_string_lossy().to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    if !v.iter().any(|p| p == "default") {
        v.insert(0, "default".into());
    }
    v.sort_by_key(|p| (p != "default", p.to_lowercase()));
    v
}

/// Seçili sürüm: profilde yazılıysa ve kuruluysa o, değilse en yeni kurulu sürüm.
pub fn current(cat: &str) -> Option<String> {
    let list = installed(cat);
    let prof = settings().profile;
    if let Ok(ini) = Ini::load_from_file(profile_file(&prof)) {
        if let Some(v) = ini.get_from(Some(cat), "Version") {
            if list.iter().any(|x| x == v) {
                return Some(v.to_string());
            }
        }
    }
    // Profil bu kategoriyi sabitlemiyorsa en yenisini seç ve profile yaz; böylece sonradan
    // indirilen bir sürüm aktif sürümü sessizce değiştirmez (Laragon davranışı).
    let first = list.into_iter().next()?;
    let _ = set_current(cat, &first);
    Some(first)
}

pub fn current_dir(cat: &str) -> Option<PathBuf> {
    current(cat).map(|v| paths().bin().join(cat).join(v))
}

pub fn set_current(cat: &str, ver: &str) -> anyhow::Result<()> {
    let prof = settings().profile;
    let file = profile_file(&prof);
    let mut ini = Ini::load_from_file(&file).unwrap_or_default();
    ini.with_section(Some(cat)).set("Version", ver);
    std::fs::create_dir_all(paths().profiles())?;
    ini.write_to_file(file)?;
    Ok(())
}

/// Mevcut profilin tüm seçimlerini yeni bir profile kopyalar.
pub fn create_profile(name: &str) -> anyhow::Result<()> {
    let file = profile_file(name);
    if file.exists() {
        anyhow::bail!(crate::i18n::tf("msg.profile_exists", &[name]));
    }
    let mut ini = Ini::new();
    for (cat, _) in VERSIONED {
        if let Some(v) = current(cat) {
            ini.with_section(Some(*cat)).set("Version", v);
        }
    }
    std::fs::create_dir_all(paths().profiles())?;
    ini.write_to_file(file)?;
    Ok(())
}

/// Kategori içinde bir exe'yi bulur (ör. `exe("apache", "httpd.exe")`).
pub fn exe(cat: &str, name: &str) -> Option<PathBuf> {
    let dir = current_dir(cat)?;
    find_file(&dir, name, 3)
}

/// Sürümsüz araçlar (`bin\git`, `bin\composer`...) için exe araması.
pub fn tool(cat: &str, name: &str) -> Option<PathBuf> {
    let dir = paths().bin().join(cat);
    if !dir.exists() {
        return None;
    }
    find_file(&dir, name, 3)
}

/// "mysql-8.4.3-winx64" -> "mysql-8.4", "mariadb-11.4.8-winx64" -> "mariadb-11.4".
/// Veri dizini adı olarak kullanılır (Laragon ile aynı kural, ör. data\mysql-8.4).
pub fn major_minor_key(folder: &str, fallback_prefix: &str) -> String {
    let re = regex::Regex::new(r"(?i)^([a-z]+)?[-_]?(\d+)\.(\d+)").unwrap();
    let lower = folder.to_lowercase();
    if let Some(c) = re.captures(&lower) {
        let prefix = c.get(1).map(|m| m.as_str()).unwrap_or(fallback_prefix);
        let prefix = if prefix.is_empty() { fallback_prefix } else { prefix };
        return format!("{prefix}-{}.{}", &c[2], &c[3]);
    }
    if let Some(c) = regex::Regex::new(r"(\d+)\.(\d+)").unwrap().captures(&lower) {
        return format!("{fallback_prefix}-{}.{}", &c[1], &c[2]);
    }
    lower
}

#[cfg(test)]
mod tests {
    use super::major_minor_key;

    #[test]
    fn keys() {
        assert_eq!(major_minor_key("mysql-8.4.3-winx64", "mysql"), "mysql-8.4");
        assert_eq!(major_minor_key("mariadb-11.4.8-winx64", "mysql"), "mariadb-11.4");
        assert_eq!(major_minor_key("postgresql-18.2", "postgresql"), "postgresql-18.2");
        assert_eq!(major_minor_key("redis-x64-5.0.14.1", "redis"), "redis-5.0");
    }
}
