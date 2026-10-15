//! "Servis olarak çalıştır" (macOS): `~/Library/LaunchAgents/dev.karadelik.<servis>.plist` kullanıcı ajanı.
//! Oturum açılınca launchd servisi KaradelikDev açık olmasa da başlatır; yönetici izni gerekmez.
//! Durdurmak ajanı bu oturum için kaldırır (`launchctl bootout`), plist kaldığı için sonraki oturumda yine başlar.

use crate::services::Svc;
use crate::kcore::{paths, settings};
use crate::util::{hidden, run_capture};
use anyhow::{anyhow, bail, Result};
use std::path::PathBuf;

pub fn supported(svc: Svc) -> bool {
    matches!(svc, Svc::Apache | Svc::Mysql | Svc::Postgresql)
}

pub fn label(svc: Svc) -> String {
    format!("dev.karadelik.{}", svc.id())
}

/// Windows'la aynı arayüz.
#[allow(dead_code)]
pub fn name(svc: Svc) -> String {
    label(svc)
}

fn plist_path(svc: Svc) -> PathBuf {
    crate::platform::home().join("Library").join("LaunchAgents").join(format!("{}.plist", label(svc)))
}

pub fn installed(svc: Svc) -> bool {
    supported(svc) && plist_path(svc).is_file()
}

fn domain() -> String {
    let uid = run_capture({
        let mut c = hidden("/usr/bin/id");
        c.arg("-u");
        c
    })
    .map(|(_, o)| o.trim().to_string())
    .unwrap_or_else(|_| "501".into());
    format!("gui/{uid}")
}

fn launchctl(args: &[&str]) -> Result<(i32, String)> {
    let mut c = hidden("/bin/launchctl");
    c.args(args);
    run_capture(c)
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn plist(svc: Svc, program: &[String], log: &std::path::Path) -> String {
    let args: String = program.iter().map(|a| format!("        <string>{}</string>\n", xml(a))).collect();
    let path = crate::services::service_env(&[]).into_iter().find(|(k, _)| k == "PATH").map(|(_, v)| v).unwrap_or_default();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
{args}    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>{path}</string>
    </dict>
    <key>WorkingDirectory</key>
    <string>{cwd}</string>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{log}</string>
    <key>StandardErrorPath</key>
    <string>{log}</string>
</dict>
</plist>
"#,
        label = label(svc),
        path = xml(&path),
        cwd = xml(&paths().tmp().display().to_string()),
        log = xml(&log.display().to_string()),
    )
}

pub fn install(svc: Svc) -> Result<()> {
    let logs = paths().logs();
    let (program, log): (Vec<String>, PathBuf) = match svc {
        Svc::Apache => {
            crate::services::apache::write_conf()?;
            crate::services::apache::test_conf()?;
            let exe = crate::services::apache::exe().ok_or_else(|| anyhow!("httpd"))?;
            (
                vec![exe.display().to_string(), "-f".into(), crate::services::apache::conf_path().display().to_string(), "-DFOREGROUND".into()],
                logs.join("apache_stdout.log"),
            )
        }
        Svc::Mysql => {
            let ini = crate::services::mysql::write_ini()?;
            crate::services::mysql::init_data_dir()?;
            let exe = crate::services::mysql::mysqld().ok_or_else(|| anyhow!("mysqld"))?;
            (vec![exe.display().to_string(), format!("--defaults-file={}", ini.display())], logs.join("mysqld_stdout.log"))
        }
        Svc::Postgresql => {
            let bin = crate::services::others::pg_bin().ok_or_else(|| anyhow!("postgres"))?;
            let data = crate::services::others::pg_data().ok_or_else(|| anyhow!("postgres"))?;
            if !data.join("PG_VERSION").exists() {
                bail!(crate::i18n::tf("msg.start_first", &["PostgreSQL"]));
            }
            let port = settings().svc("postgresql").port;
            (
                vec![
                    bin.join("postgres").display().to_string(),
                    "-D".into(),
                    data.display().to_string(),
                    "-p".into(),
                    port.to_string(),
                    "-k".into(),
                    crate::services::others::pg_socket_dir().display().to_string(),
                ],
                crate::services::others::pg_log(),
            )
        }
        _ => bail!("desteklenmiyor"),
    };
    let file = plist_path(svc);
    std::fs::create_dir_all(file.parent().unwrap())?;
    std::fs::write(&file, plist(svc, &program, &log))?;
    start(svc)
}

pub fn uninstall(svc: Svc) -> Result<()> {
    let _ = stop(svc);
    let _ = std::fs::remove_file(plist_path(svc));
    Ok(())
}

pub fn start(svc: Svc) -> Result<()> {
    let file = plist_path(svc).display().to_string();
    let (code, out) = launchctl(&["bootstrap", &domain(), &file])?;
    // Zaten yüklüyse (ör. oturum açılışında başladı ve kapandı) yeniden çalıştır
    if code != 0 {
        let (code2, out2) = launchctl(&["kickstart", &format!("{}/{}", domain(), label(svc))])?;
        if code2 != 0 {
            bail!("launchctl: {} {}", out.trim(), out2.trim());
        }
    }
    let port = settings().svc(svc.id()).port;
    if !crate::util::wait_port(port, true, std::time::Duration::from_secs(30)) {
        bail!(crate::i18n::tf("msg.start_failed", &[svc.title()]));
    }
    Ok(())
}

pub fn stop(svc: Svc) -> Result<()> {
    let (code, out) = launchctl(&["bootout", &format!("{}/{}", domain(), label(svc))])?;
    // 3/113: yüklü değil (zaten durmuş)
    if code != 0 && !out.contains("No such process") && !out.contains("Could not find") && code != 3 && code != 113 {
        bail!("launchctl bootout: {}", out.trim());
    }
    let port = settings().svc(svc.id()).port;
    crate::util::wait_port(port, false, std::time::Duration::from_secs(15));
    Ok(())
}
