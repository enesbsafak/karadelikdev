//! "Windows servisi olarak çalıştır": servis desteği olan binary'ler (Apache, MySQL, PostgreSQL)
//! kendi kurulum parametreleriyle yönetici olarak kaydedilir.

use crate::services::Svc;
use crate::kcore::settings;
use crate::util::{hidden, run_capture, run_elevated};
use anyhow::{anyhow, bail, Result};
use std::path::Path;

pub fn supported(svc: Svc) -> bool {
    matches!(svc, Svc::Apache | Svc::Mysql | Svc::Postgresql)
}

pub fn name(svc: Svc) -> String {
    format!("Karadelik{}", svc.title().replace(' ', ""))
}

pub fn installed(svc: Svc) -> bool {
    if !supported(svc) {
        return false;
    }
    let mut c = hidden("sc");
    c.args(["query", &name(svc)]);
    run_capture(c).map(|(code, _)| code == 0).unwrap_or(false)
}

fn elevated_cmd(line: &str) -> Result<()> {
    run_elevated(Path::new("cmd.exe"), &format!("/c {line}"), true)
}

pub fn install(svc: Svc) -> Result<()> {
    let n = name(svc);
    let line = match svc {
        Svc::Apache => {
            crate::services::apache::write_conf()?;
            let exe = crate::services::apache::exe().ok_or_else(|| anyhow!("httpd.exe"))?;
            format!("\"\"{}\" -k install -n {n} -f \"{}\"\"", exe.display(), crate::services::apache::conf_path().display())
        }
        Svc::Mysql => {
            let ini = crate::services::mysql::write_ini()?;
            crate::services::mysql::init_data_dir()?;
            let exe = crate::services::mysql::mysqld().ok_or_else(|| anyhow!("mysqld.exe"))?;
            format!("\"\"{}\" --install {n} --defaults-file=\"{}\"\"", exe.display(), ini.display())
        }
        Svc::Postgresql => {
            let bin = crate::services::others::pg_bin().ok_or_else(|| anyhow!("pg_ctl.exe"))?;
            let data = crate::services::others::pg_data().unwrap();
            let port = settings().svc("postgresql").port;
            format!(
                "\"\"{}\" register -N {n} -D \"{}\" -o \"-p {port}\"\"",
                bin.join("pg_ctl.exe").display(),
                data.display()
            )
        }
        _ => bail!("desteklenmiyor"),
    };
    elevated_cmd(&line)
}

pub fn uninstall(svc: Svc) -> Result<()> {
    let n = name(svc);
    elevated_cmd(&format!("sc stop {n} & sc delete {n}"))
}

pub fn start(svc: Svc) -> Result<()> {
    elevated_cmd(&format!("sc start {}", name(svc)))
}

pub fn stop(svc: Svc) -> Result<()> {
    elevated_cmd(&format!("sc stop {}", name(svc)))
}
