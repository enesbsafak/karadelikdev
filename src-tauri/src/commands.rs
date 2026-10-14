//! Web arayüzünün çağırdığı Tauri komutları.

use crate::kcore::{self, settings, update_settings};
use crate::services::{self, Svc};
use crate::settings::Settings;
use crate::windows::{self, DialogSpec};
use serde::Serialize;
use std::collections::HashMap;
use tauri::{LogicalPosition, Manager, WebviewWindow, Window};

#[derive(Serialize)]
pub struct ProjectInfo {
    name: String,
    host: String,
    url: String,
}

#[derive(Serialize)]
pub struct AppState {
    services: Vec<services::SvcStatus>,
    message: String,
    root: String,
    version: String,
    profile: String,
    document_root: String,
    projects: Vec<ProjectInfo>,
    empty: bool,
    laragon: Option<String>,
    sharing: Option<(String, String)>,
}

#[tauri::command]
pub fn get_state() -> AppState {
    let s = settings();
    let st = services::statuses();
    let empty = st.iter().all(|x| !x.installed);
    AppState {
        services: st,
        message: kcore::core().message.read().clone(),
        root: kcore::paths().root.display().to_string(),
        version: env!("CARGO_PKG_VERSION").into(),
        profile: s.profile.clone(),
        document_root: s.document_root.clone(),
        projects: crate::vhosts::projects()
            .into_iter()
            .map(|p| ProjectInfo { url: crate::vhosts::project_url(&p), name: p.name, host: p.host })
            .collect(),
        empty,
        laragon: crate::importer::find_laragon().map(|p| p.display().to_string()),
        sharing: crate::tunnel::current(),
    }
}

#[tauri::command]
pub fn i18n_table() -> HashMap<String, String> {
    crate::i18n::table(&settings().language)
}

#[tauri::command]
pub fn run_action(id: String) {
    // Senkron komutlar ana iş parçacığında çalışır; eylemler onu meşgul etmesin
    std::thread::spawn(move || crate::actions::dispatch(&id));
}

