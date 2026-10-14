//! PHP: sürüm başına php.ini (etc\php\<sürüm>\php.ini), eklentiler, hızlı ayarlar, Apache entegrasyonu.
//! Laragon php.ini'yi bin içinde düzenler; biz bin'e dokunmadan PHPRC / -c / PHPIniDir kullanırız.

use crate::kcore::{paths, settings};
use crate::paths::fwd;
use crate::versions;
use anyhow::{anyhow, Result};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

/// Varsayılan olarak açılan eklentiler (yaygın PHP projelerinin ihtiyaç duyduğu küme).
const DEFAULT_EXTENSIONS: &[&str] = &[
    "curl", "fileinfo", "gd", "intl", "mbstring", "exif", "mysqli", "openssl", "pdo_mysql", "pdo_sqlite",
    "sodium", "sqlite3", "xsl", "zip",
];

pub fn dir() -> Option<PathBuf> {
    versions::current_dir("php")
}

pub fn version_name() -> Option<String> {
    versions::current("php")
}

pub fn ini_dir_for(ver: &str) -> PathBuf {
    paths().etc().join("php").join(ver)
}

pub fn ini_dir() -> Option<PathBuf> {
    version_name().map(|v| ini_dir_for(&v))
}

pub fn ini_path() -> Option<PathBuf> {
    ini_dir().map(|d| d.join("php.ini"))
}

pub fn php_exe() -> Option<PathBuf> {
    dir().map(|d| d.join("php.exe")).filter(|p| p.exists())
}

pub fn cgi_exe() -> Option<PathBuf> {
    dir().map(|d| d.join("php-cgi.exe")).filter(|p| p.exists())
}

/// Apache modülü: TS (thread safe) derlemelerinde php8apache2_4.dll bulunur.
pub fn apache_dll(dir: &Path) -> Option<(String, String)> {
    for (dll, module) in [
        ("php8apache2_4.dll", "php_module"),
        ("php7apache2_4.dll", "php7_module"),
        ("php5apache2_4.dll", "php5_module"),
    ] {
        if dir.join(dll).exists() {
            return Some((dll.to_string(), module.to_string()));
        }
    }
    None
}

/// php.ini'de `key = value` ayarlar; yorumlu satırı açar, yoksa sona ekler.
pub fn set_value(content: &str, key: &str, value: &str) -> String {
    let re = Regex::new(&format!(r"(?m)^[ \t]*;?[ \t]*{}[ \t]*=.*$", regex::escape(key))).unwrap();
    let line = format!("{key} = {value}");
    // Önce yorumsuz satırı ara, yoksa yorumluyu
    let active = Regex::new(&format!(r"(?m)^[ \t]*{}[ \t]*=.*$", regex::escape(key))).unwrap();
    if let Some(m) = active.find(content) {
        return format!("{}{}{}", &content[..m.start()], line, &content[m.end()..]);
    }
    if let Some(m) = re.find(content) {
        return format!("{}{}{}", &content[..m.start()], line, &content[m.end()..]);
    }
    format!("{}\n{line}\n", content.trim_end())
}

