//! Arka plan döngüsü:
//! durum yayını, yeni proje algılama, php-cgi izleme, otomatik yedek, otomatik güncelleme, cron.

use crate::kcore::{log, notify, paths, settings, update_settings};
use crate::i18n::tf;
use crate::services::{self, Svc};
use crate::util;
use anyhow::{anyhow, Result};
use chrono::{Datelike, Local, Timelike};
use std::time::Duration;

pub fn start() {
    std::thread::spawn(|| {
        let mut tick: u64 = 0;
        let mut last_running: Vec<bool> = vec![];
        let mut last_projects: Vec<String> = crate::vhosts::projects().into_iter().map(|p| p.name).collect();
        let mut last_cron_minute = String::new();
        loop {
            std::thread::sleep(Duration::from_secs(1));
            tick += 1;
            if tick % 3 == 0 {
                let st = services::statuses();
                let running: Vec<bool> = st.iter().map(|s| s.running).collect();
                crate::kcore::emit("services", st.clone());
                if running != last_running {
                    crate::menu::refresh(&st);
                    last_running = running;
                }
            }
            if tick % 5 == 0 {
                services::phpcgi::monitor();
                // Document Root'a yeni klasör eklendi/silindi mi?
                let now: Vec<String> = crate::vhosts::projects().into_iter().map(|p| p.name).collect();
                if now != last_projects {
                    last_projects = now;
                    if settings().auto_vhosts {
                        match crate::vhosts::sync_and_reload() {
                            Ok(rep) if !rep.created.is_empty() => {
                                notify(&tf("msg.new_project", &[&rep.created.join(", ")]));
                            }
                            Ok(_) => {}
                            Err(e) => crate::kcore::error("vhost", &e),
                        }
                        services::broadcast();
                    }
                }
            }
            let minute = Local::now().format("%Y%m%d%H%M").to_string();
            if minute != last_cron_minute {
                last_cron_minute = minute;
                run_cron();
            }
            if tick % 60 == 0 {
                auto_backup();
                auto_update();
            }
            if tick % 86_400 == 0 && settings().app_update_check {
                std::thread::spawn(crate::updater::background_check);
            }
        }
    });
}

fn auto_backup() {
    let s = settings();
    if !s.auto_backup || !services::is_running(Svc::Mysql) {
        return;
    }
    let now = Local::now().timestamp();
    if now - s.last_backup < (s.backup_interval.max(1) as i64) * 3600 {
        return;
    }
    match crate::mysqlops::backup_all() {
        Ok(file) => {
            crate::mysqlops::prune_backups(s.backup_keep.max(1) as usize);
            update_settings(|x| x.last_backup = now);
            log(&format!("Otomatik yedek: {}", file.display()));
        }
        Err(e) => log(&format!("Otomatik yedek başarısız: {e:#}")),
    }
}

fn auto_update() {
    let s = settings();
    let now = Local::now().timestamp();
    if !s.auto_update || now - s.last_update_check < 24 * 3600 {
        return;
    }
    update_settings(|x| x.last_update_check = now);
    std::thread::spawn(|| {
        let _ = update_cacert(false);
        match check_php_updates(true) {
            Ok(list) => {
                for m in list {
                    notify(&m);
                }
            }
            Err(e) => log(&format!("Auto Update: {e:#}")),
        }
    });
}

/// curl/openssl için CA paketi (etc\ssl\cacert.pem), ayda bir yenilenir.
pub fn update_cacert(force: bool) -> Result<()> {
    let file = paths().ssl().join("cacert.pem");
    let old = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .map(|t| t.elapsed().unwrap_or_default() > Duration::from_secs(30 * 86400))
        .unwrap_or(true);
    if force || old {
        crate::packages::download("https://curl.se/ca/cacert.pem", &file, "cacert.pem")?;
        crate::packages::progress_done();
    }
    Ok(())
}

