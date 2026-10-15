//! KaradelikDev'in kendini güncellemesi. GitHub'daki son sürümün `latest.json` dosyası okunur; kurulum programı
//! (setup.exe) indirilip SHA-256 ile doğrulanır. Uygulama servisleri durdurup kapanır; gizli bir PowerShell
//! setup'ı sessizce (`/S /UPDATE`) çalıştırır ve yeni sürümü açar. Taşınabilir kurulumdan gelen kullanıcıda
//! setup normal (`/S`) çalışır ve eski klasör veri klasörü olarak kaydedilir.
//! Geri dönüş için güncellemeden önceki exe `backup\app` altında saklanır.

use crate::kcore::{log, paths};
use crate::services::{self, Svc};
use anyhow::{bail, Context, Result};
use parking_lot::Mutex;
use serde::Deserialize;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

const LATEST_URL: &str = "https://github.com/enesbsafak/karadelikdev/releases/latest/download/latest.json";

#[derive(Clone, Debug, Deserialize)]
pub struct Latest {
    pub version: String,
    /// Kurulum programı (KaradelikDev-<sürüm>-setup.exe).
    pub url: String,
    /// Kurulum programının SHA-256 özeti.
    pub sha256: String,
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
    if !l.url.to_lowercase().ends_with(".exe") || l.sha256.len() != 64 {
        bail!("latest.json: kurulum programı veya SHA-256 eksik");
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

fn backup_dir() -> PathBuf {
    paths().backup().join("app")
}

fn backup_exe() -> PathBuf {
    backup_dir().join("karadelikdev.exe")
}

/// Geri dönülebilecek önceki sürüm (varsa sürüm numarasıyla).
pub fn previous_version() -> Option<String> {
    backup_exe().is_file().then(|| fs::read_to_string(backup_dir().join("version.txt")).unwrap_or_default().trim().to_string())
}

pub fn rollback_available() -> bool {
    backup_exe().is_file()
}

/// Kurulum programını indirir ve doğrular; çalışan exe'yi geri dönüş için yedekler.
pub fn download(l: &Latest) -> Result<PathBuf> {
    let dir = paths().tmp().join("update");
    let _ = crate::util::remove_dir_force(&dir);
    fs::create_dir_all(&dir)?;
    let setup = dir.join(format!("KaradelikDev-{}-setup.exe", l.version));
    crate::packages::download(&l.url, &setup, &format!("KaradelikDev {}", l.version))?;
    crate::packages::progress_done();
    if !sha256_file(&setup)?.eq_ignore_ascii_case(&l.sha256) {
        let _ = fs::remove_file(&setup);
        bail!(crate::i18n::t("msg.update_bad_hash"));
    }
    fs::create_dir_all(backup_dir())?;
    fs::copy(exe_path()?, backup_exe()).context("exe yedeklenemedi")?;
    fs::write(backup_dir().join("version.txt"), current_version())?;
    Ok(setup)
}

/// Önceki exe'ye döner: çalışan exe yeniden adlandırılır (silinemez ama adı değişebilir), yedek yerine kopyalanır,
/// şimdiki sürüm de yedeğe geçer (tekrar çağrılınca yeni sürüme döner). `.swap` bir sonraki açılışta silinir.
pub fn rollback() -> Result<()> {
    let backup = backup_exe();
    if !backup.is_file() {
        bail!(crate::i18n::t("msg.no_previous_version"));
    }
    let exe = exe_path()?;
    let swap = exe.with_file_name("karadelikdev.exe.swap");
    let _ = fs::remove_file(&swap);
    let prev_version = previous_version().unwrap_or_default();
    fs::rename(&exe, &swap).with_context(|| format!("yeniden adlandırılamadı: {}", exe.display()))?;
    if let Err(e) = fs::copy(&backup, &exe) {
        let _ = fs::rename(&swap, &exe);
        return Err(anyhow::anyhow!(e).context(format!("kopyalanamadı: {}", exe.display())));
    }
    fs::copy(&swap, &backup)?;
    fs::write(backup_dir().join("version.txt"), current_version())?;
    log(&format!("Önceki KaradelikDev sürümüne dönüldü: {} -> {prev_version}", current_version()));
    Ok(())
}

/// Açılışta: geri dönüşten ve 1.1.0'ın exe değiştirme yönteminden kalan dosyaları siler.
pub fn cleanup() {
    if let Ok(exe) = exe_path() {
        for name in ["karadelikdev.exe.swap", "karadelikdev.exe.old"] {
            let _ = fs::remove_file(exe.with_file_name(name));
        }
    }
}

fn running_list() -> String {
    let running: Vec<&str> = Svc::START_ORDER.into_iter().filter(|s| services::is_running(*s) && services::owned_by_us(*s)).map(|s| s.id()).collect();
    if running.is_empty() { "-".to_string() } else { running.join(",") }
}

/// Servisleri durdurur, bu süreç kapandıktan sonra çalışacak gizli bir PowerShell bırakır ve çıkar (tek örnek kilidi
/// yüzünden yeni süreç eskisi açıkken başlatılamaz). PowerShell varsa önce setup'ı sessizce çalıştırır, sonra
/// kurulu exe'yi açar. `resume`: yeni süreç önceden çalışan servisleri başlatır (`--updated`).
fn relaunch(setup: Option<&Path>, resume: bool) -> Result<()> {
    let list = running_list();
    crate::tunnel::stop();
    services::stop_all();
    let exe = exe_path()?;
    let q = |s: &str| format!("'{}'", s.replace('\'', "''"));
    let mut script = format!("Wait-Process -Id {} -Timeout 30 -ErrorAction SilentlyContinue; ", std::process::id());
    if let Some(setup) = setup {
        // Kurulu sürümde /UPDATE: eskisini kaldırmadan üzerine kurar, kısayollara dokunmaz.
        // Taşınabilirden geçişte ilk kurulum olduğu için kısayollar oluşturulsun diye /UPDATE verilmez.
        let args = if paths().installed { "'/S','/UPDATE'" } else { "'/S'" };
        script.push_str(&format!("Start-Process -FilePath {} -ArgumentList {args} -Wait; ", q(&setup.display().to_string())));
    }
    script.push_str(&format!(
        "$d = (Get-ItemProperty -Path 'HKCU:\\Software\\KaradelikDev' -ErrorAction SilentlyContinue).InstallDir; \
         $e = if ($d) {{ Join-Path $d 'karadelikdev.exe' }} else {{ {cur} }}; \
         if (-not (Test-Path $e)) {{ $e = {cur} }}; ",
        cur = q(&exe.display().to_string())
    ));
    if resume {
        script.push_str(&format!("Start-Process -FilePath $e -ArgumentList '--updated',{}", q(&list)));
    } else {
        script.push_str("Start-Process -FilePath $e");
    }
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

/// İndirilmiş setup ile güncelle. Taşınabilir kurulumdan geçişte mevcut klasör veri klasörü olarak kaydedilir.
pub fn install_and_restart(setup: &Path) -> Result<()> {
    if !paths().installed {
        crate::paths::set_data_root(&paths().root)?;
    }
    log(&format!("KaradelikDev {} kuruluyor: {}", current_version(), setup.display()));
    relaunch(Some(setup), true)
}

/// Yalnız yeniden başlat (geri dönüş, veri klasörü değişikliği).
pub fn restart() -> Result<()> {
    relaunch(None, false)
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
}