pub fn get_value(content: &str, key: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"(?m)^[ \t]*{}[ \t]*=[ \t]*"?([^"\r\n;]*)"?"#, regex::escape(key))).unwrap();
    re.captures(content).map(|c| c[1].trim().to_string())
}

/// php.ini'yi oluşturur (yoksa) ve yola bağlı ayarları her seferinde günceller.
pub fn ensure_ini() -> Result<PathBuf> {
    let pdir = dir().ok_or_else(|| anyhow!(crate::i18n::tf("msg.not_installed", &["PHP"])))?;
    let idir = ini_dir().unwrap();
    fs::create_dir_all(&idir)?;
    let ini = idir.join("php.ini");
    let p = paths();
    let mut content = if ini.exists() {
        read_ini(&ini)
    } else {
        let src = ["php.ini", "php.ini-development", "php.ini-production"]
            .iter()
            .map(|n| pdir.join(n))
            .find(|f| f.exists())
            .ok_or_else(|| anyhow!("php.ini-development bulunamadı: {}", pdir.display()))?;
        let mut c = read_ini(&src);
        for (k, v) in [
            ("memory_limit", "512M"),
            ("upload_max_filesize", "2G"),
            ("post_max_size", "2G"),
            ("max_execution_time", "36000"),
            ("max_input_vars", "10000"),
            ("display_errors", "On"),
            ("date.timezone", "Europe/Istanbul"),
            ("cgi.fix_pathinfo", "1"),
        ] {
            c = set_value(&c, k, v);
        }
        for ext in DEFAULT_EXTENSIONS {
            if pdir.join("ext").join(format!("php_{ext}.dll")).exists() {
                c = set_extension(&c, ext, true, false);
            }
        }
        c
    };
    let cacert = p.ssl().join("cacert.pem");
    let mut managed: Vec<(&str, String)> = vec![
        ("extension_dir", format!("\"{}\"", fwd(&pdir.join("ext")))),
        ("error_log", format!("\"{}\"", fwd(&p.logs().join("php_errors.log")))),
        ("session.save_path", format!("\"{}\"", fwd(&p.tmp().join("sessions")))),
        ("upload_tmp_dir", format!("\"{}\"", fwd(&p.tmp()))),
        ("sys_temp_dir", format!("\"{}\"", fwd(&p.tmp()))),
    ];
    if cacert.exists() {
        managed.push(("curl.cainfo", format!("\"{}\"", fwd(&cacert))));
        managed.push(("openssl.cafile", format!("\"{}\"", fwd(&cacert))));
    }
    if let Some(mp) = versions::exe("mailpit", "mailpit.exe") {
        let port = settings().svc("mailpit").port;
        managed.push(("sendmail_path", format!("\"{} sendmail --smtp-addr=127.0.0.1:{port}\"", fwd(&mp))));
    }
    for (k, v) in managed {
        content = set_value(&content, k, &v);
    }
    crate::util::write_if_changed(&ini, &content)?;
    Ok(ini)
}

/// php.ini'yi LF satır sonlarıyla okur (Laragon'un dosyaları CRLF; regex'ler satır sonuna bakar).
pub fn read_ini(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default().replace("
", "
")
}

fn ext_line_re(name: &str) -> Regex {
    // extension=curl | extension=php_curl.dll | zend_extension=opcache | zend_extension="C:/.../php_xdebug.dll"
    Regex::new(&format!(
        r#"(?mi)^[ \t]*(;?)[ \t]*(zend_)?extension[ \t]*=[ \t]*"?(?:[^"\r\n]*[/\\])?(?:php_)?{}(?:\.dll)?"?[ \t]*(?:;[^\r\n]*)?\r?$"#,
        regex::escape(name)
    ))
    .unwrap()
}

fn is_zend(name: &str) -> bool {
    name == "opcache" || name.starts_with("xdebug")
}

pub fn set_extension(content: &str, name: &str, on: bool, _explicit_zend: bool) -> String {
    if on && extension_enabled(content, name) {
        return content.to_string();
    }
    let re = ext_line_re(name);
    let prefix = if is_zend(name) { "zend_extension" } else { "extension" };
    let line = if on { format!("{prefix}={name}") } else { format!(";{prefix}={name}") };
    if let Some(m) = re.find(content) {
        // Etkin ve pasif kopyalardan ilkini değiştir, diğer etkinleri yorumla
        let mut out = format!("{}{}{}", &content[..m.start()], line, &content[m.end()..]);
        if !on {
            out = re
                .replace_all(&out, |c: &regex::Captures| {
                    let s = c.get(0).unwrap().as_str();
                    if c.get(1).map(|x| x.as_str().is_empty()).unwrap_or(false) { format!(";{}", s.trim_start()) } else { s.to_string() }
                })
                .to_string();
        }
        return out;
    }
    if !on {
        return content.to_string();
    }
    // [ExtensionList] yoksa dosya sonuna ekle
    format!("{}\n{line}\n", content.trim_end())
}

pub fn extension_enabled(content: &str, name: &str) -> bool {
    ext_line_re(name)
        .captures_iter(content)
        .any(|c| c.get(1).map(|m| m.as_str().is_empty()).unwrap_or(false))
}

/// ext dizinindeki eklentiler ve durumları.
pub fn extensions() -> Vec<(String, bool)> {
    let (Some(d), Some(ini)) = (dir(), ini_path()) else { return vec![] };
    let content = read_ini(&ini);
    let mut v: Vec<String> = fs::read_dir(d.join("ext"))
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let n = e.file_name().to_string_lossy().to_lowercase();
                    n.strip_prefix("php_").and_then(|x| x.strip_suffix(".dll")).map(|x| x.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v.into_iter().map(|n| { let on = extension_enabled(&content, &n); (n, on) }).collect()
}

pub fn toggle_extension(name: &str) -> Result<bool> {
    let ini = ensure_ini()?;
    let c = read_ini(&ini);
    let on = !extension_enabled(&c, name);
    let real = if name.starts_with("xdebug") { xdebug_name().unwrap_or_else(|| name.to_string()) } else { name.to_string() };
    fs::write(&ini, set_extension(&c, &real, on, false))?;
    Ok(on)
}

fn xdebug_name() -> Option<String> {
    let d = dir()?;
    fs::read_dir(d.join("ext")).ok()?.flatten().find_map(|e| {
        let n = e.file_name().to_string_lossy().to_lowercase();
        (n.starts_with("php_xdebug") && n.ends_with(".dll")).then(|| n.trim_start_matches("php_").trim_end_matches(".dll").to_string())
    })
}

/// Laragon'daki "Quick settings" menüsü: (anahtar, etiket, açık mı).
pub fn quick_settings() -> Vec<(String, String, bool)> {
    let Some(ini) = ini_path() else { return vec![] };
    let c = read_ini(&ini);
    let val = |k: &str| get_value(&c, k).unwrap_or_default();
    let mut v = vec![
        ("max_execution_time".to_string(), "max_execution_time = 0".to_string(), val("max_execution_time") == "0"),
        ("upload_max_filesize".to_string(), "upload_max_filesize = 2G".to_string(), val("upload_max_filesize").eq_ignore_ascii_case("2G")),
        ("memory_limit".to_string(), "memory_limit = 512M".to_string(), val("memory_limit").eq_ignore_ascii_case("512M")),
        ("display_errors".to_string(), "display_errors = On".to_string(), val("display_errors").eq_ignore_ascii_case("on")),
        ("opcache".to_string(), "opcache".to_string(), extension_enabled(&c, "opcache")),
    ];
    if let Some(x) = xdebug_name() {
        v.insert(0, ("xdebug".to_string(), "xdebug".to_string(), extension_enabled(&c, &x)));
    }
    v
}

pub fn toggle_quick(key: &str) -> Result<()> {
    let ini = ensure_ini()?;
    let mut c = read_ini(&ini);
    let val = |c: &str, k: &str| get_value(c, k).unwrap_or_default();
    match key {
        "max_execution_time" => {
            let v = if val(&c, key) == "0" { "30" } else { "0" };
            c = set_value(&c, key, v);
        }
        "upload_max_filesize" => {
            let big = val(&c, key).eq_ignore_ascii_case("2G");
            c = set_value(&c, key, if big { "2M" } else { "2G" });
            c = set_value(&c, "post_max_size", if big { "8M" } else { "2G" });
        }
        "memory_limit" => {
            let v = if val(&c, key).eq_ignore_ascii_case("512M") { "128M" } else { "512M" };
            c = set_value(&c, key, v);
        }
        "display_errors" => {
            let v = if val(&c, key).eq_ignore_ascii_case("on") { "Off" } else { "On" };
            c = set_value(&c, key, v);
        }
        "opcache" => {
            let on = !extension_enabled(&c, "opcache");
            c = set_extension(&c, "opcache", on, true);
        }
        "xdebug" => {
            let x = xdebug_name().ok_or_else(|| anyhow!(crate::i18n::t("msg.no_xdebug")))?;
            let on = !extension_enabled(&c, &x);
            c = set_extension(&c, &x, on, true);
            if on && !c.contains("xdebug.mode") {
                c.push_str("\n[xdebug]\nxdebug.mode = debug\nxdebug.start_with_request = trigger\nxdebug.client_port = 9003\n");
            }
        }
        _ => {}
    }
    fs::write(&ini, c)?;
    Ok(())
}

/// Apache için etc\apache2\php.conf: TS derlemede mod_php, aksi halde php-cgi havuzu.
pub fn write_apache_php_conf() -> Result<bool> {
    let p = paths();
    let conf = p.etc().join("apache2").join("php.conf");
    let Some(pdir) = dir() else {
        let _ = fs::remove_file(&conf);
        return Ok(false);
    };
    ensure_ini()?;
    let idir = ini_dir().unwrap();
    let uses_fcgi;
    let content = if let Some((dll, module)) = apache_dll(&pdir) {
        uses_fcgi = false;
        let load_files = apache_preload(&pdir);
        crate::templates::render_named(
            "apache.php.modphp.tpl",
            &[
                ("LOAD_FILES", load_files.join("\n")),
                ("PHP_MODULE", module),
                ("PHP_DIR", fwd(&pdir)),
                ("PHP_DLL", dll),
                ("PHP_INI_DIR", fwd(&idir)),
            ],
        )
    } else {
        uses_fcgi = true;
        let n = settings().php_upstream.max(1);
        let members: Vec<String> =
            (0..n).map(|i| format!("    BalancerMember \"fcgi://127.0.0.1:{}\"", 9001 + i)).collect();
        crate::templates::render_named("apache.php.fcgi.tpl", &[("BALANCER_MEMBERS", members.join("\n"))])
    };
    crate::util::write_if_changed(&conf, &content)?;
    Ok(uses_fcgi)
}

/// mod_php için önceden yüklenecek DLL'ler. Windows bağımlılıkları önce httpd.exe'nin klasöründe arar;
/// orada başka sürümün php8ts.dll'i varsa (Laragon onu oraya kopyalar) eklentiler "yordam bulunamadı"
/// hatasıyla yüklenemez. Aynı adlı DLL bellekte varsa Windows onu kullandığı için seçili sürümünkiler
/// bağımlılık sırasıyla önce yüklenir. OpenSSL yalnızca Apache'de yoksa yüklenir; aksi halde mod_ssl
/// kendi (daha yeni) OpenSSL'i yerine PHP'ninkini kullanmak zorunda kalır.
fn apache_preload(pdir: &Path) -> Vec<String> {
    // Bağımlılıkları önce gelenler; listede olmayan diğer DLL'ler arada, çekirdek (php8ts) en sonda
    const FIRST: &[&str] = &["icudt", "icuuc", "icuin", "icuio", "libcrypto", "libssl"];
    const SKIP: &[&str] = &["php8apache", "php7apache", "php5apache", "php8phpdbg", "php7phpdbg", "php8embed", "php7embed", "deplister"];
    let apache_bin = crate::services::apache::exe().and_then(|e| e.parent().map(Path::to_path_buf));
    let mut names: Vec<String> = fs::read_dir(pdir)
        .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.to_lowercase().ends_with(".dll")).collect())
        .unwrap_or_default();
    names.retain(|n| !SKIP.iter().any(|s| n.to_lowercase().starts_with(s)));
    let rank = |n: &str| {
        let l = n.to_lowercase();
        if l.starts_with("php") && l.contains("ts") {
            return FIRST.len() + 1;
        }
        FIRST.iter().position(|p| l.starts_with(p)).unwrap_or(FIRST.len())
    };
    names.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a.to_lowercase().cmp(&b.to_lowercase())));
    names
        .into_iter()
        .filter(|n| {
            let l = n.to_lowercase();
            let openssl = l.starts_with("libcrypto") || l.starts_with("libssl");
            !(openssl && apache_bin.as_ref().map(|b| b.join(n).exists()).unwrap_or(false))
        })
        .map(|n| format!("LoadFile \"{}/{}\"", fwd(pdir), n))
        .collect()
}

