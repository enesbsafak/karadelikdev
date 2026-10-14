//! Menü ve düğme eylemleri. Kimlik biçimi: `fiil` veya `fiil:arg` veya `fiil:arg:arg2`.
//! Uzun süren işler arka plan iş parçacığında çalışır; sonuç durum satırı/bildirim olarak döner.

use crate::kcore::{self, app, error, notify, paths, settings, status, update_settings};
use crate::i18n::{t, tf};
use crate::services::{self, Svc};
use crate::windows;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

fn bg(label: &'static str, f: impl FnOnce() -> Result<()> + Send + 'static) {
    std::thread::spawn(move || {
        if let Err(e) = f() {
            error(&t(label), &e);
        }
        services::broadcast();
    });
}

pub fn open_url(url: &str) {
    if let Some(a) = app() {
        let _ = a.opener().open_url(url, None::<&str>);
    }
}

pub fn open_path(p: &Path) {
    if let Some(a) = app() {
        if p.is_dir() {
            let _ = std::fs::create_dir_all(p);
        }
        let _ = a.opener().open_path(p.display().to_string(), None::<&str>);
    }
}

pub fn copy(text: &str) {
    if let Some(a) = app() {
        let _ = a.clipboard().write_text(text.to_string());
    }
}

pub fn dispatch(id: &str) {
    if let Err(e) = dispatch_with(id, &[]) {
        error(id, &e);
    }
}

