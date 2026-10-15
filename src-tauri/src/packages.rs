//! Quick add: usr\packages.conf'tan indir, arşivi aç, doğru `bin\<kategori>\<sürüm>` dizinine yerleştir.

use crate::kcore::{emit, paths};
use crate::i18n::{t, tf};
use crate::util::{self, move_path, remove_dir_force};
use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

const UA: &str = "KaradelikDev/1.0 (+https://github.com/)";

#[derive(Clone, Debug, Serialize)]
pub struct Package {
    pub name: String,
    pub url: String,
    pub group: String,
    pub dest: PathBuf,
    pub installed: bool,
    /// packages.conf'ta `*` önekli (Laragon ile aynı işaret): ilk kurulumda önerilir.
    pub recommended: bool,
}

pub fn conf_path() -> PathBuf {
    paths().usr().join("packages.conf")
}

pub fn ensure_defaults() {
    let p = paths();
    let seeds: [(&str, PathBuf, &str); 5] = [
        ("packages.conf", conf_path(), include_str!("../resources/defaults/packages.conf")),
        ("sites.conf", p.usr().join("sites.conf"), include_str!("../resources/defaults/sites.conf")),
        ("Procfile", p.usr().join("Procfile"), include_str!("../resources/defaults/Procfile")),
        ("crontab", p.usr().join("crontab"), include_str!("../resources/defaults/crontab")),
        ("user.cmd", p.usr().join("user.cmd"), include_str!("../resources/defaults/user.cmd")),
    ];
    for (_, path, content) in seeds {
        if !path.exists() {
            let _ = fs::write(path, content.replace('\n', "\r\n"));
        }
    }
    // Varsayılan Document Root ilk kez oluşturuluyorsa başlangıç sayfasını koy
    let www = PathBuf::from(crate::kcore::settings().document_root);
    let index = include_str!("../resources/defaults/index.php");
    if !www.exists() && fs::create_dir_all(&www).is_ok() {
        let _ = fs::write(www.join("index.php"), index);
    }
    // 1.0.0'ın başlangıç sayfası (phpinfo'yu Host başlığına bakmadan açıyordu) değiştirilmemişse yenile
    const OLD_INDEX_SHA256: &[&str] = &["476d0a0060702270c690d74af9c42b12241dcc219743976d8b339dc5b51f0279"];
    let page = www.join("index.php");
    if let Ok(cur) = fs::read_to_string(&page) {
        use sha2::{Digest, Sha256};
        let h = hex::encode(Sha256::digest(cur.replace('\r', "").as_bytes()));
        if OLD_INDEX_SHA256.contains(&h.as_str()) {
            let _ = fs::write(&page, index);
        }
    }
}

/// "php-8.4" -> ("php", true)  /  "phpmyadmin" -> ("phpmyadmin", false)
fn category(name: &str) -> (String, CatKind) {
    let lower = name.to_lowercase();
    let head = lower.split(['-', '_']).next().unwrap_or(&lower).to_string();
    let kind = match head.as_str() {
        "php" => ("php", CatKind::Versioned),
        "apache" | "httpd" => ("apache", CatKind::Versioned),
        "nginx" => ("nginx", CatKind::Versioned),
        "mysql" | "mariadb" => ("mysql", CatKind::Versioned),
        "postgresql" | "postgres" | "pgsql" => ("postgresql", CatKind::Versioned),
        "mongodb" | "mongo" => ("mongodb", CatKind::Versioned),
        "redis" => ("redis", CatKind::Versioned),
        "memcached" => ("memcached", CatKind::Versioned),
        "node" | "nodejs" => ("nodejs", CatKind::Versioned),
        "python" => ("python", CatKind::Versioned),
        "go" | "golang" => ("go", CatKind::Versioned),
        "ruby" => ("ruby", CatKind::Versioned),
        "java" | "jdk" => ("java", CatKind::Versioned),
        "mailpit" => ("mailpit", CatKind::Versioned),
        "phpmyadmin" | "adminer" | "phpredisadmin" => (head.as_str(), CatKind::App),
        _ => (head.as_str(), CatKind::Tool),
    };
    (kind.0.to_string(), kind.1)
}

#[derive(Clone, Copy, PartialEq)]
enum CatKind {
    Versioned,
    App,
    Tool,
}

fn url_file_name(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit('/').next().unwrap_or("").to_string()
}

fn strip_archive_ext(n: &str) -> String {
    let l = n.to_lowercase();
    for ext in [".tar.gz", ".tar.xz", ".tgz", ".zip", ".7z", ".exe", ".phar", ".php"] {
        if l.ends_with(ext) {
            return n[..n.len() - ext.len()].to_string();
        }
    }
    n.to_string()
}

