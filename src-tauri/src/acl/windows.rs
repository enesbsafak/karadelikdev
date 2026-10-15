//! Kök klasör izinleri. Zip ör. `C:\karadelik` gibi bir sürücü kökü altına açılırsa klasör, `C:\`'nin
//! "Authenticated Users: Değiştir" iznini devralır. O durumda bilgisayardaki başka bir kullanıcı bin\
//! altındaki exe/dll'leri değiştirerek KaradelikDev'in (ve yönetici olarak kurulan servislerin) çalıştırdığı
//! kodu ele geçirebilir, veritabanı dosyalarını okuyabilir. Açılışta denetlenir; gerekirse devralma kesilip
//! yalnız SYSTEM, Administrators ve bu kullanıcıya tam yetki bırakılır.

use crate::util::{hidden, run_capture};
use anyhow::{bail, Result};
use std::path::Path;

/// Herkes, Kimliği Doğrulanmış Kullanıcılar, Users, Interactive.
const BROAD_SIDS: &[&str] = &["S-1-1-0", "S-1-5-11", "S-1-5-32-545", "S-1-5-4"];
/// Yazma/silme/izin değiştirme sayılan haklar (FILE_WRITE_DATA, FILE_APPEND_DATA, FILE_DELETE_CHILD,
/// DELETE, WRITE_DAC, WRITE_OWNER, GENERIC_ALL, GENERIC_WRITE).
const WRITE_MASK: u32 = 0x2 | 0x4 | 0x40 | 0x1_0000 | 0x4_0000 | 0x8_0000 | 0x1000_0000 | 0x4000_0000;

/// Klasöre veya altına yazma izni olan geniş gruplar (SID olarak).
pub fn broad_writers(path: &Path) -> Result<Vec<String>> {
    use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS};
    use windows_sys::Win32::Security::Authorization::{ConvertSidToStringSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        AclSizeInformation, GetAce, GetAclInformation, ACCESS_ALLOWED_ACE, ACL, ACL_SIZE_INFORMATION, DACL_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR,
    };
    let wide: Vec<u16> = path.as_os_str().to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
    let mut out = Vec::new();
    unsafe {
        let mut dacl: *mut ACL = std::ptr::null_mut();
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        let rc = GetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut dacl,
            std::ptr::null_mut(),
            &mut sd,
        );
        if rc != ERROR_SUCCESS {
            bail!("GetNamedSecurityInfo: {rc}");
        }
        if dacl.is_null() {
            // DACL yok: herkes her şeyi yapabilir
            LocalFree(sd as _);
            return Ok(vec!["S-1-1-0".into()]);
        }
        let mut info: ACL_SIZE_INFORMATION = std::mem::zeroed();
        GetAclInformation(dacl, &mut info as *mut _ as _, std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32, AclSizeInformation);
        for i in 0..info.AceCount {
            let mut ace: *mut core::ffi::c_void = std::ptr::null_mut();
            if GetAce(dacl, i, &mut ace) == 0 {
                continue;
            }
            let a = &*(ace as *const ACCESS_ALLOWED_ACE);
            // 0 = ACCESS_ALLOWED_ACE_TYPE; izin veren ACE'ler, alt öğelere uygulananlar (inherit-only) dahil
            if a.Header.AceType != 0 || a.Mask & WRITE_MASK == 0 {
                continue;
            }
            let sid = &a.SidStart as *const u32 as *mut core::ffi::c_void;
            let mut s: *mut u16 = std::ptr::null_mut();
            if ConvertSidToStringSidW(sid, &mut s) != 0 && !s.is_null() {
                let len = (0..).take_while(|&k| *s.add(k) != 0).count();
                let text = String::from_utf16_lossy(std::slice::from_raw_parts(s, len));
                LocalFree(s as _);
                if BROAD_SIDS.contains(&text.as_str()) && !out.contains(&text) {
                    out.push(text);
                }
            }
        }
        LocalFree(sd as _);
    }
    Ok(out)
}

fn current_user_sid() -> Result<String> {
    let mut c = hidden("whoami");
    c.args(["/user", "/fo", "csv", "/nh"]);
    let (code, out) = run_capture(c)?;
    let sid = out.trim().rsplit(',').next().unwrap_or("").trim_matches('"').to_string();
    if code != 0 || !sid.starts_with("S-1-") {
        bail!("whoami: {}", out.trim());
    }
    Ok(sid)
}

/// Devralmayı keser; SYSTEM, Administrators ve bu kullanıcı tam yetkili kalır. Alt klasörler yeni izinleri devralır.
pub fn restrict(path: &Path) -> Result<()> {
    let me = current_user_sid()?;
    let mut c = hidden("icacls");
    c.arg(path).args(["/inheritance:r", "/grant:r", "*S-1-5-18:(OI)(CI)F", "*S-1-5-32-544:(OI)(CI)F"]);
    c.arg(format!("*{me}:(OI)(CI)F")).arg("/Q");
    let (code, out) = run_capture(c)?;
    if code != 0 {
        bail!("icacls: {}", out.trim());
    }
    Ok(())
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
            // Her açılışta tekrar bildirmesin
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

    /// Sürücü kökünde klasör açar; elle çalıştırılır: cargo test --lib acl -- --ignored
    #[test]
    #[ignore]
    fn restrict_drive_root_folder() {
        let dir = std::path::PathBuf::from(format!(r"C:\kd-acl-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        let before = broad_writers(&dir).unwrap();
        restrict(&dir).unwrap();
        let after = broad_writers(&dir).unwrap();
        let child = broad_writers(&dir.join("bin")).unwrap();
        std::fs::write(dir.join("bin").join("x.txt"), "ok").unwrap();
        crate::util::remove_dir_force(&dir).unwrap();
        assert!(!before.is_empty(), "sürücü kökündeki klasör geniş yazma izni devralmalı: {before:?}");
        assert!(after.is_empty(), "{after:?}");
        assert!(child.is_empty(), "{child:?}");
    }
}