/// `values`: diyalogdan gelen alan değerleri (ilk değer genellikle ad/parola).
pub fn dispatch_with(id: &str, values: &[String]) -> Result<()> {
    let mut parts = id.splitn(3, ':');
    let verb = parts.next().unwrap_or("");
    let a1 = parts.next().unwrap_or("").to_string();
    let a2 = parts.next().unwrap_or("").to_string();
    let v0 = values.first().cloned().unwrap_or_default();
    let p = paths();
    let s = settings();
    match verb {
        "noop" => {}
        "show" => windows::show_main(),
        "prefs" => windows::open_prefs(),
        "setup" => windows::open_setup(),
        "help" => windows::info(
            &format!("KaradelikDev {}", env!("CARGO_PKG_VERSION")),
            &p.root.display().to_string(),
            "karadelikdev reload [apache|nginx]\n\
             karadelikdev start [servis]\n\
             karadelikdev stop [servis]\n\
             karadelikdev do <eylem>\n\
             karadelikdev --terminal <dizin>\n\n\
             Ctrl+Alt+T  Terminal",
        ),
        "exit" => exit(),
        "web" | "localhost" => open_url(&format!("{}/", crate::vhosts::localhost_url())),
        "db" | "db_manager" => windows::open_databases(),
        "db_tool" => open_database_tool(),
        "terminal" => crate::terminal::open(Path::new(&s.document_root), None)?,
        "root" => open_path(Path::new(&s.document_root)),
        "open" => {
            let path = PathBuf::from(format!("{a1}{}{a2}", if a2.is_empty() { "" } else { ":" }));
            if path.is_dir() || path.extension().is_none() {
                let _ = std::fs::create_dir_all(&path);
                open_path(&path);
            } else {
                crate::terminal::edit(&path)?;
            }
        }
        "edit" => crate::terminal::edit(&PathBuf::from(format!("{a1}{}{a2}", if a2.is_empty() { "" } else { ":" })))?,
        "url" => open_url(&format!("{a1}:{a2}")),
        "tpl" => crate::terminal::edit(&crate::templates::materialize(&a1))?,
        "startall" => bg("btn.start_all", || {
            let errs = services::start_all();
            if errs.is_empty() {
                status(&t("msg.all_started"));
            } else {
                windows::info(&t("btn.start_all"), &t("msg.some_failed"), &errs.join("\n\n"));
            }
            Ok(())
        }),
        "stopall" => bg("btn.stop_all", || {
            services::stop_all();
            status(&t("msg.all_stopped"));
            Ok(())
        }),
        "svc" => svc_action(&a1, &a2)?,
        "ver" => {
            let (cat, ver) = (a1.clone(), a2.clone());
            bg("menu.version", move || switch_version(&cat, &ver));
        }
        "profile" => {
            let name = a1.clone();
            bg("menu.profile", move || {
                update_settings(|s| s.profile = name.clone());
                restart_running()?;
                notify(&tf("msg.profile_switched", &[&name]));
                Ok(())
            });
        }
        "profile_new" => windows::input(&t("menu.new_profile"), &t("label.profile_name"), "profile_create", "", false),
        "profile_create" => {
            let name = v0.trim().to_string();
            if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || " -_.".contains(c)) {
                return Err(anyhow!(t("msg.invalid_name")));
            }
            crate::versions::create_profile(&name)?;
            update_settings(|s| s.profile = name.clone());
            notify(&tf("msg.profile_created", &[&name]));
        }
        "site" => {
            if let Some(pr) = crate::vhosts::projects().into_iter().find(|x| x.name == a1) {
                open_url(&crate::vhosts::project_url(&pr));
            }
        }
        "docroot_switch" => pick_document_root(),
        "share" => {
            let name = a1.clone();
            bg("menu.share", move || {
                let pr = crate::vhosts::projects().into_iter().find(|x| x.name == name).ok_or_else(|| anyhow!("{name}?"))?;
                status(&tf("msg.sharing", &[&pr.host]));
                let url = crate::tunnel::share(&pr.host)?;
                copy(&url);
                show_share(&pr.host, &url);
                notify(&tf("msg.shared", &[&url]));
                Ok(())
            });
        }
        "share_show" => {
            if let Some((h, u)) = crate::tunnel::current() {
                show_share(&h, &u);
            }
        }
        "sharestop" => {
            crate::tunnel::stop();
            status(&t("msg.share_stopped"));
            services::broadcast();
        }
        "tunnel" => update_settings(|s| s.tunnel = a1.clone()),
        "ngrok_token" => windows::input("ngrok", &t("label.ngrok_token"), "ngrok_token_set", &s.ngrok_token, false),
        "ngrok_token_set" => update_settings(|s| s.ngrok_token = v0.trim().to_string()),
        "quick" => windows::input(&tf("menu.quick_create", &[&a1]), &t("label.project_name"), &format!("quick_create:{a1}"), "", false),
        "quick_create" => {
            let (tpl, name) = (a1.clone(), v0.trim().to_string());
            bg("menu.quick_app", move || {
                let msg = crate::quickapp::create(&tpl, &name)?;
                notify(&msg);
                Ok(())
            });
        }
        "delproj" => windows::confirm(
            &t("menu.delete_project"),
            &tf("msg.confirm_delete_project", &[&a1, &PathBuf::from(&s.document_root).join(&a1).display().to_string()]),
            &format!("delproj_do:{a1}"),
            true,
        ),
        "delproj_do" => {
            let name = a1.clone();
            bg("menu.delete_project", move || {
                crate::quickapp::delete(&name)?;
                notify(&tf("msg.project_deleted", &[&name]));
                Ok(())
            });
        }
        "cloneproj" => windows::input(&t("menu.clone_project"), &t("label.new_project_name"), &format!("cloneproj_do:{a1}"), &format!("{a1}-copy"), false),
        "cloneproj_do" => {
            let (src, dst) = (a1.clone(), v0.trim().to_string());
            bg("menu.clone_project", move || {
                crate::quickapp::clone(&src, &dst)?;
                notify(&tf("msg.project_cloned", &[&dst]));
                Ok(())
            });
        }
        "quickadd" => open_path(&p.usr().join("packages.conf")),
        "pkg" => {
            let name = a1.clone();
            bg("menu.quick_add", move || {
                let dest = crate::packages::install(&name)?;
                crate::setup::after_package_install(&dest);
                notify(&tf("msg.package_installed", &[&name, &dest.display().to_string()]));
                Ok(())
            });
        }
        "import_laragon" => {
            if let Some(lg) = crate::importer::find_laragon() {
                windows::confirm(&t("menu.import_laragon"), &tf("msg.confirm_import", &[&lg.display().to_string()]), "import_laragon_do", false);
            }
        }
        "import_laragon_do" => bg("menu.import_laragon", || {
            let lg = crate::importer::find_laragon().ok_or_else(|| anyhow!("Laragon?"))?;
            let list = crate::importer::import(&lg)?;
            windows::info(&t("menu.import_laragon"), &tf("msg.imported", &[&list.len().to_string()]), &list.join("\n"));
            Ok(())
        }),
        "tool" => open_tool(&a1)?,
        "netstat" => {
            crate::util::open_console("netstat", &p.root, "netstat -ano | findstr LISTENING", true)?;
        }
        "ssl" => {
            let svc = a1.clone();
            bg("menu.ssl", move || {
                let mut on = false;
                update_settings(|s| {
                    if let Some(c) = s.services.get_mut(&svc) {
                        c.ssl = !c.ssl;
                        on = c.ssl;
                    }
                });
                if on {
                    crate::ssl::ensure_ca()?;
                    if !crate::ssl::is_trusted() {
                        crate::ssl::trust()?;
                    }
                }
                crate::vhosts::sync()?;
                if let Some(sv) = Svc::from_id(&svc) {
                    if services::is_running(sv) {
                        services::reload(sv)?;
                    }
                }
                notify(&tf(if on { "msg.ssl_on" } else { "msg.ssl_off" }, &[&svc]));
                Ok(())
            });
        }
        "ssl_trust" => bg("menu.ssl", || {
            if crate::ssl::is_trusted() {
                crate::ssl::untrust()?;
                notify(&t("msg.ca_untrusted"));
            } else {
                crate::ssl::trust()?;
                notify(&t("msg.ca_trusted"));
            }
            Ok(())
        }),
        "ssl_trust_only" => bg("menu.ssl", || {
            crate::ssl::trust()?;
            notify(&t("msg.ca_trusted"));
            Ok(())
        }),
        "ssl_regen" => bg("menu.ssl", || {
            crate::ssl::regenerate_all()?;
            crate::vhosts::sync()?;
            restart_web()?;
            notify(&t("msg.certs_regenerated"));
            Ok(())
        }),
        "certmgr" => {
            let _ = std::process::Command::new("cmd").args(["/c", "start", "", "certmgr.msc"]).spawn();
        }
        "cacert_update" => bg("menu.cacert_update", || {
            crate::scheduler::update_cacert(true)?;
            crate::php::ensure_ini()?;
            notify(&t("msg.cacert_updated"));
            Ok(())
        }),
        "path_add" => {
            crate::winenv::add_to_path()?;
            update_settings(|s| s.in_path = true);
            notify(&t("msg.path_added"));
        }
        "path_remove" => {
            crate::winenv::remove_from_path()?;
            update_settings(|s| s.in_path = false);
            notify(&t("msg.path_removed"));
        }
        "path_manage" => windows::info(&t("menu.path_manage"), &t("msg.path_manage"), &crate::winenv::path_report()),
        "ctx_toggle" => {
            if crate::winenv::context_menu_installed() {
                crate::winenv::remove_context_menu()?;
                notify(&t("msg.ctx_removed"));
            } else {
                crate::winenv::add_context_menu()?;
                notify(&t("msg.ctx_added"));
            }
        }
        "hosts_edit" => crate::hosts::edit_as_admin()?,
        "vhosts_delete_auto" => {
            let n = crate::vhosts::delete_all_auto()?;
            notify(&tf("msg.auto_deleted", &[&n.to_string()]));
        }
        "vhosts_sync" | "reload" => bg("menu.vhosts", || {
            let rep = crate::vhosts::sync_and_reload()?;
            if !rep.created.is_empty() {
                notify(&tf("msg.new_project", &[&rep.created.join(", ")]));
            } else {
                status(&t("msg.reloaded"));
            }
            Ok(())
        }),
        "mysql" => mysql_action(&a1, &v0)?,
        "dbcall" => {
            let msg = crate::mysqlops::call(&a1, values)?;
            notify(&msg);
        }
        "php" => php_action(&a1, &a2)?,
        "redis_cli" => {
            let cli = crate::versions::exe("redis", "redis-cli.exe").ok_or_else(|| anyhow!(tf("msg.not_installed", &["redis-cli"])))?;
            crate::util::open_console("redis-cli", &p.root, &format!("\"{}\" -p {}", cli.display(), s.svc("redis").port), true)?;
        }
        "proc" => crate::procfile::run(&a1)?,
        "proc_closeall" => crate::procfile::stop_all(),
        other => return Err(anyhow!("bilinmeyen eylem: {other}")),
    }
    Ok(())
}

