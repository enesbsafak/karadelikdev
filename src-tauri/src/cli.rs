//! Komut satırı. Laragon'daki `laragon reload [apache|nginx]` karşılığı ve iç yardımcı komutlar.
//!
//!   karadelikdev reload [apache|nginx]   vhost'ları yenile, web sunucularını yeniden yükle
//!   karadelikdev start|stop [servis]      servis(ler)i başlat/durdur
//!   karadelikdev --terminal <dizin>       KaradelikDev ortamıyla terminal aç (sağ tık menüsü)
//!   karadelikdev --post-create <proje>    Hızlı uygulama sonrası .env + veritabanı (terminalden çağrılır)
//!   karadelikdev --apply-hosts <dosya>    (iç) yükseltilmiş hosts yazımı

use crate::paths::Paths;
use std::path::Path;

fn attach_console() {
    crate::platform::attach_console();
}

/// Kendi std tutamaçlarımızı devredilemez yap: aksi halde başlattığımız httpd/mysqld bunları miras
/// alır ve bizi çağıran betiğin borusu (pipe) servisler kapanana kadar açık kalır.
pub fn protect_std_handles() {
    crate::platform::protect_std_handles();
}

const USAGE: &str = "KaradelikDev\n\nKullanım:\n  karadelikdev reload [apache|nginx]\n  karadelikdev start [servis]\n  karadelikdev stop [servis]\n  karadelikdev --terminal <dizin>\n";

/// GUI açılmadan bitmesi gereken komutları çalıştırır. `Some(çıkış kodu)` dönerse süreç sonlanır.
pub fn handle(args: &[String]) -> Option<i32> {
    let cmd = args.get(1)?.as_str();
    match cmd {
        "--apply-hosts" => {
            let file = args.get(2)?;
            Some(match crate::hosts::apply_from(Path::new(file)) {
                Ok(_) => 0,
                Err(_) => 1,
            })
        }
        "--post-create" => {
            attach_console();
            crate::kcore::init(Paths::detect());
            let name = args.get(2)?;
            Some(match crate::quickapp::post_create(name) {
                Ok(_) => 0,
                Err(e) => {
                    eprintln!("[KaradelikDev] {e:#}");
                    1
                }
            })
        }
        "--setup" => {
            attach_console();
            crate::kcore::init(Paths::detect());
            crate::packages::ensure_defaults();
            Some(crate::setup::run_cli(&args[2..]))
        }
        // Pencere açmadan servisleri başlat/durdur (betikler ve testler için)
        "--headless" => {
            attach_console();
            crate::kcore::init(Paths::detect());
            crate::packages::ensure_defaults();
            match args.get(2).map(String::as_str) {
                Some("start") => {
                    crate::acl::check_root();
                    let errs = crate::services::start_all();
                    for e in &errs {
                        eprintln!("[KaradelikDev] {e}");
                    }
                    Some(if errs.is_empty() { 0 } else { 1 })
                }
                Some("stop") => {
                    crate::services::stop_all();
                    Some(0)
                }
                // PostgreSQL çalışırken `postgres` parolasını ayarlar (boş = parolasız)
                Some("pg-password") => Some(match crate::services::others::pg_set_password(args.get(3).map(String::as_str).unwrap_or("")) {
                    Ok(_) => 0,
                    Err(e) => {
                        eprintln!("[KaradelikDev] {e:#}");
                        1
                    }
                }),
                _ => Some(2),
            }
        }
        // Kaldırıcı (NSIS) çağırır: servisleri durdurur, sisteme eklenen kayıtları siler. Veriler kalır.
        "--uninstall" => {
            attach_console();
            crate::kcore::init(Paths::detect());
            crate::services::stop_all();
            crate::winenv::remove_autostart();
            if crate::winenv::context_menu_installed() {
                let _ = crate::winenv::remove_context_menu();
            }
            if crate::winenv::in_path() {
                let _ = crate::winenv::remove_from_path();
            }
            for svc in crate::services::Svc::ALL {
                if crate::services::winservice::installed(svc) {
                    let _ = crate::services::winservice::uninstall(svc);
                }
            }
            crate::kcore::log("KaradelikDev kaldırıldı (veri klasörü korunur)");
            Some(0)
        }
        "--terminal" => {
            crate::kcore::init(Paths::detect());
            let dir = args.get(2).map(|d| d.trim_end_matches('"').to_string()).unwrap_or_else(|| crate::kcore::settings().document_root);
            Some(crate::terminal::open(Path::new(&dir), None).map(|_| 0).unwrap_or(1))
        }
        "help" | "--help" | "-h" | "/?" => {
            attach_console();
            println!("{USAGE}");
            Some(0)
        }
        _ => None,
    }
}

/// Çalışan örneğe iletilen argümanlar (single-instance) veya ilk açılış argümanları.
pub fn forwarded(argv: &[String]) {
    let cmd = argv.get(1).map(String::as_str).unwrap_or("");
    let arg = argv.get(2).map(String::as_str).unwrap_or("");
    match cmd {
        "reload" => {
            if arg.is_empty() {
                crate::actions::dispatch("reload");
            } else {
                crate::actions::dispatch(&format!("svc:{arg}:reload"));
            }
        }
        "start" if arg.is_empty() => crate::actions::dispatch("startall"),
        "stop" if arg.is_empty() => crate::actions::dispatch("stopall"),
        "start" | "stop" => crate::actions::dispatch(&format!("svc:{arg}:{cmd}")),
        // Betik/otomasyon: menü eylemini çalıştır (ör. `karadelikdev do startall`)
        "do" if !arg.is_empty() => {
            let values: Vec<String> = argv.iter().skip(3).cloned().collect();
            if let Err(e) = crate::actions::dispatch_with(arg, &values) {
                crate::kcore::error(arg, &e);
            }
        }
        "--autostart" => {}
        _ => crate::windows::show_main(),
    }
}