/// "Menü" düğmesinin altında native popup menüyü açar.
#[tauri::command]
pub fn show_menu(window: Window, x: f64, y: f64) -> Result<(), String> {
    let app = window.app_handle().clone();
    let menu = crate::menu::build(&app, &services::statuses()).map_err(|e| e.to_string())?;
    window.popup_menu_at(&menu, LogicalPosition::new(x, y)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings() -> Settings {
    settings()
}

#[derive(Serialize)]
pub struct Meta {
    languages: Vec<(String, String)>,
    autostart: bool,
    context_menu: bool,
    ssl_trusted: bool,
    laragon: Option<String>,
}

#[tauri::command]
pub fn get_meta(app: tauri::AppHandle) -> Meta {
    use tauri_plugin_autostart::ManagerExt;
    Meta {
        languages: crate::i18n::LANGUAGES.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        context_menu: crate::winenv::context_menu_installed(),
        ssl_trusted: crate::ssl::is_trusted(),
        laragon: crate::importer::find_laragon().map(|p| p.display().to_string()),
    }
}

#[tauri::command]
pub fn save_settings(app: tauri::AppHandle, new: Settings) -> Result<(), String> {
    use tauri::Emitter;
    use tauri_plugin_autostart::ManagerExt;
    if !new.hostname_format.contains("{name}") {
        return Err(crate::i18n::t("msg.hostname_format"));
    }
    let old = settings();
    let auto = app.autolaunch();
    if new.run_at_startup {
        let _ = auto.enable();
    } else {
        let _ = auto.disable();
    }
    if old.language != new.language {
        let _ = app.emit("i18n", crate::i18n::table(&new.language));
    }
    let web_changed = old.document_root != new.document_root
        || old.hostname_format != new.hostname_format
        || old.auto_vhosts != new.auto_vhosts
        || old.manage_hosts != new.manage_hosts
        || old.vhost_ignore != new.vhost_ignore
        || old.php_upstream != new.php_upstream
        || old.lan_access != new.lan_access;
    let data_changed = old.data_dir != new.data_dir;
    // Ayarı değişen ve çalışan servisler: eski ayarla durdur, kaydet, yeni ayarla başlat
    let restart: Vec<Svc> = Svc::ALL
        .into_iter()
        .filter(|s| {
            old.svc(s.id()) != new.svc(s.id())
                || (web_changed && matches!(s, Svc::Apache | Svc::Nginx))
                || (data_changed && matches!(s, Svc::Mysql | Svc::Postgresql | Svc::Mongodb | Svc::Redis | Svc::Mailpit))
        })
        .filter(|s| services::is_running(*s))
        .collect();
    std::thread::spawn(move || {
        for svc in restart.iter().rev() {
            let _ = services::stop(*svc);
        }
        update_settings(|s| {
            let (lb, lu, inpath) = (s.last_backup, s.last_update_check, s.in_path);
            let (pw, pws, pgs) = (s.mysql_root_password.clone(), s.mysql_passwords.clone(), s.pg_passwords.clone());
            *s = new.clone();
            s.last_backup = lb;
            s.last_update_check = lu;
            s.in_path = inpath;
            // Parolalar Tercihler'den değil veritabanı yöneticisinden değişir
            s.mysql_root_password = pw;
            s.mysql_passwords = pws;
            s.pg_passwords = pgs;
        });
        let mut restarted = vec![];
        if !restart.is_empty() || web_changed {
            services::prepare_web();
        }
        for svc in restart {
            if settings().svc(svc.id()).enabled {
                match services::start(svc) {
                    Ok(_) => restarted.push(svc.title()),
                    Err(e) => kcore::error(svc.title(), &e),
                }
            }
        }
        if web_changed {
            let _ = crate::vhosts::sync_and_reload();
        }
        let msg = if restarted.is_empty() {
            crate::i18n::t("msg.settings_saved")
        } else {
            format!("{} ({})", crate::i18n::t("msg.settings_saved"), restarted.join(", "))
        };
        kcore::status(&msg);
        services::broadcast();
    });
    Ok(())
}

#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle, current: String) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog().file().set_directory(current).blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
}

#[tauri::command]
pub async fn pick_file(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog().file().add_filter("exe", &["exe", "cmd", "bat"]).blocking_pick_file().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
}

#[tauri::command]
pub fn dialog_get(window: WebviewWindow) -> Option<DialogSpec> {
    windows::dialog_spec(window.label())
}

#[tauri::command]
pub fn dialog_submit(window: WebviewWindow, values: Vec<String>) -> Result<(), String> {
    let spec = windows::dialog_spec(window.label()).ok_or("dialog?")?;
    let label = window.label().to_string();
    windows::close_dialog(&label);
    if !spec.action.is_empty() {
        crate::actions::dispatch_with(&spec.action, &values).map_err(|e| format!("{e:#}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn dialog_close(window: WebviewWindow) {
    windows::close_dialog(window.label());
}

#[tauri::command]
pub fn setup_plan() -> serde_json::Value {
    let plan = crate::setup::plan();
    let defaults = crate::setup::default_options(&plan);
    serde_json::json!({ "plan": plan, "defaults": defaults })
}

#[tauri::command]
pub fn setup_run(app: tauri::AppHandle, options: crate::setup::SetupOptions) {
    std::thread::spawn(move || {
        use tauri_plugin_autostart::ManagerExt;
        let autostart = |on: bool| -> anyhow::Result<()> {
            let a = app.autolaunch();
            if on { a.enable() } else { a.disable() }.map_err(|e| anyhow::anyhow!("{e}"))
        };
        crate::setup::run(&options, Some(&autostart));
        services::broadcast();
    });
}

#[tauri::command]
pub fn setup_finish() {
    update_settings(|s| s.first_run = false);
    windows::close_setup();
    windows::show_main();
}

/// Veritabanı yöneticisi: bağlantı bilgileri, veritabanları, kullanıcılar, yedekler.
#[tauri::command]
pub async fn db_overview() -> serde_json::Value {
    use crate::mysqlops as m;
    let s = settings();
    let running = services::is_running(Svc::Mysql);
    let aliases = crate::vhosts::app_aliases();
    let base = crate::vhosts::localhost_url();
    let web = |name: &str| aliases.iter().any(|(a, _)| a == name).then(|| format!("{base}/{name}/"));
    let err = |e: anyhow::Error| format!("{e:#}");
    // Sorgular önce: kayıtlı parola eskiyse sql() onu düzeltir, gösterilen parola güncel olur
    let databases = if running { m::database_infos().map_err(err) } else { Ok(vec![]) };
    let users = if running { m::users().map_err(err) } else { Ok(vec![]) };
    serde_json::json!({
        "installed": services::Svc::Mysql.exe().is_some(),
        "running": running,
        "version": crate::versions::current("mysql"),
        "host": "127.0.0.1",
        "port": s.svc("mysql").port,
        "user": "root",
        "password": crate::mysqlops::root_password(),
        "databases": databases,
        "users": users,
        "backups": m::backups(),
        "backup_dir": m::backup_dir().display().to_string(),
        "phpmyadmin": web("phpmyadmin"),
        "adminer": web("adminer"),
        "heidisql": crate::versions::tool("heidisql", "heidisql.exe").is_some(),
    })
}

/// Veritabanı yöneticisinin değiştiren işlemleri. Uzun sürebilenler (yedek/geri yükleme) burada beklenir.
#[tauri::command]
pub async fn db_call(op: String, args: Vec<String>) -> Result<String, String> {
    let r = crate::mysqlops::call(&op, &args);
    services::broadcast();
    r.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn pick_sql_file(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .add_filter("SQL", &["sql"])
        .set_directory(crate::mysqlops::backup_dir())
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.display().to_string())
}

#[tauri::command]
pub fn copy_text(text: String) {
    crate::actions::copy(&text);
}

#[tauri::command]
pub fn open_url(url: String) {
    crate::actions::open_url(&url);
}