fn svc_action(id: &str, op: &str) -> Result<()> {
    let svc = Svc::from_id(id).ok_or_else(|| anyhow!("{id}?"))?;
    let op = op.to_string();
    bg("menu.service", move || {
        match op.as_str() {
            "start" => {
                if matches!(svc, Svc::Apache | Svc::Nginx) {
                    services::prepare_web();
                }
                services::start(svc)?;
                status(&tf("msg.svc_started", &[svc.title()]));
            }
            "stop" => {
                services::stop(svc)?;
                status(&tf("msg.svc_stopped", &[svc.title()]));
            }
            "reload" => {
                if matches!(svc, Svc::Apache | Svc::Nginx) {
                    services::prepare_web();
                }
                services::reload(svc)?;
                notify(&tf("msg.svc_reloaded", &[svc.title()]));
            }
            "toggle" => {
                if services::is_running(svc) {
                    services::stop(svc)?;
                } else {
                    if matches!(svc, Svc::Apache | Svc::Nginx) {
                        services::prepare_web();
                    }
                    services::start(svc)?;
                }
            }
            "winsvc" => {
                if services::winservice::installed(svc) {
                    services::winservice::uninstall(svc)?;
                    notify(&tf("msg.winsvc_removed", &[svc.title()]));
                } else {
                    if services::is_running(svc) {
                        services::stop(svc)?;
                    }
                    services::winservice::install(svc)?;
                    notify(&tf("msg.winsvc_added", &[svc.title()]));
                }
            }
            _ => {}
        }
        Ok(())
    });
    Ok(())
}

