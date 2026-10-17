//! KaradelikDev - Windows ve macOS için yerel geliştirme ortamı yöneticisi (Laragon tarzı).

mod acl;
mod actions;
mod cli;
mod commands;
mod kcore;
mod hosts;
mod i18n;
mod importer;
mod menu;
mod mysqlops;
mod packages;
mod paths;
mod platform;
mod php;
mod phpext;
mod procfile;
mod projects;
mod quickapp;
mod scheduler;
mod services;
mod settings;
mod setup;
mod ssl;
mod templates;
mod terminal;
mod tunnel;
mod updater;
mod util;
mod versions;
mod vhosts;
mod windows;
mod winenv;

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Taşınabilir sürümden geçişte veri klasöründe kalan eski exe yanlışlıkla açılmasın diye adı değiştirilir.
#[cfg(windows)]
fn retire_portable_exe() {
    let old = kcore::paths().root.join("karadelikdev.exe");
    if old.is_file() {
        let to = kcore::paths().root.join("karadelikdev-tasinabilir.exe.old");
        let _ = std::fs::remove_file(&to);
        if std::fs::rename(&old, &to).is_ok() {
            kcore::log(&format!("Eski taşınabilir exe kullanım dışı: {}", to.display()));
        }
    }
}

#[cfg(not(windows))]
fn retire_portable_exe() {}

