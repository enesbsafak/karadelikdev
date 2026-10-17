//! PHP eklentisi kurulumu (Menü → PHP → Eklentiler → Eklenti kur; `karadelikdev do php:extinstall:<ad>`).
//! Windows: PECL'in resmi Windows derlemeleri, `downloads.php.net/~windows/pecl/releases/<ad>/<sürüm>/
//! php_<ad>-<sürüm>-<x.y>-<ts|nts>-<vs16|vs17>-x64.zip`. Eklenti DLL'i `ext\`'e, yanındaki bağımlılık DLL'leri
//! (ör. Imagick'in CORE_RL_*.dll'leri) PHP klasörüne kopyalanır; PHP'nin kendi DLL'lerinin üzerine yazılmaz.
//! macOS: Homebrew PHP'nin `pecl install` komutu (kaynaktan derler; autoconf, pkgconf ve eklentinin kütüphaneleri
//! önce Homebrew'dan kurulur). pecl Homebrew'un php.ini'sine dokunmaz (`-d php_ini=`); ayar bizim php.ini'mizdedir.
//! Kurulan eklenti php.ini'de açılır ve PHP'nin onu yükleyebildiği denetlenir; yüklenemezse ayar geri alınır.

use crate::i18n::tf;
use crate::kcore::{log, status};
use anyhow::{anyhow, bail, Result};
use std::path::Path;

pub struct Ext {
    pub name: &'static str,
    pub title: &'static str,
    /// macOS: derleme için gereken Homebrew paketleri.
    #[allow(dead_code)]
    brew: &'static [&'static str],
}

pub const CATALOG: &[Ext] = &[
    Ext { name: "xdebug", title: "Xdebug", brew: &[] },
    Ext { name: "redis", title: "Redis (phpredis)", brew: &[] },
    Ext { name: "imagick", title: "Imagick", brew: &["imagemagick"] },
    Ext { name: "mongodb", title: "MongoDB", brew: &["openssl@3"] },
    Ext { name: "pcov", title: "PCOV", brew: &[] },
    Ext { name: "apcu", title: "APCu", brew: &[] },
    Ext { name: "igbinary", title: "igbinary", brew: &[] },
];

/// Seçili PHP sürümünde henüz bulunmayan, kurulabilir eklentiler.
pub fn installable() -> Vec<&'static Ext> {
    let present: Vec<String> = crate::php::extensions().into_iter().map(|(n, _)| n).collect();
    CATALOG.iter().filter(|e| !present.iter().any(|p| p == e.name || p.starts_with(&format!("{}-", e.name)))).collect()
}

/// Eklentiyi seçili PHP sürümüne kurar, php.ini'de açar ve yüklendiğini doğrular. Kurulan sürümü döndürür.
pub fn install(name: &str) -> Result<String> {
    let ext = CATALOG.iter().find(|e| e.name == name).ok_or_else(|| anyhow!("{name}?"))?;
    let ver = crate::php::version_name().ok_or_else(|| anyhow!(tf("msg.not_installed", &["PHP"])))?;
    let pdir = crate::php::dir_for(&ver);
    status(&tf("msg.ext_installing", &[ext.title]));
    let installed = fetch(ext, &pdir)?;
    enable(name, &ver)?;
    log(&format!("Eklenti kuruldu: {name} {installed} ({ver})"));
    Ok(installed)
}

/// php.ini'de açar; PHP eklentiyi yükleyemezse eski php.ini'yi geri yazar.
fn enable(name: &str, ver: &str) -> Result<()> {
    let ini = crate::php::ensure_ini_for(ver)?;
    let before = crate::php::read_ini(&ini);
    let mut c = crate::php::set_extension(&before, name, true, false);
    if name == "xdebug" && !c.contains("xdebug.mode") {
        c.push_str(crate::php::XDEBUG_DEFAULTS);
    }
    std::fs::write(&ini, &c)?;
    let php = crate::php::php_exe_for(ver).ok_or_else(|| anyhow!(tf("msg.not_installed", &[ver])))?;
    let mut cmd = crate::util::hidden(php);
    cmd.env("PHPRC", crate::php::ini_dir_for(ver));
    cmd.args(["-r", &format!("exit(extension_loaded('{name}') ? 0 : 3);")]);
    let (code, out) = crate::util::run_capture(cmd)?;
    if code != 0 {
        std::fs::write(&ini, before)?;
        bail!("{}\n{}", tf("msg.ext_load_failed", &[name]), out.trim());
    }
    Ok(())
}

