//! Uygulama genelinde paylaşılan durum: yollar, ayarlar, çalışan süreçler, olay yayını.

use crate::paths::Paths;
use crate::settings::Settings;
use once_cell::sync::OnceCell;
use parking_lot::{Mutex, ReentrantMutex, RwLock};
use serde::Serialize;
use std::collections::HashMap;
use std::process::Child;
use tauri::{AppHandle, Emitter};

pub struct Core {
    pub paths: Paths,
    pub settings: RwLock<Settings>,
    /// Bizim başlattığımız arka plan süreçleri (anahtar: "apache", "php-cgi-9001", ...).
    pub procs: Mutex<HashMap<String, Child>>,
    pub app: OnceCell<AppHandle>,
    pub message: RwLock<String>,
    /// Servis işlemleri (başlat/durdur/yeniden yükle) sırayla çalışsın; aynı iş parçacığında
    /// iç içe çağrılabildiği için yeniden girilebilir.
    pub op_lock: ReentrantMutex<()>,
}

static CORE: OnceCell<Core> = OnceCell::new();

pub fn init(paths: Paths) -> &'static Core {
    paths.ensure_layout();
    let settings = Settings::load(&paths);
    CORE.get_or_init(|| Core {
        paths,
        settings: RwLock::new(settings),
        procs: Mutex::new(HashMap::new()),
        app: OnceCell::new(),
        message: RwLock::new(String::new()),
        op_lock: ReentrantMutex::new(()),
    })
}

pub fn core() -> &'static Core {
    CORE.get().expect("core başlatılmadı")
}

pub fn paths() -> &'static Paths {
    &core().paths
}

pub fn settings() -> Settings {
    core().settings.read().clone()
}

pub fn update_settings(f: impl FnOnce(&mut Settings)) {
    let c = core();
    let mut s = c.settings.write();
    f(&mut s);
    if let Err(e) = s.save(&c.paths) {
        log(&format!("Ayarlar kaydedilemedi: {e}"));
    }
}

pub fn log(msg: &str) {
    let line = format!("{}: {msg}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    if let Some(c) = CORE.get() {
        crate::util::append_line(&c.paths.log_file(), &line);
    }
    #[cfg(debug_assertions)]
    eprintln!("{line}");
}

pub fn emit<S: Serialize + Clone>(event: &str, payload: S) {
    if let Some(app) = CORE.get().and_then(|c| c.app.get()) {
        let _ = app.emit(event, payload);
    }
}

/// Ana penceredeki durum satırı.
pub fn status(msg: &str) {
    *core().message.write() = msg.to_string();
    emit("status", msg.to_string());
}

/// Durum satırı + Windows bildirimi (Laragon'daki tepsi balonunun karşılığı).
pub fn notify(msg: &str) {
    status(msg);
    log(msg);
    if let Some(app) = CORE.get().and_then(|c| c.app.get()) {
        use tauri_plugin_notification::NotificationExt;
        let _ = app.notification().builder().title("KaradelikDev").body(msg).show();
    }
}

pub fn error(context: &str, e: &anyhow::Error) {
    let msg = format!("{context}: {e:#}");
    log(&msg);
    status(&msg);
    emit("error", msg);
}

pub fn app() -> Option<&'static AppHandle> {
    CORE.get().and_then(|c| c.app.get())
}