/// macOS: `karadelikdev` komutu terminalden çalışsın diye `usr/bin` (PATH'teki ilk dizin) altına uygulamanın
/// ikilisine bağlantı.
#[cfg(target_os = "macos")]
fn link_cli() {
    let Ok(exe) = std::env::current_exe() else { return };
    let link = kcore::paths().usr().join("bin").join("karadelikdev");
    if std::fs::read_link(&link).map(|t| t == exe).unwrap_or(false) {
        return;
    }
    let _ = std::fs::remove_file(&link);
    let _ = std::fs::create_dir_all(link.parent().unwrap());
    let _ = std::os::unix::fs::symlink(&exe, &link);
}

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    #[cfg(target_os = "macos")]
    platform::extend_gui_path();
    cli::protect_std_handles();
    if let Some(code) = cli::handle(&args) {
        std::process::exit(code);
    }
    kcore::init(paths::Paths::detect());
    packages::ensure_defaults();

    // Windows Ctrl+Alt+T, macOS ⌥⌘T
    let terminal_mods = if cfg!(target_os = "macos") { Modifiers::SUPER | Modifiers::ALT } else { Modifiers::CONTROL | Modifiers::ALT };
    let terminal_key = Shortcut::new(Some(terminal_mods), Code::KeyT);
    let start_args = args.clone();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, argv, _cwd| {
            // "Laragon is already running!" yerine: argümanları çalışan örneğe ilet
            cli::forwarded(&argv);
        }))
        .plugin(tauri_plugin_autostart::Builder::new().args(["--autostart"]).app_name("KaradelikDev").build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |_app, sc, ev| {
                    if ev.state() == ShortcutState::Pressed && *sc == terminal_key {
                        std::thread::spawn(|| actions::dispatch("terminal"));
                    }
                })
                .build(),
        )
        .on_menu_event(|_app, ev| {
            let id = ev.id().as_ref().to_string();
            std::thread::spawn(move || actions::dispatch(&id));
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::i18n_table,
            commands::run_action,
            commands::run_action_values,
            commands::show_menu,
            commands::get_settings,
            commands::get_meta,
            commands::save_settings,
            commands::pick_folder,
            commands::pick_file,
            commands::dialog_get,
            commands::dialog_submit,
            commands::dialog_close,
            commands::copy_text,
            commands::setup_plan,
            commands::db_overview,
            commands::db_call,
            commands::pick_sql_file,
            commands::setup_run,
            commands::setup_finish,
            commands::setup_set_root,
            commands::open_url,
            commands::projects_overview,
            commands::project_set,
            commands::packages_overview,
            commands::logs_list,
            commands::log_tail,
            commands::log_clear,
        ])
        .on_window_event(|window, event| {
            // Ana pencere kapatılınca tepsiye küçül (Laragon: "Laragon is minimized here.")
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                    static ONCE: std::sync::Once = std::sync::Once::new();
                    ONCE.call_once(|| kcore::notify(&i18n::t("msg.minimized_here")));
                }
            }
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            let _ = kcore::core().app.set(handle.clone());
            kcore::log(&format!("KaradelikDev {} başlatıldı, kök: {}", env!("CARGO_PKG_VERSION"), kcore::paths().root.display()));

            let st = services::statuses();
            let menu = menu::build(&handle, &st)?;
            let icon = tauri::image::Image::from_bytes(menu::tray_icon(false))?;
            #[cfg(target_os = "macos")]
            {
                let _ = app.set_menu(menu::app_menu(&handle, &st)?);
            }
            TrayIconBuilder::with_id("main")
                .icon(icon)
                .icon_as_template(cfg!(target_os = "macos"))
                .tooltip("KaradelikDev")
                .menu(&menu)
                // macOS menü çubuğu simgeleri tıklanınca menüyü açar; Windows'ta sol tık pencereyi gösterir
                .show_menu_on_left_click(cfg!(target_os = "macos"))
                .on_tray_icon_event(|_tray, ev| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                        windows::show_main();
                    }
                })
                .build(app)?;
            menu::refresh(&st);

            if let Err(e) = app.global_shortcut().register(terminal_key) {
                kcore::log(&format!("{} kısayolu atanamadı: {e}", platform::TERMINAL_SHORTCUT));
            }

            let s = kcore::settings();
            // Kaldırıp yeniden kurulunca veya taşınabilirden kuruluma geçince sistem kayıtlarını ayarlara göre ve
            // bu exe'nin yoluyla yeniden yaz (eski taşınabilir exe'yi göstermesinler)
            {
                use tauri_plugin_autostart::ManagerExt;
                if s.run_at_startup {
                    let _ = app.autolaunch().enable();
                }
            }
            if s.in_path && !winenv::in_path() {
                let _ = winenv::add_to_path();
            }
            if kcore::paths().installed {
                if winenv::context_menu_installed() {
                    let _ = winenv::add_context_menu();
                }
                retire_portable_exe();
            }
            #[cfg(target_os = "macos")]
            link_cli();
            let autostarted = start_args.iter().any(|a| a == "--autostart");
            // Güncelleme sonrası yeniden başlatma: `--updated <servisler>`
            let resumed = (start_args.get(1).map(String::as_str) == Some("--updated")).then(|| start_args.get(2).cloned().unwrap_or_default());
            if s.first_run {
                // Laragon yükleyicisinin karşılığı: ilk açılışta kurulum sihirbazı
                windows::open_setup();
            } else if !s.run_minimized {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                }
            }
            scheduler::start();
            let was_resumed = resumed.is_some();
            std::thread::spawn(move || {
                updater::cleanup();
                acl::check_root();
                let _ = terminal::write_env_cmd();
                if let Some(list) = &resumed {
                    updater::resume_services(list);
                } else if s.auto_start && !s.first_run {
                    let errs = services::start_all();
                    if !errs.is_empty() {
                        windows::info(&i18n::t("btn.start_all"), &i18n::t("msg.some_failed"), &errs.join("\n\n"));
                    }
                }
                procfile::autorun();
                services::broadcast();
                if s.app_update_check {
                    std::thread::sleep(std::time::Duration::from_secs(20));
                    updater::background_check();
                }
            });
            if start_args.len() > 1 && !autostarted && !was_resumed {
                cli::forwarded(&start_args);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("KaradelikDev başlatılamadı");

    app.run(|_app, event| {
        // macOS: pencere kapalıyken Dock simgesine tıklanınca ana pencere açılır
        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen { .. } = event {
            windows::show_main();
        }
        if let RunEvent::Exit = event {
            // Normal çıkış actions::exit ile servisleri zaten durdurur; oturum kapanışı vb. için güvence
            tunnel::stop();
            procfile::stop_all();
            services::phpcgi::stop_all();
        }
    });
}