pub fn dest_for(name: &str, url: &str) -> PathBuf {
    let (cat, kind) = category(name);
    let p = paths();
    match kind {
        CatKind::App => p.apps().join(&cat),
        CatKind::Tool => p.bin().join(name.to_lowercase()),
        CatKind::Versioned => {
            let base = strip_archive_ext(&url_file_name(url));
            let has_version = regex::Regex::new(r"\d+\.\d+").unwrap().is_match(&base);
            let folder = if has_version && !base.contains(".jsp") { base } else { name.to_lowercase() };
            p.bin().join(cat).join(folder)
        }
    }
}

pub fn list() -> Vec<Package> {
    ensure_defaults();
    let text = fs::read_to_string(conf_path()).unwrap_or_default();
    let mut out = Vec::new();
    let mut group = String::new();
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('#') {
            let g = l.trim_start_matches('#').trim();
            if !g.is_empty() && !g.contains('=') && !g.contains("Biçim") && !g.contains("http") {
                group = g.to_string();
            }
            continue;
        }
        if l.is_empty() || l.starts_with("---") || l.starts_with(';') {
            continue;
        }
        let Some((n, u)) = l.split_once('=') else { continue };
        let recommended = n.trim().starts_with('*');
        let name = n.trim().trim_start_matches('*').to_string();
        let url = u.trim().to_string();
        if name.is_empty() || !url.starts_with("http") {
            continue;
        }
        let dest = dest_for(&name, &url);
        let installed = dest.exists();
        out.push(Package { name, url, group: group.clone(), dest, installed, recommended });
    }
    out
}

fn progress(label: &str, done: u64, total: Option<u64>) {
    let pct = total.filter(|t| *t > 0).map(|t| (done * 100 / t) as u32);
    emit("progress", serde_json::json!({ "label": label, "percent": pct, "done": done, "total": total }));
}

pub fn progress_done() {
    emit("progress", serde_json::json!({ "label": "", "percent": null, "done": 0, "total": null, "finished": true }));
}

/// İndirme; ilerleme "progress" olayıyla arayüze gider.
pub fn download(url: &str, dest: &Path, label: &str) -> Result<()> {
    if let Some(p) = dest.parent() {
        fs::create_dir_all(p)?;
    }
    let mut res = ureq::get(url).header("User-Agent", UA).call().with_context(|| format!("{}: {url}", t("msg.download_error")))?;
    let total = res.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
    let mut reader = res.body_mut().with_config().limit(u64::MAX).reader();
    let tmp = dest.with_extension("part");
    let mut f = File::create(&tmp)?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut done: u64 = 0;
    let mut last = std::time::Instant::now();
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        f.write_all(&buf[..n])?;
        done += n as u64;
        if last.elapsed().as_millis() > 200 {
            progress(&format!("{} {label}", t("progress.downloading")), done, total);
            last = std::time::Instant::now();
        }
    }
    drop(f);
    if let Some(expected) = total {
        if done != expected {
            let _ = fs::remove_file(&tmp);
            bail!(t("msg.download_incomplete"));
        }
    }
    fs::rename(&tmp, dest)?;
    Ok(())
}

pub fn fetch_text(url: &str) -> Result<String> {
    let mut res = ureq::get(url).header("User-Agent", UA).call()?;
    Ok(res.body_mut().with_config().limit(20 * 1024 * 1024).read_to_string()?)
}

#[derive(Debug, PartialEq)]
enum Format {
    Zip,
    Gz,
    Xz,
    Other,
}

fn sniff(path: &Path) -> Format {
    let mut head = [0u8; 6];
    let n = File::open(path).and_then(|mut f| f.read(&mut head)).unwrap_or(0);
    if n >= 4 && head[..4] == [0x50, 0x4b, 0x03, 0x04] {
        Format::Zip
    } else if n >= 2 && head[..2] == [0x1f, 0x8b] {
        Format::Gz
    } else if n >= 6 && head[..6] == [0xfd, 0x37, 0x7a, 0x58, 0x5a, 0x00] {
        Format::Xz
    } else {
        Format::Other
    }
}

pub fn is_archive(path: &Path) -> bool {
    sniff(path) != Format::Other
}

