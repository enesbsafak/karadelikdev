//! Kök klasör izinleri (macOS). Klasör başka kullanıcıların yazabileceği biçimdeyse (grup/diğerleri yazma izni,
//! ör. `/Users/Shared` altına açılmış) bin/ altındaki bağlantılar ve ayarlar değiştirilerek KaradelikDev'in
//! çalıştırdığı kod ele geçirilebilir. Açılışta denetlenir; gerekirse yalnız sahibine yazma izni bırakılır.

use anyhow::{Context, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Grup ya da diğer kullanıcılar için yazma izni varsa ilgili izin adlarını döndürür.
pub fn broad_writers(path: &Path) -> Result<Vec<String>> {
    let mode = std::fs::metadata(path).with_context(|| path.display().to_string())?.permissions().mode();
    let mut out = Vec::new();
    if mode & 0o020 != 0 {
        out.push("group".to_string());
    }
    if mode & 0o002 != 0 {
        out.push("other".to_string());
    }
    Ok(out)
}

/// Grup/diğerleri yazma izinlerini kaldırır (okuma izni Apache gibi araçlar için kalır).
pub fn restrict(path: &Path) -> Result<()> {
    let mut perm = std::fs::metadata(path)?.permissions();
    perm.set_mode(perm.mode() & !0o022);
    std::fs::set_permissions(path, perm).with_context(|| path.display().to_string())
}

/// Açılışta: kök klasör başka kullanıcılara yazılabilir durumdaysa kısıtlar. Sonuç loga ve bildirime gider.
pub fn check_root() {
    let root = crate::kcore::paths().root.clone();
    let writers = match broad_writers(&root) {
        Ok(w) => w,
        Err(e) => {
            crate::kcore::log(&format!("Kök klasör izinleri okunamadı: {e:#}"));
            return;
        }
    };
    if writers.is_empty() {
        return;
    }
    crate::kcore::log(&format!("Kök klasör başka kullanıcılara yazılabilir ({}); izinler kısıtlanıyor", writers.join(", ")));
    match restrict(&root) {
        Ok(_) => crate::kcore::notify(&crate::i18n::t("msg.acl_restricted")),
        Err(e) => {
            crate::kcore::log(&format!("İzinler kısıtlanamadı: {e:#}"));
            let marker = crate::kcore::paths().usr().join(".acl-warned");
            if !marker.exists() {
                let _ = std::fs::write(&marker, "");
                crate::kcore::notify(&crate::i18n::t("msg.acl_failed"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restrict_group_write() {
        let dir = std::env::temp_dir().join(format!("kd-acl-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert_eq!(broad_writers(&dir).unwrap(), vec!["group", "other"]);
        restrict(&dir).unwrap();
        assert!(broad_writers(&dir).unwrap().is_empty());
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o755);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