#[cfg(windows)]
const PECL_WIN: &str = "https://downloads.php.net/~windows/pecl/releases";

/// Derleme etiketi, ör. `8.4-ts-vs17-x64`: PECL zip adlarındaki PHP sürümü, iş parçacığı güvenliği ve derleyici.
#[cfg(windows)]
pub fn build_tag(folder: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?i)^php-(\d+\.\d+)\.\d+(-nts)?-Win32-(v[cs]\d+)-(x64|x86)$").unwrap();
    let c = re.captures(folder)?;
    let ts = if c.get(2).is_some() { "nts" } else { "ts" };
    Some(format!("{}-{ts}-{}-{}", &c[1], c[3].to_lowercase(), c[4].to_lowercase()))
}

/// Dizin listesindeki kararlı sürümler (RC/beta hariç), yeniden eskiye.
#[cfg(windows)]
pub fn stable_versions(index_html: &str) -> Vec<String> {
    let re = regex::Regex::new(r#"href="(\d+)\.(\d+)\.(\d+)/""#).unwrap();
    let mut v: Vec<(u32, u32, u32)> = re
        .captures_iter(index_html)
        .filter_map(|c| Some((c[1].parse().ok()?, c[2].parse().ok()?, c[3].parse().ok()?)))
        .collect();
    v.sort_unstable_by(|a, b| b.cmp(a));
    v.dedup();
    v.into_iter().map(|(a, b, c)| format!("{a}.{b}.{c}")).collect()
}

#[cfg(windows)]
fn fetch(ext: &Ext, pdir: &Path) -> Result<String> {
    let folder = pdir.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let tag = build_tag(&folder).ok_or_else(|| anyhow!(tf("msg.ext_unknown_build", &[&folder])))?;
    let index = crate::packages::fetch_text(&format!("{PECL_WIN}/{}/", ext.name))?;
    // En yeni sürümün bu PHP için derlemesi henüz olmayabilir; birkaç önceki sürüme bakılır
    let found = stable_versions(&index).into_iter().take(8).find_map(|v| {
        let url = format!("{PECL_WIN}/{0}/{v}/php_{0}-{v}-{tag}.zip", ext.name);
        ureq::head(&url).header("User-Agent", "KaradelikDev").call().is_ok().then_some((v, url))
    });
    let Some((v, url)) = found else { bail!(tf("msg.ext_no_build", &[ext.title, &tag])) };
    let p = crate::kcore::paths();
    let stamp = crate::util::now_stamp();
    let zip = p.tmp().join("downloads").join(format!("php_{}-{v}-{tag}.zip", ext.name));
    let staging = p.tmp().join("extract").join(format!("pecl-{}-{stamp}", ext.name));
    // fetch_into kullanılmaz: tek alt klasörlü zip'lerde (xdebug: contrib/) klasörü yukarı taşıyıp kökteki DLL'i atar
    crate::packages::download(&url, &zip, ext.title)?;
    let result = (|| -> Result<()> {
        crate::packages::extract(&zip, &staging, ext.title)?;
        let dll = format!("php_{}.dll", ext.name);
        let src = crate::util::find_file(&staging, &dll, 2).ok_or_else(|| anyhow!("{dll}: {url}"))?;
        std::fs::copy(&src, pdir.join("ext").join(&dll))?;
        for e in std::fs::read_dir(src.parent().unwrap())?.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            let l = n.to_lowercase();
            if l.ends_with(".dll") && !l.starts_with("php_") && !pdir.join(&n).exists() {
                std::fs::copy(e.path(), pdir.join(&n))?;
            }
        }
        Ok(())
    })();
    let _ = crate::util::remove_dir_force(&staging);
    let _ = std::fs::remove_file(&zip);
    crate::packages::progress_done();
    result?;
    Ok(v)
}