/// macOS: `bin/php` altındaki sürümler Homebrew formüllerine bağlıdır; yama sürümleri `brew upgrade` ile gelir
/// (klasör adı x.y olduğu için profil ve php.ini aynı kalır). `install` false ise yalnız listeler.
#[cfg(not(windows))]
pub fn check_php_updates(install: bool) -> Result<Vec<String>> {
    let installed = crate::versions::installed("php");
    if installed.is_empty() {
        return Err(anyhow!(tf("msg.not_installed", &["PHP"])));
    }
    let brew = crate::platform::brew().ok_or_else(|| anyhow!(crate::i18n::t("setup.brew_missing")))?;
    let formulas: Vec<String> = installed
        .iter()
        .filter_map(|v| std::fs::read_link(paths().bin().join("php").join(v)).ok())
        .filter_map(|t| t.file_name().map(|f| f.to_string_lossy().to_string()))
        .filter(|f| f == "php" || f.starts_with("php@"))
        .collect();
    if formulas.is_empty() {
        return Ok(vec![]);
    }
    let mut c = util::hidden(&brew);
    c.args(["outdated", "--json=v2"]).args(&formulas).env("HOMEBREW_NO_AUTO_UPDATE", "1");
    let out = c.output()?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
    let mut list = Vec::new();
    for f in v["formulae"].as_array().cloned().unwrap_or_default() {
        let name = f["name"].as_str().unwrap_or_default().to_string();
        let latest = f["current_version"].as_str().unwrap_or_default().to_string();
        if !install {
            list.push(tf("msg.update_available", &["PHP", &latest]));
            continue;
        }
        crate::kcore::status(&format!("brew upgrade {name}"));
        let mut u = util::hidden(&brew);
        u.args(["upgrade", &name]).env("HOMEBREW_NO_ENV_HINTS", "1").env("NONINTERACTIVE", "1");
        let (code, text) = util::run_capture(u)?;
        util::append_line(&paths().logs().join("brew.log"), &text);
        if code != 0 {
            return Err(anyhow!("brew upgrade {name}: {}", text.lines().rev().take(4).collect::<Vec<_>>().join("\n")));
        }
        log(&format!("PHP {latest} Homebrew ile güncellendi ({name})"));
        list.push(tf("msg.update_added", &["PHP", &latest]));
    }
    Ok(list)
}

/// Kurulu her PHP alt sürümü (8.3, 8.4...) için windows.php.net'te daha yenisi var mı?
/// `install` true ise indirip bin\php altına ekler (profil değiştirilmez, Laragon ile aynı).
#[cfg(windows)]
pub fn check_php_updates(install: bool) -> Result<Vec<String>> {
    let body = crate::packages::fetch_text("https://windows.php.net/downloads/releases/releases.json")?;
    let rel: serde_json::Value = serde_json::from_str(&body)?;
    let re = regex::Regex::new(r"(?i)^php-(\d+)\.(\d+)\.(\d+)(-nts)?-Win32-(v[cs]\d+)-x64$").unwrap();
    let installed = crate::versions::installed("php");
    let mut out = Vec::new();
    for folder in &installed {
        let Some(c) = re.captures(folder) else { continue };
        let minor = format!("{}.{}", &c[1], &c[2]);
        let patch: u32 = c[3].parse().unwrap_or(0);
        let nts = c.get(4).is_some();
        let vs = c[5].to_lowercase();
        let Some(entry) = rel.get(&minor) else { continue };
        let latest = entry["version"].as_str().unwrap_or("");
        let latest_patch: u32 = latest.rsplit('.').next().and_then(|p| p.parse().ok()).unwrap_or(0);
        if latest_patch <= patch {
            continue;
        }
        let key = format!("{}-{vs}-x64", if nts { "nts" } else { "ts" });
        let Some(zip) = entry[&key]["zip"]["path"].as_str() else { continue };
        let name = zip.trim_end_matches(".zip").to_string();
        if installed.iter().any(|i| i.eq_ignore_ascii_case(&name)) {
            continue;
        }
        if !install {
            out.push(tf("msg.update_available", &["PHP", latest]));
            continue;
        }
        let url = format!("https://windows.php.net/downloads/releases/{zip}");
        let dest = paths().bin().join("php").join(&name);
        crate::packages::fetch_into(&url, &dest, &name, None, entry[&key]["zip"]["sha256"].as_str())?;
        log(&format!("PHP {latest} indirildi ve SHA-256 doğrulandı"));
        out.push(tf("msg.update_added", &["PHP", latest]));
    }
    if installed.is_empty() {
        return Err(anyhow!(tf("msg.not_installed", &["PHP"])));
    }
    Ok(out)
}

