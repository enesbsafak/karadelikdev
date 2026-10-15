//! hosts dosyası. KaradelikDev girdileri işaretli bir blok içinde tutulur; yazma izni yoksa geçici dosya yazılır ve
//! yönetici olarak kopyalanır (Windows: yükseltilmiş `--apply-hosts`, UAC; macOS: parola penceresi, ardından DNS önbelleği temizlenir).

use crate::kcore::paths;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

const BEGIN: &str = "# >>> KaradelikDev (otomatik, elle düzenlemeyin)";
const END: &str = "# <<< KaradelikDev";

pub fn hosts_path() -> PathBuf {
    crate::platform::hosts_path()
}

pub fn build(current: &str, hosts: &[String]) -> String {
    let nl = crate::platform::LINE_END;
    let mut out = String::new();
    let mut inside = false;
    for line in current.lines() {
        if line.trim() == BEGIN {
            inside = true;
            continue;
        }
        if line.trim() == END {
            inside = false;
            continue;
        }
        if !inside {
            out.push_str(line);
            out.push_str(nl);
        }
    }
    let trimmed = out.trim_end().to_string();
    let mut out = trimmed;
    if !hosts.is_empty() {
        out.push_str(nl);
        out.push_str(nl);
        out.push_str(BEGIN);
        out.push_str(nl);
        for h in hosts {
            out.push_str(&format!("127.0.0.1      {h}{nl}"));
        }
        out.push_str(END);
    }
    out.push_str(nl);
    out
}

/// Blok içeriği değiştiyse yazar. Gerekirse UAC ister. Değişiklik yapıldıysa true.
pub fn sync(hosts: &[String]) -> Result<bool> {
    let path = hosts_path();
    let current = fs::read_to_string(&path).unwrap_or_default();
    let wanted = build(&current, hosts);
    if normalize(&wanted) == normalize(&current) {
        return Ok(false);
    }
    match fs::write(&path, &wanted) {
        Ok(_) => Ok(true),
        Err(_) => {
            let tmp = paths().tmp().join("hosts.tmp");
            fs::write(&tmp, &wanted)?;
            apply_elevated(&tmp).context(crate::i18n::t("msg.hosts_denied"))?;
            Ok(true)
        }
    }
}

#[cfg(windows)]
fn apply_elevated(tmp: &Path) -> Result<()> {
    let exe = std::env::current_exe()?;
    crate::util::run_elevated(&exe, &format!("--apply-hosts \"{}\"", tmp.display()), true)
}

/// macOS: `cat` hosts dosyasının sahibini ve izinlerini korur; ardından DNS önbelleği temizlenir.
#[cfg(not(windows))]
fn apply_elevated(tmp: &Path) -> Result<()> {
    let q = |p: &Path| crate::platform::sh_quote(&p.display().to_string());
    let script = format!(
        "/bin/cat {} > {} && /usr/bin/dscacheutil -flushcache; /usr/bin/killall -HUP mDNSResponder 2>/dev/null; true",
        q(tmp),
        q(&hosts_path())
    );
    crate::platform::run_elevated_shell(&script, &crate::i18n::t("msg.hosts_prompt"))
}

fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n").trim_end().to_string()
}

/// Yükseltilmiş süreçte çalışır: geçici dosyayı hosts üzerine kopyalar.
pub fn apply_from(tmp: &Path) -> Result<()> {
    let content = fs::read_to_string(tmp)?;
    let path = hosts_path();
    // Salt okunur bayrağını kaldır (Laragon'un uyarı verdiği durum)
    if let Ok(md) = fs::metadata(&path) {
        let mut p = md.permissions();
        if p.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            p.set_readonly(false);
            let _ = fs::set_permissions(&path, p);
        }
    }
    fs::write(&path, content)?;
    Ok(())
}

/// macOS: hosts dosyası Terminal'de `sudo nano` ile açılır (kaydedince DNS önbelleği temizlenir).
#[cfg(not(windows))]
pub fn edit_as_admin() -> Result<()> {
    let path = crate::platform::sh_quote(&hosts_path().display().to_string());
    let cmd = format!("sudo ${{EDITOR:-nano}} {path} && sudo /usr/bin/dscacheutil -flushcache && sudo /usr/bin/killall -HUP mDNSResponder; exit");
    crate::platform::terminal_script("hosts", &paths().root, None, &cmd, false, None)
}

/// Hosts dosyasını yönetici olarak Not Defteri'nde açar.
#[cfg(windows)]
pub fn edit_as_admin() -> Result<()> {
    crate::util::run_elevated(Path::new("notepad.exe"), &format!("\"{}\"", hosts_path().display()), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_roundtrip() {
        let base = &format!("127.0.0.1 localhost{}", crate::platform::LINE_END);
        let a = build(base, &["a.test".into(), "b.test".into()]);
        assert!(a.contains("127.0.0.1      a.test"));
        let b = build(&a, &["c.test".into()]);
        assert!(!b.contains("a.test"));
        assert!(b.contains("c.test"));
        assert!(b.starts_with("127.0.0.1 localhost"));
        let c = build(&b, &[]);
        assert!(!c.contains("KaradelikDev"));
    }
}
