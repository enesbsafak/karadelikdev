//! KaradelikDev'in kendini güncellemesi. GitHub'daki son sürümün `latest.json` dosyası okunur; zip indirilip
//! SHA-256 ile doğrulanır, exe değiştirilir ve uygulama yeniden başlatılır. Önceki exe `<ad>.exe.old` olarak
//! saklanır; "Önceki sürüme dön" ikisini yer değiştirir. Çalışan exe Windows'ta silinemez ama yeniden adlandırılabilir.

use crate::kcore::{log, paths};
use crate::services::{self, Svc};
use anyhow::{anyhow, bail, Context, Result};
use parking_lot::Mutex;
use serde::Deserialize;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

const LATEST_URL: &str = "https://github.com/enesbsafak/karadelikdev/releases/latest/download/latest.json";

#[derive(Clone, Debug, Deserialize)]
pub struct Latest {
    pub version: String,
    /// Zip paketi (exe, LICENSE, README).
    pub url: String,
    /// Zip'in SHA-256 özeti.
    pub sha256: String,
    /// Zip içindeki exe'nin SHA-256 özeti.
    #[serde(default)]
    pub exe_sha256: String,
    #[serde(default)]
    pub notes: String,
}

/// Bu oturumda bulunan yeni sürüm (menü ve ana pencere gösterir).
static AVAILABLE: Mutex<Option<Latest>> = Mutex::new(None);

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn available() -> Option<Latest> {
    AVAILABLE.lock().clone()
}

/// "1.10.0" > "1.9.2"; yalnız sayısal parçalar karşılaştırılır ("v" öneki yok sayılır).
pub fn is_newer(remote: &str, current: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> { v.trim().trim_start_matches('v').split(['.', '-']).map_while(|p| p.parse().ok()).collect() };
    let (r, c) = (parts(remote), parts(current));
    !r.is_empty() && r > c
}

fn fetch_latest() -> Result<Latest> {
    // Geliştirme derlemesinde yerel sunucuyla denemek için: KARADELIK_UPDATE_URL=http://127.0.0.1:8765/latest.json
    let test_url = std::env::var("KARADELIK_UPDATE_URL").ok().filter(|_| cfg!(debug_assertions));
    let body = crate::packages::fetch_text(test_url.as_deref().unwrap_or(LATEST_URL))?;
    let l: Latest = serde_json::from_str(&body).context("latest.json")?;
    if test_url.is_none() && !l.url.starts_with("https://github.com/enesbsafak/karadelikdev/releases/download/") {
        bail!("latest.json: beklenmeyen indirme adresi {}", l.url);
    }
    if l.sha256.len() != 64 {
        bail!("latest.json: SHA-256 eksik");
    }
    Ok(l)
}

/// Yeni sürüm varsa döndürür ve hatırlar.
pub fn check() -> Result<Option<Latest>> {
    let l = fetch_latest()?;
    let newer = is_newer(&l.version, current_version());
    *AVAILABLE.lock() = newer.then(|| l.clone());
    Ok(newer.then_some(l))
}

/// Açılışta ve günde bir: yeni sürüm varsa bir kez bildirir.
pub fn background_check() {
    static NOTIFIED: Mutex<Option<String>> = Mutex::new(None);
    match check() {
        Ok(Some(l)) => {
            let mut n = NOTIFIED.lock();
            if n.as_deref() != Some(l.version.as_str()) {
                *n = Some(l.version.clone());
                crate::kcore::notify(&crate::i18n::tf("msg.app_update_available", &[&l.version]));
                services::broadcast();
            }
        }
        Ok(None) => {}
        Err(e) => log(&format!("Güncelleme denetimi: {e:#}")),
    }
}

fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    std::io::copy(&mut File::open(path)?, &mut h)?;
    Ok(hex::encode(h.finalize()))
}

pub fn exe_path() -> Result<PathBuf> {
    Ok(crate::paths::strip_unc(std::env::current_exe()?))
}

fn old_path(exe: &Path) -> PathBuf {
    exe.with_file_name(format!("{}.old", exe.file_name().unwrap_or_default().to_string_lossy()))
}

pub fn rollback_available() -> bool {
    exe_path().map(|e| old_path(&e).is_file()).unwrap_or(false)
}

/// İndirir, doğrular ve exe'yi değiştirir. Yeniden başlatma `restart` ile yapılır.
pub fn install(l: &Latest) -> Result<()> {
    let dir = paths().tmp().join("update");
    let _ = crate::util::remove_dir_force(&dir);
    fs::create_dir_all(&dir)?;
    let zip_path = dir.join(format!("KaradelikDev-{}.zip", l.version));
    crate::packages::download(&l.url, &zip_path, &format!("KaradelikDev {}", l.version))?;
    crate::packages::progress_done();
    if !sha256_file(&zip_path)?.eq_ignore_ascii_case(&l.sha256) {
        bail!(crate::i18n::t("msg.update_bad_hash"));
    }
    let exe = exe_path()?;
    let exe_name = exe.file_name().unwrap_or_default().to_string_lossy().to_string();
    let new_exe = dir.join(&exe_name);
    {
        let mut z = zip::ZipArchive::new(std::io::BufReader::new(File::open(&zip_path)?))?;
        let idx = (0..z.len())
            .find(|&i| {
                z.by_index(i)
                    .ok()
                    .and_then(|e| e.enclosed_name())
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case("karadelikdev.exe")))
                    .unwrap_or(false)
            })
            .ok_or_else(|| anyhow!("zip: karadelikdev.exe yok"))?;
        let mut entry = z.by_index(idx)?;
        std::io::copy(&mut entry, &mut File::create(&new_exe)?)?;
    }
    if !l.exe_sha256.is_empty() && !sha256_file(&new_exe)?.eq_ignore_ascii_case(&l.exe_sha256) {
        bail!(crate::i18n::t("msg.update_bad_hash"));
    }
    replace_exe(&exe, &new_exe)?;
    let _ = crate::util::remove_dir_force(&dir);
    log(&format!("KaradelikDev {} -> {} güncellendi", current_version(), l.version));
    Ok(())
}

