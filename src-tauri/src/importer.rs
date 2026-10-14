//! Laragon'dan içe aktarma: Laragon'un bin ve etc\apps klasörleri dizin bağlantısıyla (junction)
//! eklenir, gigabaytlarca kopya yapılmaz. KaradelikDev bin içine yazmadığı için paylaşım güvenlidir.

use crate::kcore::paths;
use crate::util::{hidden, run_capture};
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

pub fn find_laragon() -> Option<PathBuf> {
    ["C:\\laragon", "D:\\laragon", "E:\\laragon"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.join("bin").is_dir() && p.join("laragon.exe").exists())
}

fn junction(link: &Path, target: &Path) -> Result<()> {
    if let Some(p) = link.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut c = hidden("cmd.exe");
    c.arg("/c").arg("mklink").arg("/J").arg(link).arg(target);
    let (code, out) = run_capture(c)?;
    if code != 0 {
        bail!("mklink: {}", out.trim());
    }
    Ok(())
}

/// Bağlanan öğelerin listesini döndürür.
pub fn import(laragon: &Path) -> Result<Vec<String>> {
    let p = paths();
    let mut linked = Vec::new();
    let src_bin = laragon.join("bin");
    for e in std::fs::read_dir(&src_bin)?.flatten() {
        let cat = e.file_name().to_string_lossy().to_string();
        if !e.path().is_dir() || cat.eq_ignore_ascii_case("laragon") {
            continue;
        }
        if crate::versions::is_versioned(&cat) {
            for v in std::fs::read_dir(e.path())?.flatten() {
                if !v.path().is_dir() {
                    continue;
                }
                let link = p.bin().join(&cat).join(v.file_name());
                if !link.exists() {
                    junction(&link, &v.path())?;
                    linked.push(format!("bin\\{cat}\\{}", v.file_name().to_string_lossy()));
                }
            }
        } else {
            let link = p.bin().join(&cat);
            if !link.exists() {
                junction(&link, &e.path())?;
                linked.push(format!("bin\\{cat}"));
            }
        }
    }
    let src_apps = laragon.join("etc").join("apps");
    if let Ok(rd) = std::fs::read_dir(&src_apps) {
        for e in rd.flatten() {
            let link = p.apps().join(e.file_name());
            if e.path().is_dir() && !link.exists() {
                junction(&link, &e.path())?;
                linked.push(format!("etc\\apps\\{}", e.file_name().to_string_lossy()));
            }
        }
    }
    // Laragon'un cacert.pem'i varsa kopyala (curl/openssl için)
    let cacert = laragon.join("etc").join("ssl").join("cacert.pem");
    if cacert.exists() && !p.ssl().join("cacert.pem").exists() {
        let _ = std::fs::copy(cacert, p.ssl().join("cacert.pem"));
    }
    Ok(linked)
}
