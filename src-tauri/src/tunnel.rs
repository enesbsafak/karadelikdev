//! Projeyi internette paylaşma. Laragon yalnız ngrok kullanır; KaradelikDev hesap gerektirmeyen
//! Cloudflare Quick Tunnel'ı (cloudflared) da destekler. Herkese açık URL panoya kopyalanır.

use crate::kcore::{log, paths, settings};
use crate::i18n::{t, tf};
use crate::util::{self, hidden, which};
use anyhow::{anyhow, bail, Result};
use parking_lot::Mutex;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

pub struct Active {
    pub host: String,
    pub url: String,
    child: Child,
}

static ACTIVE: Mutex<Option<Active>> = Mutex::new(None);

fn find(tool: &str, exe: &str) -> Option<PathBuf> {
    crate::versions::tool(tool, exe).or_else(|| which(exe))
}

pub fn current() -> Option<(String, String)> {
    ACTIVE.lock().as_ref().map(|a| (a.host.clone(), a.url.clone()))
}

pub fn stop() {
    if let Some(mut a) = ACTIVE.lock().take() {
        util::kill_tree(a.child.id());
        let _ = a.child.wait();
    }
}

fn web_port() -> u16 {
    let s = settings();
    if s.svc("apache").enabled || !s.svc("nginx").enabled { s.svc("apache").port } else { s.svc("nginx").port }
}

/// Tüneli başlatır ve herkese açık URL'yi döndürür (birkaç saniye sürebilir).
pub fn share(host: &str) -> Result<String> {
    stop();
    let port = web_port();
    if !util::port_open(port) {
        bail!(tf("msg.start_first", &["Apache/Nginx"]));
    }
    let provider = settings().tunnel;
    let (url, child) = if provider == "ngrok" { ngrok(host, port)? } else { cloudflared(host, port)? };
    *ACTIVE.lock() = Some(Active { host: host.to_string(), url: url.clone(), child });
    log(&format!("Paylaşım: {host} -> {url}"));
    Ok(url)
}

fn cloudflared(host: &str, port: u16) -> Result<(String, Child)> {
    let exe = find("cloudflared", &crate::platform::exe("cloudflared")).ok_or_else(|| anyhow!(tf("msg.tool_missing", &["cloudflared"])))?;
    let mut c = hidden(exe);
    c.args(["tunnel", "--no-autoupdate", "--url", &format!("http://localhost:{port}"), "--http-host-header", host])
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = c.spawn()?;
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let logf = paths().logs().join("cloudflared.log");
    std::thread::spawn(move || {
        let re = regex::Regex::new(r"https://[a-z0-9-]+\.trycloudflare\.com").unwrap();
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            util::append_line(&logf, &line);
            if let Some(m) = re.find(&line) {
                let _ = tx.send(m.as_str().to_string());
            }
        }
    });
    match rx.recv_timeout(Duration::from_secs(40)) {
        Ok(url) => Ok((url, child)),
        Err(_) => {
            util::kill_tree(child.id());
            bail!("{}\n{}", t("msg.tunnel_failed"), util::tail(&paths().logs().join("cloudflared.log"), 5))
        }
    }
}

fn ngrok(host: &str, port: u16) -> Result<(String, Child)> {
    let exe = find("ngrok", &crate::platform::exe("ngrok")).ok_or_else(|| anyhow!(tf("msg.tool_missing", &["ngrok"])))?;
    let token = settings().ngrok_token;
    if token.trim().is_empty() {
        bail!(t("msg.ngrok_token"));
    }
    let mut cfg = hidden(&exe);
    cfg.args(["config", "add-authtoken", token.trim()]);
    let _ = util::run_capture(cfg);
    let mut c = hidden(&exe);
    c.args(["http", &format!("--host-header={host}"), &port.to_string(), "--log=stdout"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = c.spawn()?;
    // ngrok'un yerel API'sinden URL'yi al
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(600));
        if let Ok(body) = crate::packages::fetch_text("http://127.0.0.1:4040/api/tunnels") {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(u) = v["tunnels"].as_array().and_then(|a| a.iter().find_map(|t| t["public_url"].as_str().filter(|u| u.starts_with("https")))) {
                    return Ok((u.to_string(), child));
                }
            }
        }
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
    }
    util::kill_tree(child.id());
    bail!(t("msg.tunnel_failed"))
}

/// URL için SVG QR kodu (Laragon: "Auto-generated QR Code").
pub fn qr_svg(url: &str) -> String {
    use qrcode::render::svg;
    qrcode::QrCode::new(url.as_bytes())
        .map(|c| c.render::<svg::Color>().min_dimensions(220, 220).quiet_zone(true).build())
        .unwrap_or_default()
}