/// Sürüm değiştirme: profile yaz, ilgili servis çalışıyorsa yeniden başlat.
/// Servisler eski sürümün ayarlarıyla (exe yolu, MySQL parolası) durdurulur, ancak ondan sonra
/// profil değişir; aksi halde ör. MySQL 9.4 sunucusu 8.4'ün parolasıyla kapatılmaya çalışılır.
fn switch_version(cat: &str, ver: &str) -> Result<()> {
    let affected: Vec<Svc> = if cat == "php" {
        vec![Svc::Apache, Svc::Nginx, Svc::PhpServer]
    } else {
        Svc::ALL.into_iter().filter(|s| s.category() == cat && *s != Svc::PhpServer).collect()
    };
    let running: Vec<Svc> = affected.into_iter().filter(|s| services::is_running(*s)).collect();
    for svc in running.iter().rev() {
        services::stop(*svc)?;
    }
    if cat == "php" {
        services::phpcgi::stop_all();
    }
    crate::versions::set_current(cat, ver)?;
    if cat == "php" {
        crate::php::ensure_ini()?;
    }
    if running.iter().any(|s| matches!(s, Svc::Apache | Svc::Nginx)) {
        services::prepare_web();
    }
    for svc in running {
        services::start(svc)?;
    }
    let _ = crate::terminal::write_env_cmd();
    if settings().in_path {
        let _ = crate::winenv::add_to_path();
    }
    notify(&tf("msg.version_switched", &[&crate::versions::title(cat), ver]));
    Ok(())
}

fn restart_web() -> Result<()> {
    for svc in [Svc::Apache, Svc::Nginx] {
        if services::is_running(svc) {
            services::stop(svc)?;
            services::prepare_web();
            services::start(svc)?;
        }
    }
    Ok(())
}

/// Profil değişince çalışan servisleri yeni sürümlerle yeniden başlatır.
fn restart_running() -> Result<()> {
    let running: Vec<Svc> = Svc::START_ORDER.into_iter().filter(|s| services::is_running(*s)).collect();
    for svc in running.iter().rev() {
        services::stop(*svc)?;
    }
    services::phpcgi::stop_all();
    if running.iter().any(|s| matches!(s, Svc::Apache | Svc::Nginx)) {
        services::prepare_web();
    }
    for svc in running {
        services::start(svc)?;
    }
    Ok(())
}

fn mysql_action(op: &str, value: &str) -> Result<()> {
    match op {
        "createdb" => windows::input(&t("menu.create_db"), &t("label.db_name"), "mysql:createdb_do", "", false),
        "createdb_do" => {
            let name = value.trim().to_string();
            bg("menu.create_db", move || {
                crate::mysqlops::create_database(&name)?;
                notify(&tf("msg.db_created", &[&name]));
                Ok(())
            });
        }
        "chpw" => windows::input(&t("menu.change_root_pw"), &t("label.new_password_or_empty"), "mysql:chpw_do", "", true),
        "chpw_do" => {
            let pw = value.to_string();
            bg("menu.change_root_pw", move || {
                crate::mysqlops::change_root_password(&pw)?;
                notify(&t(if pw.is_empty() { "msg.pw_removed" } else { "msg.pw_changed" }));
                Ok(())
            });
        }
        "getpw" => {
            let pw = crate::mysqlops::root_password();
            copy(&pw);
            notify(&if pw.is_empty() { t("msg.pw_empty_info") } else { t("msg.pw_copied") });
        }
        "backup" => bg("menu.backup_all", || {
            let f = crate::mysqlops::backup_all()?;
            crate::mysqlops::prune_backups(settings().backup_keep.max(1) as usize);
            notify(&tf("msg.backup_done", &[&f.display().to_string()]));
            Ok(())
        }),
        "reset" => windows::confirm(&t("menu.reset_root_pw"), &t("msg.confirm_reset_pw"), "mysql:reset_do", true),
        "reset_do" => bg("menu.reset_root_pw", || {
            crate::mysqlops::reset_root_password()?;
            notify(&t("msg.pw_removed"));
            Ok(())
        }),
        _ => {}
    }
    Ok(())
}