// ---------------------------------------------------------------- cron

fn field_matches(field: &str, value: u32, min: u32, max: u32, names: &[&str]) -> bool {
    let to_num = |s: &str| -> Option<u32> {
        let l = s.to_lowercase();
        if let Some(i) = names.iter().position(|n| *n == l) {
            return Some(i as u32 + if min == 1 && names.len() == 12 { 1 } else { 0 });
        }
        s.parse().ok()
    };
    field.split(',').any(|part| {
        let (range, step) = match part.split_once('/') {
            Some((r, s)) => (r, s.parse::<u32>().unwrap_or(1).max(1)),
            None => (part, 1),
        };
        let (lo, hi) = if range == "*" {
            (min, max)
        } else if let Some((a, b)) = range.split_once('-') {
            match (to_num(a), to_num(b)) {
                (Some(a), Some(b)) => (a, b),
                _ => return false,
            }
        } else {
            match to_num(range) {
                Some(v) if part.contains('/') => (v, max),
                Some(v) => (v, v),
                None => return false,
            }
        };
        // Pazar hem 0 hem 7 olabilir
        let value_alt = if names.len() == 7 && value == 0 { Some(7) } else { None };
        [Some(value), value_alt].into_iter().flatten().any(|v| v >= lo && v <= hi && (v - lo) % step == 0)
    })
}

pub fn cron_matches(expr: &str, t: &chrono::DateTime<Local>) -> bool {
    const MONTHS: &[&str] = &["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    const DAYS: &[&str] = &["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
    let f: Vec<&str> = expr.split_whitespace().collect();
    if f.len() != 5 {
        return false;
    }
    field_matches(f[0], t.minute(), 0, 59, &[])
        && field_matches(f[1], t.hour(), 0, 23, &[])
        && field_matches(f[2], t.day(), 1, 31, &[])
        && field_matches(f[3], t.month(), 1, 12, MONTHS)
        && field_matches(f[4], t.weekday().num_days_from_sunday(), 0, 7, DAYS)
}

fn run_cron() {
    let Ok(text) = std::fs::read_to_string(paths().usr().join("crontab")) else { return };
    let now = Local::now();
    let jobs: Vec<(String, String)> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let parts: Vec<&str> = l.splitn(6, char::is_whitespace).filter(|p| !p.is_empty()).collect();
            (parts.len() == 6).then(|| (parts[..5].join(" "), parts[5].to_string()))
        })
        .collect();
    for (expr, cmd) in jobs {
        if !cron_matches(&expr, &now) {
            continue;
        }
        let env_cmd = match crate::terminal::write_env_cmd() {
            Ok(f) => f,
            Err(_) => continue,
        };
        let logf = paths().logs().join("cron.log");
        util::append_line(&logf, &format!("[{}] {cmd}", now.format("%Y-%m-%d %H:%M")));
        // Ortam dosyası: Windows'ta `call kdenv.cmd`, macOS'ta `. kdenv.sh`
        let load = if cfg!(windows) { format!("call \"{}\"", env_cmd.display()) } else { format!(". \"{}\"", env_cmd.display()) };
        let mut c = crate::platform::shell(&format!("{load} && {cmd} >> \"{}\" 2>&1", logf.display()));
        c.current_dir(&paths().root);
        if let Err(e) = c.spawn() {
            log(&format!("cron: {e}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn cron() {
        // 2026-10-14 Çarşamba 17:45
        let t = Local.with_ymd_and_hms(2026, 10, 14, 17, 45, 0).unwrap();
        assert!(cron_matches("* * * * *", &t));
        assert!(cron_matches("45 17 * * *", &t));
        assert!(cron_matches("*/15 * * * *", &t));
        assert!(!cron_matches("*/20 * * * *", &t));
        assert!(cron_matches("0-50/5 17 14 oct wed", &t));
        assert!(cron_matches("* * * * 1-5", &t));
        assert!(!cron_matches("* * * * 0,6", &t));
        assert!(!cron_matches("0 12 * * *", &t));
    }
}