pub fn extract(archive: &Path, dest: &Path, label: &str) -> Result<()> {
    fs::create_dir_all(dest)?;
    progress(&format!("{} {label}", t("progress.extracting")), 0, None);
    match sniff(archive) {
        Format::Zip => {
            let mut z = zip::ZipArchive::new(BufReader::new(File::open(archive)?))?;
            let total = z.len() as u64;
            for i in 0..z.len() {
                let mut entry = z.by_index(i)?;
                let Some(rel) = entry.enclosed_name() else { continue };
                let out = dest.join(rel);
                if entry.is_dir() {
                    fs::create_dir_all(&out)?;
                } else {
                    if let Some(p) = out.parent() {
                        fs::create_dir_all(p)?;
                    }
                    let mut f = File::create(&out)?;
                    std::io::copy(&mut entry, &mut f)?;
                }
                if i % 200 == 0 {
                    progress(&format!("{} {label}", t("progress.extracting")), i as u64, Some(total));
                }
            }
        }
        Format::Gz => {
            let gz = flate2::read::GzDecoder::new(BufReader::new(File::open(archive)?));
            tar::Archive::new(gz).unpack(dest)?;
        }
        Format::Xz => {
            let mut raw = Vec::new();
            lzma_rs::xz_decompress(&mut BufReader::new(File::open(archive)?), &mut raw).map_err(|e| anyhow!("xz: {e:?}"))?;
            tar::Archive::new(std::io::Cursor::new(raw)).unpack(dest)?;
        }
        Format::Other => bail!(t("msg.unsupported_format")),
    }
    Ok(())
}

/// Arşivde tek bir üst klasör varsa içeriğini bir üst seviyeye taşır (Apache24/, wordpress/ ...).
pub fn hoist(dir: &Path) -> Result<()> {
    let entries: Vec<PathBuf> = fs::read_dir(dir)?.flatten().map(|e| e.path()).collect();
    let dirs: Vec<&PathBuf> = entries.iter().filter(|p| p.is_dir()).collect();
    if dirs.len() == 1 {
        let inner = dirs[0].clone();
        let tmp = dir.with_file_name(format!("{}.hoist", dir.file_name().unwrap().to_string_lossy()));
        fs::rename(&inner, &tmp)?;
        remove_dir_force(dir)?;
        fs::rename(&tmp, dir)?;
    }
    Ok(())
}

/// URL'yi indirip `dest` dizinine açar (veya tek dosyayı kopyalar).
pub fn fetch_into(url: &str, dest: &Path, label: &str, single_file_name: Option<&str>, sha256: Option<&str>) -> Result<()> {
    let p = paths();
    let mut fname = url_file_name(url);
    if fname.is_empty() || fname.contains(".jsp") {
        fname = format!("{label}.download");
    }
    let file = p.tmp().join("downloads").join(&fname);
    download(url, &file, label)?;
    if let Some(expected) = sha256 {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        std::io::copy(&mut File::open(&file)?, &mut h)?;
        let got = hex::encode(h.finalize());
        if !got.eq_ignore_ascii_case(expected) {
            let _ = fs::remove_file(&file);
            bail!("SHA-256 uyuşmuyor: {label}");
        }
    }
    let staging = p.tmp().join("extract").join(format!("{label}-{}", util::now_stamp()));
    let result = (|| -> Result<()> {
        if is_archive(&file) {
            extract(&file, &staging, label)?;
            hoist(&staging)?;
            move_path(&staging, dest)?;
        } else {
            fs::create_dir_all(dest)?;
            let target = dest.join(single_file_name.unwrap_or(&fname));
            fs::copy(&file, &target)?;
        }
        Ok(())
    })();
    let _ = fs::remove_file(&file);
    let _ = remove_dir_force(&staging);
    progress_done();
    result
}

pub fn install(name: &str) -> Result<PathBuf> {
    let pkg = list().into_iter().find(|p| p.name == name).ok_or_else(|| anyhow!("{name}?"))?;
    if pkg.installed {
        bail!(tf("msg.already_installed", &[&pkg.name]));
    }
    let lower = pkg.name.to_lowercase();
    // Tek dosyalık araçlar bilinen adla kaydedilir (ör. cloudflared-windows-amd64.exe -> cloudflared.exe);
    // paylaşım ve terminal bu adla arar.
    let exe_name = format!("{lower}.exe");
    let single = if lower == "adminer" {
        Some("index.php")
    } else if lower == "composer" {
        Some("composer.phar")
    } else if url_file_name(&pkg.url).to_lowercase().ends_with(".exe") {
        Some(exe_name.as_str())
    } else {
        None
    };
    fetch_into(&pkg.url, &pkg.dest, &pkg.name, single, None)?;
    if lower == "composer" {
        fs::write(pkg.dest.join("composer.bat"), "@php \"%~dp0composer.phar\" %*\r\n")?;
    }
    Ok(pkg.dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(strip_archive_ext("php-8.4.26-Win32-vs17-x64.zip"), "php-8.4.26-Win32-vs17-x64");
        assert_eq!(strip_archive_ext("latest.tar.gz"), "latest");
        assert_eq!(url_file_name("https://x/y/getfile.jsp?fileid=1"), "getfile.jsp");
        assert_eq!(category("mariadb-11.4").0, "mysql");
        assert_eq!(category("node-24.22").0, "nodejs");
    }
}