fn php_action(op: &str, arg: &str) -> Result<()> {
    let arg = arg.to_string();
    match op {
        "ext" => bg("menu.extensions", move || {
            let on = crate::php::toggle_extension(&arg)?;
            restart_php_consumers()?;
            status(&format!("{arg}: {}", if on { "ON" } else { "OFF" }));
            Ok(())
        }),
        "quick" => bg("menu.quick_settings", move || {
            crate::php::toggle_quick(&arg)?;
            restart_php_consumers()?;
            status(&format!("php.ini: {arg}"));
            Ok(())
        }),
        "sendmail" => {
            let mp = crate::versions::exe("mailpit", "mailpit.exe").ok_or_else(|| anyhow!(tf("msg.not_installed", &["Mailpit"])))?;
            let line = format!("sendmail_path = \"{} sendmail --smtp-addr=127.0.0.1:{}\"", crate::paths::fwd(&mp), settings().svc("mailpit").port);
            copy(&line);
            notify(&t("msg.sendmail_copied"));
        }
        "checkupdate" => bg("menu.check_php_update", || {
            let list = crate::scheduler::check_php_updates(false)?;
            if list.is_empty() {
                notify(&t("msg.php_up_to_date"));
            } else {
                windows::confirm(&t("menu.check_php_update"), &format!("{}\n\n{}", list.join("\n"), t("msg.download_now")), "php:installupdate", false);
            }
            Ok(())
        }),
        "installupdate" => bg("menu.check_php_update", || {
            for m in crate::scheduler::check_php_updates(true)? {
                notify(&m);
            }
            Ok(())
        }),
        _ => {}
    }
    Ok(())
}

fn restart_php_consumers() -> Result<()> {
    if services::phpcgi::needed() {
        services::phpcgi::stop_all();
        services::phpcgi::ensure()?;
    }
    if services::is_running(Svc::Apache) {
        services::reload(Svc::Apache)?;
    }
    if services::is_running(Svc::PhpServer) {
        services::reload(Svc::PhpServer)?;
    }
    Ok(())
}

fn open_database_tool() {
    if let Some(h) = crate::versions::tool("heidisql", "heidisql.exe") {
        let _ = std::process::Command::new(h).spawn();
        return;
    }
    let aliases = crate::vhosts::app_aliases();
    for name in ["phpmyadmin", "adminer"] {
        if aliases.iter().any(|(a, _)| a == name) {
            open_url(&format!("{}/{name}/", crate::vhosts::localhost_url()));
            return;
        }
    }
    windows::info(&t("btn.database"), &t("msg.no_db_tool"), "");
}

fn open_tool(name: &str) -> Result<()> {
    let (cat, exe) = match name {
        "heidisql" => ("heidisql", "heidisql.exe"),
        "dbeaver" => ("dbeaver", "dbeaver.exe"),
        "notepad++" => ("notepad++", "notepad++.exe"),
        "vscode" => ("vscode", "Code.exe"),
        other => (other, ""),
    };
    let path = crate::versions::tool(cat, exe).ok_or_else(|| anyhow!(tf("msg.not_installed", &[name])))?;
    std::process::Command::new(path).spawn()?;
    Ok(())
}

fn show_share(host: &str, url: &str) {
    windows::open_dialog(windows::DialogSpec {
        kind: "info".into(),
        title: t("menu.share"),
        message: tf("msg.share_info", &[host]),
        url: url.to_string(),
        svg: crate::tunnel::qr_svg(url),
        ..Default::default()
    });
}

fn pick_document_root() {
    use tauri_plugin_dialog::DialogExt;
    let Some(a) = app() else { return };
    a.dialog().file().set_directory(settings().document_root).pick_folder(|f| {
        if let Some(path) = f.and_then(|p| p.into_path().ok()) {
            let path = path.display().to_string();
            update_settings(|s| s.document_root = path.clone());
            bg("menu.switch_docroot", move || {
                crate::vhosts::sync()?;
                restart_web()?;
                notify(&tf("msg.docroot_switched", &[&path]));
                Ok(())
            });
        }
    });
}

pub fn exit() {
    std::thread::spawn(|| {
        status(&t("msg.stopping_all"));
        crate::tunnel::stop();
        services::stop_all();
        if let Some(a) = kcore::app() {
            a.exit(0);
        }
        // Olay döngüsü çıkış isteğini işlemezse (bir kez gözlendi) süreci yine de kapat;
        // servisler bu noktada zaten durdurulmuş durumda.
        std::thread::sleep(std::time::Duration::from_secs(3));
        std::process::exit(0);
    });
}
