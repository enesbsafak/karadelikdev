//! Quick add: usr\packages.conf'tan indir, arşivi aç, doğru `bin\<kategori>\<sürüm>` dizinine yerleştir.
//! macOS'ta çoğu paket `brew:<formül>` biçimindedir: Homebrew ile kurulur ve `bin/<kategori>/<ad>-<x.y>`
//! Homebrew'un `opt/<formül>` klasörüne sembolik bağlantı olur (sürüm menüleri değişmeden çalışır).

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
    #[cfg(windows)]
    let (packages, crontab, user) = (
        include_str!("../resources/defaults/packages.conf"),
        include_str!("../resources/defaults/crontab"),
        include_str!("../resources/defaults/user.cmd"),
    );
    #[cfg(not(windows))]
    let (packages, crontab, user) = (
        include_str!("../resources/defaults/packages.mac.conf"),
        include_str!("../resources/defaults/crontab.mac"),
        include_str!("../resources/defaults/user.sh"),
    );
    let seeds: [(&str, PathBuf, &str); 5] = [
        ("packages.conf", conf_path(), packages),
        ("sites.conf", p.usr().join("sites.conf"), include_str!("../resources/defaults/sites.conf")),
        ("Procfile", p.usr().join("Procfile"), include_str!("../resources/defaults/Procfile")),
        ("crontab", p.usr().join("crontab"), crontab),
        ("user script", crate::terminal::user_script_path(), user),
    ];
    for (_, path, content) in seeds {
        if !path.exists() {
            let _ = fs::write(path, content.replace('\n', crate::platform::LINE_END));
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
    if let Some(formula) = url.strip_prefix("brew:") {
        // Kuruluysa onu gösteren bağlantı; değilse kurulumda sürümle adlandırılacak klasörün yer tutucusu
        return brew_link(&cat, formula).unwrap_or_else(|| p.bin().join(&cat).join(name.to_lowercase()));
    }
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
        let url = u.trim().replace("{arch}", if cfg!(target_arch = "aarch64") { "arm64" } else { "amd64" });
        if name.is_empty() || !(url.starts_with("http") || url.starts_with("brew:")) {
            continue;
        }
        let mut dest = dest_for(&name, &url);
        // PHP ve Apache: aynı dalın (ör. PHP 8.4 TS vs17) herhangi bir yaması kuruluysa paket kurulu sayılır;
        // kurulum güncel yamayı indirdiği için klasör adı packages.conf'takiyle aynı olmayabilir.
        if let Some((src, b)) = rolling(&url) {
            if let Some(f) = crate::versions::installed(src.cat())
                .into_iter()
                .filter_map(|f| src.build(&f).filter(|x| x.tag == b.tag).map(|x| (x.patch, f)))
                .max()
            {
                dest = paths().bin().join(src.cat()).join(f.1);
            }
        }
        let installed = dest.exists() && (!url.starts_with("brew:") || brew_link(&category(&name).0, &url[5..]).is_some());
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
                    // Unix izinleri (ör. ngrok ikilisi çalıştırılabilir kalsın)
                    #[cfg(unix)]
                    if let Some(mode) = entry.unix_mode() {
                        use std::os::unix::fs::PermissionsExt;
                        let _ = fs::set_permissions(&out, fs::Permissions::from_mode(mode & 0o777));
                    }
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

pub fn set_executable(_path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(_path, fs::Permissions::from_mode(0o755));
    }
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

// ---------------------------------------------------------------- Homebrew

/// Formülün `opt` klasörünün adı: `mongodb/brew/mongodb-community@8.0` -> `mongodb-community@8.0`.
fn formula_short(formula: &str) -> &str {
    formula.rsplit('/').next().unwrap_or(formula)
}

/// `bin/<kategori>` altında bu formülün `opt` klasörüne giden bağlantı (kurulu sürüm klasörü).
pub fn brew_link(cat: &str, formula: &str) -> Option<PathBuf> {
    let short = formula_short(formula);
    fs::read_dir(paths().bin().join(cat)).ok()?.flatten().map(|e| e.path()).find(|p| {
        fs::read_link(p).map(|t| t.ends_with(Path::new("opt").join(short))).unwrap_or(false) && p.exists()
    })
}

/// `opt/<formül>` bağlantısının gösterdiği Cellar sürümü: "8.4.26", "8.5.11_1" -> "8.4", "8.5".
#[cfg_attr(windows, allow(dead_code))]
fn major_minor(version: &str) -> String {
    let nums: Vec<&str> = version.split(['.', '_', '-']).take_while(|p| p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty()).take(2).collect();
    nums.join(".")
}

/// Homebrew ile kurar (kuruluysa yalnız bağlar) ve `bin/<kategori>/<ad>-<x.y>` bağlantısını oluşturur.
#[cfg(not(windows))]
fn brew_install(cat: &str, formula: &str, label: &str) -> Result<PathBuf> {
    use std::io::BufRead;
    let brew = crate::platform::brew().ok_or_else(|| anyhow!(t("setup.brew_missing")))?;
    let prefix = crate::platform::brew_prefix().unwrap();
    let short = formula_short(formula);
    let opt = prefix.join("opt").join(short);
    if !opt.exists() {
        let log = paths().logs().join("brew.log");
        // (argümanlar, hata olursa dur)
        let mut steps: Vec<(Vec<String>, bool)> = vec![];
        // Üçüncü taraf tap (ör. mongodb/brew/...) önce eklenir. Homebrew 6+ güvenilmeyen tap'lerden formül yüklemez;
        // kullanıcı paketi katalogdan seçtiği için tap'e (bağımlılıklar da oradan gelir, ör. mongodb-database-tools)
        // güvenilir. Eski Homebrew'da `brew trust` yoktur; o adımın hatası yok sayılır.
        let parts: Vec<&str> = formula.split('/').collect();
        if parts.len() == 3 {
            let tap = format!("{}/{}", parts[0], parts[1]);
            steps.push((vec!["tap".into(), tap.clone()], true));
            steps.push((vec!["trust".into(), "--tap".into(), tap], false));
        }
        steps.push((vec!["install".into(), formula.into()], true));
        for (args, required) in steps {
            util::append_line(&log, &format!("$ brew {}", args.join(" ")));
            let mut c = crate::platform::shell(&format!(
                "{} {} 2>&1",
                crate::platform::sh_quote(&brew.display().to_string()),
                args.iter().map(|a| crate::platform::sh_quote(a)).collect::<Vec<_>>().join(" ")
            ));
            c.env("HOMEBREW_NO_ENV_HINTS", "1").env("NONINTERACTIVE", "1").stdout(std::process::Stdio::piped()).stdin(std::process::Stdio::null());
            let mut child = c.spawn().context("brew")?;
            let out = child.stdout.take().unwrap();
            let mut tail = Vec::new();
            for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                util::append_line(&log, &line);
                let l = line.trim().to_string();
                if !l.is_empty() {
                    progress(&format!("Homebrew {label}: {}", l.chars().take(90).collect::<String>()), 0, None);
                    tail.push(l);
                    if tail.len() > 6 {
                        tail.remove(0);
                    }
                }
            }
            if !child.wait()?.success() && required {
                // Formül kuruldu ama ortak dosyaları bağlanamadı (ör. include/mysql başka bir MySQL'e bağlı):
                // KaradelikDev yalnız opt/<formül> klasörünü kullanır
                if args[0] == "install" && opt.exists() {
                    crate::kcore::log(&format!("brew install {formula}: bağlama uyarısı (opt/{short} kullanılıyor): {}", tail.join(" | ")));
                    continue;
                }
                bail!("brew {}:\n{}", args.join(" "), tail.join("\n"));
            }
        }
    }
    let real = fs::canonicalize(&opt).with_context(|| opt.display().to_string())?;
    let version = real.file_name().map(|v| v.to_string_lossy().to_string()).unwrap_or_default();
    let base = short.split('@').next().unwrap_or(short);
    let mm = major_minor(&version);
    let folder = if mm.is_empty() { base.to_string() } else { format!("{base}-{mm}") };
    let dest = paths().bin().join(cat).join(folder);
    fs::create_dir_all(dest.parent().unwrap())?;
    if fs::symlink_metadata(&dest).is_ok() {
        fs::remove_file(&dest).with_context(|| dest.display().to_string())?;
    }
    std::os::unix::fs::symlink(&opt, &dest).with_context(|| dest.display().to_string())?;
    crate::kcore::log(&format!("Homebrew {formula} {version} -> {}", dest.display()));
    Ok(dest)
}

#[cfg(windows)]
fn brew_install(_cat: &str, _formula: &str, _label: &str) -> Result<PathBuf> {
    bail!("Homebrew yalnız macOS'ta")
}

/// Homebrew'da kurulu ama henüz bağlanmamış katalog paketleri (sihirbaz bunları varsayılan olarak seçer).
pub fn brew_available(pkg: &Package) -> bool {
    match (pkg.url.strip_prefix("brew:"), brew_prefix_any()) {
        (Some(f), Some(prefix)) => !pkg.installed && prefix.join("opt").join(formula_short(f)).exists(),
        _ => false,
    }
}

fn brew_prefix_any() -> Option<PathBuf> {
    #[cfg(not(windows))]
    return crate::platform::brew_prefix();
    #[cfg(windows)]
    None
}

// ---------------------------------------------------------------- Güncel derlemesi aranan kaynaklar (PHP, Apache)

/// PHP'nin Windows derlemeleri (windows.php.net/downloads/releases buraya yönlendiriyor).
pub const PHP_WIN: &str = "https://downloads.php.net/~windows/releases";

/// Derleme klasörü/dosya adının çözümlemesi (PHP ve Apache).
#[derive(Debug, PartialEq)]
pub struct Build {
    /// `8.4-ts-vs17-x64`: alt sürüm, iş parçacığı güvenliği, derleyici, mimari.
    pub tag: String,
    /// (yama, yeniden derleme): `8.4.27-1` -> (27, 1), `8.4.26` -> (26, 0).
    pub patch: (u32, u32),
}

/// `php-8.4.27-1-nts-Win32-vs17-x64` (veya `.zip` uzantılı) -> etiket ve yama.
pub fn php_build(folder: &str) -> Option<Build> {
    let re = regex::Regex::new(r"(?i)^php-(\d+\.\d+)\.(\d+)(?:-(\d+))?(-nts)?-Win32-(v[cs]\d+)-(x64|x86)$").unwrap();
    let c = re.captures(folder)?;
    let ts = if c.get(4).is_some() { "nts" } else { "ts" };
    let patch = (c[2].parse().ok()?, c.get(3).map_or(Some(0), |m| m.as_str().parse().ok())?);
    Some(Build { tag: format!("{}-{ts}-{}-{}", &c[1], c[5].to_lowercase(), c[6].to_lowercase()), patch })
}

/// releases.json'daki sürüm metni: `8.4.27-1` -> (27, 1).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn php_patch_of(version: &str) -> (u32, u32) {
    let last = version.splitn(3, '.').nth(2).unwrap_or("");
    let (p, b) = last.split_once('-').unwrap_or((last, "0"));
    (p.parse().unwrap_or(0), b.parse().unwrap_or(0))
}

