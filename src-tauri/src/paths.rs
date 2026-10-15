//! KaradelikDev kök dizini ve alt dizinleri (bin, etc, data, www, usr, tmp, logs, backup).

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    pub root: PathBuf,
    /// Setup ile kurulmuş (exe ve veriler ayrı klasörlerde).
    pub installed: bool,
    /// Kök klasör bu açılışta oluşturuldu (sihirbazda başka klasör seçilirse boş kalan bu klasör silinir).
    pub created: bool,
}

/// Setup'ın yazdığı ve uygulamanın okuduğu kayıt anahtarı (HKCU).
pub const REG_KEY: &str = r"Software\KaradelikDev";
/// Kurulumlu sürümde veri klasörünün varsayılanı: kısa ve boşluksuz (Apache/MySQL boşluklu yollarla sorun çıkarır).
pub const DEFAULT_DATA_ROOT: &str = r"C:\karadelik";

fn reg_get(name: &str) -> Option<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    winreg::RegKey::predef(HKEY_CURRENT_USER).open_subkey(REG_KEY).ok()?.get_value::<String, _>(name).ok().filter(|v| !v.trim().is_empty())
}

/// Setup'ın kurduğu klasör (NSIS kancası yazar).
pub fn install_dir() -> Option<PathBuf> {
    reg_get("InstallDir").map(PathBuf::from)
}

/// Kurulumlu sürümün veri klasörünü kaydeder (bir sonraki açılışta geçerli olur).
pub fn set_data_root(path: &Path) -> anyhow::Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    let (key, _) = winreg::RegKey::predef(HKEY_CURRENT_USER).create_subkey(REG_KEY)?;
    key.set_value("DataRoot", &path.display().to_string())?;
    Ok(())
}

fn same_dir(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase();
    norm(a) == norm(b)
}

impl Paths {
    /// Sıra: KARADELIK_ROOT ortam değişkeni; setup ile kurulmuşsa kayıtlı veri klasörü (varsayılan C:\karadelik);
    /// geliştirme derlemesinde `<repo>/runtime`; aksi halde exe'nin bulunduğu dizin (taşınabilir).
    pub fn detect() -> Paths {
        let make = |root: PathBuf, installed: bool| {
            let created = !root.exists();
            Paths { root, installed, created }
        };
        if let Ok(r) = std::env::var("KARADELIK_ROOT") {
            if !r.trim().is_empty() {
                return make(PathBuf::from(r.trim()), false);
            }
        }
        let exe = std::env::current_exe().map(strip_unc).unwrap_or_default();
        let exe_dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
        if install_dir().map(|d| same_dir(&d, &exe_dir)).unwrap_or(false) {
            let root = reg_get("DataRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_ROOT));
            return make(root, true);
        }
        if cfg!(debug_assertions) {
            let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("runtime");
            let _ = std::fs::create_dir_all(&dev);
            if let Ok(c) = dev.canonicalize() {
                return make(strip_unc(c), false);
            }
        }
        make(exe_dir, false)
    }

    pub fn bin(&self) -> PathBuf { self.root.join("bin") }
    pub fn etc(&self) -> PathBuf { self.root.join("etc") }
    pub fn usr(&self) -> PathBuf { self.root.join("usr") }
    pub fn tmp(&self) -> PathBuf { self.root.join("tmp") }
    pub fn logs(&self) -> PathBuf { self.root.join("logs") }
    pub fn backup(&self) -> PathBuf { self.root.join("backup") }
    pub fn ssl(&self) -> PathBuf { self.etc().join("ssl") }
    pub fn apps(&self) -> PathBuf { self.etc().join("apps") }
    pub fn profiles(&self) -> PathBuf { self.usr().join("profile") }
    pub fn ini(&self) -> PathBuf { self.usr().join("karadelik.ini") }
    pub fn log_file(&self) -> PathBuf { self.logs().join("karadelik.log") }
    pub fn default_www(&self) -> PathBuf { self.root.join("www") }
    pub fn default_data(&self) -> PathBuf { self.root.join("data") }

    pub fn ensure_layout(&self) {
        for d in [
            self.bin(),
            self.etc().join("apache2").join("sites-enabled"),
            self.etc().join("apache2").join("alias"),
            self.etc().join("nginx").join("sites-enabled"),
            self.etc().join("nginx").join("alias"),
            self.etc().join("php"),
            self.etc().join("mysql"),
            self.ssl(),
            self.apps(),
            self.usr().join("tpl"),
            self.usr().join("bin"),
            self.profiles(),
            self.tmp().join("downloads"),
            self.tmp().join("nginx").join("logs"),
            self.tmp().join("nginx").join("temp"),
            self.tmp().join("sessions"),
            self.logs(),
            self.backup().join("mysql"),
        ] {
            let _ = std::fs::create_dir_all(d);
        }
    }
}

/// `\\?\C:\...` önekini kaldırır; Apache/MySQL bu biçimi anlamaz.
pub fn strip_unc(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        p
    }
}

/// Konfigürasyon dosyaları için ileri eğik çizgili yol.
pub fn fwd(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_string()
}