#[cfg(not(windows))]
fn fetch(ext: &Ext, pdir: &Path) -> Result<String> {
    use crate::util::run_capture;
    let brew = crate::platform::brew().ok_or_else(|| anyhow!(tf("msg.not_installed", &["Homebrew"])))?;
    let pecl = [pdir.join("bin").join("pecl"), pdir.join("pecl")]
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| anyhow!(tf("msg.not_installed", &["pecl"])))?;
    let mut deps: Vec<&str> = vec!["autoconf", "pkgconf"];
    deps.extend(ext.brew);
    let missing: Vec<&str> = deps
        .into_iter()
        .filter(|f| {
            let mut c = crate::util::hidden(&brew);
            c.args(["list", "--versions", f]);
            !run_capture(c).map(|(code, _)| code == 0).unwrap_or(false)
        })
        .collect();
    if !missing.is_empty() {
        status(&format!("brew install {}", missing.join(" ")));
        let mut c = crate::util::hidden(&brew);
        c.arg("install").args(&missing);
        let (code, out) = run_capture(c)?;
        log(&format!("brew install {}:\n{}", missing.join(" "), tail(&out)));
        if code != 0 {
            bail!("brew install {}: {}", missing.join(" "), tail(&out));
        }
    }
    let prefix = crate::platform::brew_prefix().unwrap_or_default();
    // Homebrew'un anahtarsız (keg-only) openssl@3'ü pkg-config ile bulunsun (MongoDB)
    let pkgconfig = prefix.join("opt").join("openssl@3").join("lib").join("pkgconfig");
    let q = |p: &Path| crate::platform::sh_quote(&p.display().to_string());
    // Sorulan seçenekler (ör. redis: igbinary desteği) varsayılanla yanıtlanır
    let cmd = format!("yes '' | {} -d php_ini= install {}", q(&pecl), ext.name);
    let mut c = crate::util::hidden("/bin/sh");
    // Apple Silicon'da /opt/homebrew/include derleyicinin varsayılan arama yolunda değil; PHP başlıkları
    // Homebrew'un pcre2.h'sini ister (pcov, apcu)
    c.arg("-c")
        .arg(&cmd)
        .env("PKG_CONFIG_PATH", &pkgconfig)
        .env("CPPFLAGS", format!("-I{}", prefix.join("include").display()))
        .env("LDFLAGS", format!("-L{}", prefix.join("lib").display()));
    status(&tf("msg.ext_building", &[ext.title]));
    let (code, out) = run_capture(c)?;
    log(&format!("pecl install {}:\n{}", ext.name, tail(&out)));
    if code != 0 && !out.contains("already installed") {
        bail!("pecl install {}: {}", ext.name, tail(&out));
    }
    let re = regex::Regex::new(&format!(r"install ok: channel://pecl\.php\.net/{}-(\d+\.\d+\.\d+)", ext.name)).unwrap();
    Ok(re.captures_iter(&out).last().map(|c| c[1].to_string()).unwrap_or_default())
}

#[cfg(not(windows))]
fn tail(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    lines[lines.len().saturating_sub(25)..].join("\n")
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_build_tags() {
        assert_eq!(build_tag("php-8.4.26-Win32-vs17-x64").as_deref(), Some("8.4-ts-vs17-x64"));
        assert_eq!(build_tag("php-8.4.26-nts-Win32-vs17-x64").as_deref(), Some("8.4-nts-vs17-x64"));
        assert_eq!(build_tag("php-7.4.33-Win32-vc15-x64").as_deref(), Some("7.4-ts-vc15-x64"));
        assert_eq!(build_tag("php-8.3"), None);
    }

    #[cfg(windows)]
    #[test]
    fn pecl_index_versions() {
        let html = r#"<a href="3.4.7/">3.4.7/</a><a href="3.5.0RC1/">x</a><a href="3.5.3/">x</a><a href="3.10.0/">x</a><a href="../">"#;
        assert_eq!(stable_versions(html), vec!["3.10.0", "3.5.3", "3.4.7"]);
    }
}