/// `httpd-2.4.69-261002-Win64-VS18` -> etiket `2.4-win64`, yama (69, 261002).
fn apache_build(folder: &str) -> Option<Build> {
    let re = regex::Regex::new(r"(?i)^httpd-(\d+\.\d+)\.(\d+)-(\d+)-(win64|win32)-(v[cs]\d+)$").unwrap();
    let c = re.captures(folder)?;
    Some(Build { tag: format!("{}-{}", &c[1], c[4].to_lowercase()), patch: (c[2].parse().ok()?, c[3].parse().ok()?) })
}

const APACHE_LOUNGE: &str = "https://www.apachelounge.com";

/// Dosyaları sabit adreste kalmayan kaynaklar: yeni yama çıkınca eski dosya kaldırılır veya taşınır,
/// packages.conf'taki (kullanıcının veri dizinine bir kez kopyalanan) adres 404 verir.
/// Bunlar için kurulumda kaynağın güncel derlemesi aranır.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Rolling {
    /// downloads.php.net/~windows (eski adı windows.php.net): yeni yamada eski dosya archives/'a taşınır.
    PhpWin,
    /// Apache Lounge yalnız son derlemeyi tutar.
    ApacheLounge,
}

impl Rolling {
    fn cat(self) -> &'static str {
        match self {
            Rolling::PhpWin => "php",
            Rolling::ApacheLounge => "apache",
        }
    }

    fn build(self, folder: &str) -> Option<Build> {
        match self {
            Rolling::PhpWin => php_build(folder),
            Rolling::ApacheLounge => apache_build(folder),
        }
    }

    /// Denenecek (adres, SHA-256) çiftleri, sırayla: önce kaynağın güncel derlemesi, sonra packages.conf'taki dosya.
    fn candidates(self, url: &str, tag: &str) -> Vec<(String, Option<String>)> {
        let file = url_file_name(url);
        let mut out = Vec::new();
        match self {
            Rolling::PhpWin => {
                // Ömrü biten alt sürümler releases.json'dan kalkar; archives/ çıkan her derlemeyi tutar.
                let (minor, key) = tag.split_once('-').unwrap_or((tag, ""));
                if let Ok(rel) = fetch_text(&format!("{PHP_WIN}/releases.json")).and_then(|b| Ok(serde_json::from_str::<serde_json::Value>(&b)?)) {
                    let zip = &rel[minor][key]["zip"];
                    if let Some(path) = zip["path"].as_str().filter(|p| !p.contains(['/', '\\'])) {
                        out.push((format!("{PHP_WIN}/{path}"), zip["sha256"].as_str().map(str::to_string)));
                    }
                }
                out.push((format!("{PHP_WIN}/{file}"), None));
                out.push((format!("{PHP_WIN}/archives/{file}"), None));
            }
            Rolling::ApacheLounge => {
                if let Ok(page) = fetch_text(&format!("{APACHE_LOUNGE}/download/")) {
                    let re = regex::Regex::new(r#"(?i)href="(/download/VS\d+/binaries/(httpd-[0-9.]+-\d+-win(?:64|32)-vs\d+)\.zip)""#).unwrap();
                    let best = re
                        .captures_iter(&page)
                        .filter_map(|c| apache_build(&c[2]).filter(|b| b.tag == tag).map(|b| (b.patch, c[1].to_string())))
                        .max();
                    if let Some((_, path)) = best {
                        out.push((format!("{APACHE_LOUNGE}{path}"), None));
                    }
                }
                out.push((url.to_string(), None));
            }
        }
        out.dedup();
        out
    }
}