/// PHP ve eklentilerinin DLL bağımlılıkları için PATH (Apache/php-cgi ortamına eklenir).
pub fn env_path() -> Option<String> {
    let d = dir()?;
    Some(format!("{};{}", d.display(), d.join("ext").display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_values() {
        let c = "memory_limit = 128M\n;date.timezone =\n";
        let c = set_value(c, "memory_limit", "512M");
        let c = set_value(&c, "date.timezone", "Europe/Istanbul");
        let c = set_value(&c, "new_key", "1");
        assert_eq!(get_value(&c, "memory_limit").as_deref(), Some("512M"));
        assert_eq!(get_value(&c, "date.timezone").as_deref(), Some("Europe/Istanbul"));
        assert!(c.contains("new_key = 1"));
    }

    #[test]
    fn extensions_toggle() {
        let c = ";extension=curl\nextension=php_gd2.dll\n;zend_extension=opcache\n";
        assert!(!extension_enabled(c, "curl"));
        let c2 = set_extension(c, "curl", true, false);
        assert!(extension_enabled(&c2, "curl"));
        assert!(extension_enabled(&c2, "gd2"));
        let c3 = set_extension(&c2, "gd2", false, false);
        assert!(!extension_enabled(&c3, "gd2"));
        let c4 = set_extension(&c3, "opcache", true, true);
        assert!(c4.contains("zend_extension=opcache"));
        // "pdo_mysql" etkinleşince "mysqli" etkilenmemeli
        let c5 = set_extension(";extension=mysqli\n;extension=pdo_mysql\n", "pdo_mysql", true, false);
        assert!(!extension_enabled(&c5, "mysqli"));
        assert!(extension_enabled(&c5, "pdo_mysql"));
    }

    #[test]
    fn crlf_ini_does_not_duplicate() {
        let c = "extension=curl\r\nextension=exif      ; yorum\r\n;extension=zip\r\n";
        assert!(extension_enabled(c, "curl"));
        assert!(extension_enabled(c, "exif"));
        assert_eq!(set_extension(c, "curl", true, false), c);
        assert_eq!(set_extension(c, "zip", true, false).matches("extension=zip").count(), 1);
    }
}
