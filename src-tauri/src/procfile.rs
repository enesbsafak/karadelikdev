//! usr\Procfile: `Ad: [autorun] KOMUTLAR [env_file=..] [pwd=..] [DEGISKEN=deger]...`
//! Her girdi KaradelikDev ortamı yüklenmiş ayrı bir konsolda çalışır.

use crate::kcore::{log, paths};
use crate::util::{self, split_args};
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Child;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Proc {
    pub name: String,
    pub autorun: bool,
    pub command: String,
    pub pwd: Option<String>,
    pub env_file: Option<String>,
    pub env: Vec<(String, String)>,
}

static RUNNING: once_cell::sync::Lazy<Mutex<HashMap<String, Child>>> = once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

pub fn path() -> PathBuf {
    paths().usr().join("Procfile")
}

pub fn parse(text: &str) -> Vec<Proc> {
    let mut out = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with(';') || l.starts_with("---") {
            continue;
        }
        let Some((name, rest)) = l.split_once(':') else { continue };
        // "C:\..." gibi sürücü harfi ad sanılmasın
        if name.trim().len() <= 1 {
            continue;
        }
        let mut p = Proc { name: name.trim().to_string(), autorun: false, command: String::new(), pwd: None, env_file: None, env: vec![] };
        let mut cmd = Vec::new();
        for (i, tok) in split_args(rest).into_iter().enumerate() {
            if i == 0 && tok.eq_ignore_ascii_case("autorun") {
                p.autorun = true;
                continue;
            }
            if let Some((k, v)) = tok.split_once('=') {
                if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    match k.to_lowercase().as_str() {
                        "pwd" => p.pwd = Some(v.to_string()),
                        "env_file" => p.env_file = Some(v.to_string()),
                        _ => p.env.push((k.to_string(), v.to_string())),
                    }
                    continue;
                }
            }
            cmd.push(if tok.contains(' ') { format!("\"{tok}\"") } else { tok });
        }
        p.command = cmd.join(" ");
        if !p.command.is_empty() {
            out.push(p);
        }
    }
    out
}

pub fn list() -> Vec<Proc> {
    parse(&std::fs::read_to_string(path()).unwrap_or_default())
}

fn resolve(p: &str) -> PathBuf {
    let pb = PathBuf::from(p.replace('/', "\\"));
    if pb.is_absolute() { pb } else { paths().root.join(pb) }
}

pub fn run(name: &str) -> Result<()> {
    let p = list().into_iter().find(|x| x.name == name).ok_or_else(|| anyhow!("{name}?"))?;
    let env_cmd = crate::terminal::write_env_cmd()?;
    let cwd = p.pwd.as_deref().map(resolve).unwrap_or_else(|| paths().root.clone());
    let mut chain = vec![format!("call \"{}\"", env_cmd.display())];
    if let Some(f) = &p.env_file {
        if let Ok(text) = std::fs::read_to_string(resolve(f)) {
            for l in text.lines().map(str::trim).filter(|l| l.contains('=') && !l.starts_with('#')) {
                chain.push(format!("set \"{l}\""));
            }
        }
    }
    for (k, v) in &p.env {
        chain.push(format!("set \"{k}={v}\""));
    }
    chain.push(p.command.clone());
    let child = util::open_console(&p.name, &cwd, &chain.join(" && "), true)?;
    log(&format!("Procfile: {} başlatıldı", p.name));
    if let Some(mut old) = RUNNING.lock().insert(p.name.clone(), child) {
        let _ = old.try_wait();
    }
    Ok(())
}

pub fn autorun() {
    for p in list().into_iter().filter(|p| p.autorun) {
        if let Err(e) = run(&p.name) {
            log(&format!("Procfile {}: {e:#}", p.name));
        }
    }
}

/// "Close all": Procfile konsollarını kapatır.
pub fn stop_all() {
    for (_, mut c) in RUNNING.lock().drain() {
        util::kill_tree(c.id());
        let _ = c.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn procfile() {
        let v = parse("; yorum\nMy Cool App: autorun node start $PORT PORT=9000 env_file=usr\\my.env PWD=usr/proj DB_PASS=\"a b\"\nVite: npm run dev pwd=www/x\n");
        assert_eq!(v.len(), 2);
        assert!(v[0].autorun);
        assert_eq!(v[0].command, "node start $PORT");
        assert_eq!(v[0].pwd.as_deref(), Some("usr/proj"));
        assert_eq!(v[0].env_file.as_deref(), Some("usr\\my.env"));
        assert!(v[0].env.contains(&("DB_PASS".into(), "a b".into())));
        assert_eq!(v[1].command, "npm run dev");
    }
}
