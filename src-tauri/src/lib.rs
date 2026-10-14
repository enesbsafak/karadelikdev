//! KaradelikDev - Windows için yerel geliştirme ortamı yöneticisi (Laragon tarzı).

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
mod php;
mod procfile;
mod quickapp;
mod scheduler;
mod services;
mod settings;
mod setup;
mod ssl;
mod templates;
mod terminal;
mod tunnel;
mod util;
mod versions;
mod vhosts;
mod windows;
mod winenv;

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    cli::protect_std_handles();
    if let Some(code) = cli::handle(&args) {
        std::process::exit(code);
    }
    kcore::init(paths::Paths::detect());
    packages::ensure_defaults();

    let terminal_key = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyT);
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
            commands::open_url,
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
            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-off.png"))?;
            TrayIconBuilder::with_id("main")
                .icon(icon)
                .tooltip("KaradelikDev")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|_tray, ev| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                        windows::show_main();
                    }
                })
                .build(app)?;
            menu::refresh(&st);

            if let Err(e) = app.global_shortcut().register(terminal_key) {
                kcore::log(&format!("CTRL+ALT+T kısayolu atanamadı: {e}"));
            }

            let s = kcore::settings();
            let autostarted = start_args.iter().any(|a| a == "--autostart");
            if s.first_run {
                // Laragon yükleyicisinin karşılığı: ilk açılışta kurulum sihirbazı
                windows::open_setup();
            } else if !s.run_minimized {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                }
            }
            scheduler::start();
            std::thread::spawn(move || {
                let _ = terminal::write_env_cmd();
                if s.auto_start && !s.first_run {
                    let errs = services::start_all();
                    if !errs.is_empty() {
                        windows::info(&i18n::t("btn.start_all"), &i18n::t("msg.some_failed"), &errs.join("\n\n"));
                    }
                }
                procfile::autorun();
                services::broadcast();
            });
            if start_args.len() > 1 && !autostarted {
                cli::forwarded(&start_args);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("KaradelikDev başlatılamadı");

    app.run(|_app, event| {
        if let RunEvent::Exit = event {
            // Normal çıkış actions::exit ile servisleri zaten durdurur; oturum kapanışı vb. için güvence
            tunnel::stop();
            procfile::stop_all();
            services::phpcgi::stop_all();
        }
    });
}