/// Çalışan exe'yi `.old` yapar, yenisini yerine koyar; başarısızsa eskiyi geri getirir.
fn replace_exe(exe: &Path, new_exe: &Path) -> Result<()> {
    let old = old_path(exe);
    if old.exists() {
        fs::remove_file(&old).with_context(|| format!("silinemedi: {}", old.display()))?;
    }
    fs::rename(exe, &old).with_context(|| format!("yeniden adlandırılamadı: {}", exe.display()))?;
    if let Err(e) = fs::copy(new_exe, exe) {
        let _ = fs::remove_file(exe);
        let _ = fs::rename(&old, exe);
        return Err(anyhow!(e).context(format!("kopyalanamadı: {}", exe.display())));
    }
    Ok(())
}

/// Mevcut exe ile `.old`'u yer değiştirir; tekrar çağrılırsa yeni sürüme geri döner.
pub fn rollback() -> Result<()> {
    let exe = exe_path()?;
    let old = old_path(&exe);
    if !old.is_file() {
        bail!(crate::i18n::t("msg.no_previous_version"));
    }
    let swap = exe.with_file_name(format!("{}.swap", exe.file_name().unwrap_or_default().to_string_lossy()));
    let _ = fs::remove_file(&swap);
    fs::rename(&exe, &swap)?;
    if let Err(e) = fs::rename(&old, &exe) {
        let _ = fs::rename(&swap, &exe);
        return Err(e.into());
    }
    fs::rename(&swap, &old)?;
    log("Önceki KaradelikDev sürümüne dönüldü");
    Ok(())
}

/// Servisleri durdurur, bu süreç kapandıktan sonra exe'yi yeniden başlatacak gizli bir PowerShell bırakır ve çıkar
/// (tek örnek kilidi yüzünden yeni süreç eskisi açıkken başlatılamaz). `resume` ise yeni süreç çalışan servisleri
/// yeniden başlatır; eski sürümler `--updated` argümanını tanımadığı için geri dönüşte kullanılmaz.
pub fn restart(resume: bool) -> Result<()> {
    let running: Vec<&str> = Svc::START_ORDER.into_iter().filter(|s| services::is_running(*s) && services::owned_by_us(*s)).map(|s| s.id()).collect();
    let list = if running.is_empty() { "-".to_string() } else { running.join(",") };
    crate::tunnel::stop();
    services::stop_all();
    let exe = exe_path()?;
    let q = |s: &str| format!("'{}'", s.replace('\'', "''"));
    let args = if resume { format!(" -ArgumentList '--updated',{}", q(&list)) } else { String::new() };
    let script = format!(
        "Wait-Process -Id {} -Timeout 30 -ErrorAction SilentlyContinue; Start-Process -FilePath {}{args}",
        std::process::id(),
        q(&exe.display().to_string())
    );
    crate::util::hidden("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .spawn()
        .context("powershell")?;
    if let Some(a) = crate::kcore::app() {
        a.exit(0);
    }
    std::thread::sleep(std::time::Duration::from_secs(3));
    std::process::exit(0);
}

/// Güncellemeden sonra açılışta: önceden çalışan servisleri başlatır.
pub fn resume_services(list: &str) {
    let svcs: Vec<Svc> = list.split(',').filter_map(Svc::from_id).collect();
    if svcs.iter().any(|s| matches!(s, Svc::Apache | Svc::Nginx)) {
        services::prepare_web();
    }
    for s in svcs {
        if let Err(e) = services::start(s) {
            crate::kcore::error(s.title(), &e);
        }
    }
    crate::kcore::notify(&crate::i18n::tf("msg.app_updated", &[current_version()]));
    services::broadcast();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("1.1.0", "1.0.0"));
        assert!(is_newer("v1.10.0", "1.9.9"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("0.9.0", "1.0.0"));
        assert!(!is_newer("", "1.0.0"));
    }

    #[test]
    fn swap_and_rollback() {
        let dir = std::env::temp_dir().join(format!("kd-upd-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("karadelikdev.exe");
        let new = dir.join("new.exe");
        fs::write(&exe, "v1").unwrap();
        fs::write(&new, "v2").unwrap();
        replace_exe(&exe, &new).unwrap();
        assert_eq!(fs::read_to_string(&exe).unwrap(), "v2");
        assert_eq!(fs::read_to_string(old_path(&exe)).unwrap(), "v1");
        fs::remove_dir_all(&dir).unwrap();
    }
}