/// Bağlantı güncel derlemesi aranacak bir kaynaktan mı? (kaynak, packages.conf'taki derleme)
fn rolling(url: &str) -> Option<(Rolling, Build)> {
    let l = url.to_lowercase();
    let src = if l.contains("windows.php.net/downloads/releases/") || l.contains("downloads.php.net/~windows/releases/") {
        Rolling::PhpWin
    } else if l.starts_with("https://www.apachelounge.com/download/") {
        Rolling::ApacheLounge
    } else {
        return None;
    };
    Some((src, src.build(&strip_archive_ext(&url_file_name(url)))?))
}

pub fn install(name: &str) -> Result<PathBuf> {
    let pkg = list().into_iter().find(|p| p.name == name).ok_or_else(|| anyhow!("{name}?"))?;
    if pkg.installed {
        bail!(tf("msg.already_installed", &[&pkg.name]));
    }
    if let Some((src, b)) = rolling(&pkg.url) {
        let mut last = None;
        for (url, sha) in src.candidates(&pkg.url, &b.tag) {
            let dest = paths().bin().join(src.cat()).join(strip_archive_ext(&url_file_name(&url)));
            match fetch_into(&url, &dest, &pkg.name, None, sha.as_deref()) {
                Ok(()) => return Ok(dest),
                Err(e) => {
                    crate::kcore::log(&format!("{}: {e:#}", pkg.name));
                    last = Some(e);
                }
            }
        }
        return Err(last.unwrap_or_else(|| anyhow!(t("msg.download_error"))));
    }
    if let Some(formula) = pkg.url.strip_prefix("brew:") {
        let dest = brew_install(&category(&pkg.name).0, formula, &pkg.name)?;
        progress_done();
        return Ok(dest);
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
        if cfg!(windows) {
            fs::write(pkg.dest.join("composer.bat"), "@php \"%~dp0composer.phar\" %*\r\n")?;
        } else {
            let sh = pkg.dest.join("composer");
            fs::write(&sh, "#!/bin/sh\nexec php \"$(dirname \"$0\")/composer.phar\" \"$@\"\n")?;
            set_executable(&sh);
        }
    }
    Ok(pkg.dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brew_names() {
        assert_eq!(major_minor("8.4.26"), "8.4");
        assert_eq!(major_minor("8.5.11_1"), "8.5");
        assert_eq!(major_minor("27"), "27");
        assert_eq!(formula_short("mongodb/brew/mongodb-community@8.0"), "mongodb-community@8.0");
    }

    #[test]
    fn names() {
        assert_eq!(strip_archive_ext("php-8.4.26-Win32-vs17-x64.zip"), "php-8.4.26-Win32-vs17-x64");
        assert_eq!(strip_archive_ext("latest.tar.gz"), "latest");
        assert_eq!(url_file_name("https://x/y/getfile.jsp?fileid=1"), "getfile.jsp");
        assert_eq!(category("mariadb-11.4").0, "mysql");
        assert_eq!(category("node-24.22").0, "nodejs");
    }

    #[test]
    fn php_windows_builds() {
        let b = php_build("php-8.4.27-1-nts-Win32-vs17-x64").unwrap();
        assert_eq!(b.tag, "8.4-nts-vs17-x64");
        assert_eq!(b.patch, (27, 1));
        assert_eq!(php_build("php-8.4.26-Win32-vs17-x64").unwrap().patch, (26, 0));
        assert_eq!(php_build("php-8.4"), None);
        assert_eq!(php_patch_of("8.4.27-1"), (27, 1));
        assert_eq!(php_patch_of("8.3.35"), (35, 0));
        assert!(php_patch_of("8.4.27-1") > (27, 0));
        let old = "https://windows.php.net/downloads/releases/php-8.4.26-Win32-vs17-x64.zip";
        assert_eq!(rolling(old).unwrap().1.tag, "8.4-ts-vs17-x64");
        let new = "https://downloads.php.net/~windows/releases/archives/php-8.2.34-nts-Win32-vs16-x64.zip";
        assert_eq!(rolling(new).unwrap(), (Rolling::PhpWin, php_build("php-8.2.34-nts-Win32-vs16-x64").unwrap()));
        assert!(rolling("https://example.com/php-8.4.26-Win32-vs17-x64.zip").is_none());
    }

    #[test]
    fn apache_lounge_builds() {
        let b = apache_build("httpd-2.4.69-261002-Win64-VS18").unwrap();
        assert_eq!(b.tag, "2.4-win64");
        assert_eq!(b.patch, (69, 261002));
        assert!(apache_build("httpd-2.4.70-261115-Win64-VS18").unwrap().patch > b.patch);
        let url = "https://www.apachelounge.com/download/VS18/binaries/httpd-2.4.69-261002-Win64-VS18.zip";
        assert_eq!(rolling(url).unwrap().0, Rolling::ApacheLounge);
        assert!(rolling("https://nginx.org/download/nginx-1.30.6.zip").is_none());
    }

    /// Ağ gerektirir: `cargo test --lib -- --ignored rolling_sources_resolve`
    /// (haftalık CI: .github/workflows/url-check.yml). Varsayılan packages.conf'taki PHP ve Apache adresleri
    /// ile kaldırılmış eski adresler, kaynağın güncel ve indirilebilir derlemesine çözülmeli.
    #[test]
    #[ignore]
    fn rolling_sources_resolve() {
        let defaults = include_str!("../resources/defaults/packages.conf");
        let mut urls: Vec<&str> = defaults
            .lines()
            .filter_map(|l| l.split_once('=').map(|(_, u)| u.trim()))
            .filter(|u| rolling(u).is_some())
            .collect();
        assert!(urls.len() >= 5, "packages.conf'ta PHP/Apache adresi bulunamadı");
        let removed = [
            "https://windows.php.net/downloads/releases/php-8.4.26-Win32-vs17-x64.zip",
            "https://windows.php.net/downloads/releases/php-8.4.26-nts-Win32-vs17-x64.zip",
            "https://www.apachelounge.com/download/VS17/binaries/httpd-2.4.60-240701-Win64-VS17.zip",
        ];
        urls.extend(removed);
        let mut failed = Vec::new();
        for url in urls {
            let (src, b) = rolling(url).unwrap();
            let c = src.candidates(url, &b.tag);
            println!("{url}\n  -> {c:?}");
            let first = &c[0].0;
            // PHP: releases.json'dan SHA-256'lı derleme; kaldırılmış Apache: sayfadan farklı (yeni) derleme
            let resolved = match src {
                Rolling::PhpWin => c[0].1.is_some(),
                Rolling::ApacheLounge => !removed.contains(&url) || first != url,
            };
            let code = ureq::head(first).header("User-Agent", UA).call().map(|r| r.status().as_u16()).unwrap_or(0);
            let same_branch = src.build(&strip_archive_ext(&url_file_name(first))).is_some_and(|x| x.tag == b.tag);
            if !resolved || code != 200 || !same_branch {
                failed.push(format!("{url} -> {first} (çözüldü: {resolved}, HTTP {code}, aynı dal: {same_branch})"));
            }
        }
        assert!(failed.is_empty(), "çözülemeyen adresler:\n{}", failed.join("\n"));
    }
}
