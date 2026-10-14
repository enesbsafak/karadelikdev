//! KaradelikDev kök dizini ve alt dizinleri (bin, etc, data, www, usr, tmp, logs, backup).

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    /// Sıra: KARADELIK_ROOT ortam değişkeni, geliştirme derlemesinde `<repo>/runtime`,
    /// aksi halde exe'nin bulunduğu dizin (taşınabilir kurulum).
    pub fn detect() -> Paths {
        if let Ok(r) = std::env::var("KARADELIK_ROOT") {
            if !r.trim().is_empty() {
                return Paths { root: PathBuf::from(r.trim()) };
            }
        }
        if cfg!(debug_assertions) {
            let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("runtime");
            let _ = std::fs::create_dir_all(&dev);
            if let Ok(c) = dev.canonicalize() {
                return Paths { root: strip_unc(c) };
            }
        }
        let exe = std::env::current_exe().unwrap_or_default();
        Paths { root: exe.parent().map(Path::to_path_buf).unwrap_or_default() }
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
