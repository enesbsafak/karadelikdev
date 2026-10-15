//! Ana pencere, Tercihler penceresi ve küçük diyalog pencereleri (Laragon'daki InputBox/mesaj formları).

use crate::kcore::app;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Field {
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DialogSpec {
    /// input | confirm | info
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub fields: Vec<Field>,
    /// Onaylanınca çalışacak eylem kimliği; alan değerleri eklenir.
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub ok: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub svg: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub danger: bool,
}

static DIALOGS: once_cell::sync::Lazy<Mutex<HashMap<String, DialogSpec>>> = once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

pub fn dialog_spec(label: &str) -> Option<DialogSpec> {
    DIALOGS.lock().get(label).cloned()
}

pub fn open_dialog(spec: DialogSpec) {
    let Some(app) = app() else { return };
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let label = format!("dialog-{}", N.fetch_add(1, Ordering::Relaxed));
    let height = match spec.kind.as_str() {
        "info" if !spec.svg.is_empty() => 470.0,
        "info" => 420.0,
        "input" => 170.0 + 64.0 * spec.fields.len() as f64,
        "confirm" if !spec.text.is_empty() => 440.0,
        _ => 210.0,
    };
    let title = spec.title.clone();
    DIALOGS.lock().insert(label.clone(), spec);
    let app2 = app.clone();
    // Windows'ta pencere ana iş parçacığındaki bir olay işleyicisinden oluşturulursa WebView2 kilitlenir
    // (Tauri: WebviewWindowBuilder::build belgesi); bu yüzden ayrı iş parçacığından oluşturulur.
    std::thread::spawn(move || {
        let w = WebviewWindowBuilder::new(&app2, &label, WebviewUrl::App("dialog.html".into()))
            .title(title)
            .inner_size(440.0, height)
            .resizable(false)
            .minimizable(false)
            .maximizable(false)
            .always_on_top(true)
            .center()
            .build();
        if let Ok(w) = w {
            let _ = w.set_focus();
        }
    });
}

pub fn input(title: &str, label: &str, action: &str, value: &str, password: bool) {
    open_dialog(DialogSpec {
        kind: "input".into(),
        title: title.into(),
        fields: vec![Field { name: "value".into(), label: label.into(), kind: if password { "password".into() } else { "text".into() }, value: value.into() }],
        action: action.into(),
        ..Default::default()
    });
}

pub fn confirm(title: &str, message: &str, action: &str, danger: bool) {
    open_dialog(DialogSpec { kind: "confirm".into(), title: title.into(), message: message.into(), action: action.into(), danger, ..Default::default() });
}

pub fn info(title: &str, message: &str, text: &str) {
    open_dialog(DialogSpec { kind: "info".into(), title: title.into(), message: message.into(), text: text.into(), ..Default::default() });
}

pub fn close_dialog(label: &str) {
    DIALOGS.lock().remove(label);
    if let Some(w) = app().and_then(|a| a.get_webview_window(label)) {
        let _ = w.close();
    }
}

pub fn show_main() {
    if let Some(w) = app().and_then(|a| a.get_webview_window("main")) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn open_setup() {
    let Some(app) = app() else { return };
    if let Some(w) = app.get_webview_window("setup") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let app2 = app.clone();
    // Windows'ta pencere ana iş parçacığındaki bir olay işleyicisinden oluşturulursa WebView2 kilitlenir
    // (Tauri: WebviewWindowBuilder::build belgesi); bu yüzden ayrı iş parçacığından oluşturulur.
    std::thread::spawn(move || {
        let _ = WebviewWindowBuilder::new(&app2, "setup", WebviewUrl::App("setup.html".into()))
            .title(crate::i18n::t("setup.title"))
            .inner_size(640.0, 660.0)
            .resizable(false)
            .maximizable(false)
            .center()
            .build();
    });
}

pub fn open_databases() {
    let Some(app) = app() else { return };
    if let Some(w) = app.get_webview_window("databases") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let app2 = app.clone();
    std::thread::spawn(move || {
        let _ = WebviewWindowBuilder::new(&app2, "databases", WebviewUrl::App("databases.html".into()))
            .title(crate::i18n::t("db.title"))
            .inner_size(780.0, 600.0)
            .min_inner_size(640.0, 480.0)
            .center()
            .build();
    });
}

/// Projeler / Paketler / Loglar gibi yeniden boyutlandırılabilir araç pencereleri (`<etiket>.html`).
pub fn open_page(label: &'static str, title_key: &'static str, width: f64, height: f64) {
    let Some(app) = app() else { return };
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let app2 = app.clone();
    std::thread::spawn(move || {
        let _ = WebviewWindowBuilder::new(&app2, label, WebviewUrl::App(format!("{label}.html").into()))
            .title(crate::i18n::t(title_key))
            .inner_size(width, height)
            .min_inner_size(600.0, 420.0)
            .center()
            .build();
    });
}

pub fn close_setup() {
    if let Some(w) = app().and_then(|a| a.get_webview_window("setup")) {
        let _ = w.close();
    }
}

pub fn open_prefs() {
    let Some(app) = app() else { return };
    if let Some(w) = app.get_webview_window("prefs") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let app2 = app.clone();
    // Windows'ta pencere ana iş parçacığındaki bir olay işleyicisinden oluşturulursa WebView2 kilitlenir
    // (Tauri: WebviewWindowBuilder::build belgesi); bu yüzden ayrı iş parçacığından oluşturulur.
    std::thread::spawn(move || {
        let _ = WebviewWindowBuilder::new(&app2, "prefs", WebviewUrl::App("prefs.html".into()))
            .title(crate::i18n::t("menu.preferences").trim_end_matches('.').to_string())
            .inner_size(600.0, 600.0)
            .resizable(false)
            .minimizable(false)
            .maximizable(false)
            .center()
            .build();
    });
}
