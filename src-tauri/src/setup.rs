//! İlk kurulum sihirbazı: VC++ çalışma zamanı (macOS'ta Homebrew), PATH, sağ tık menüsü,
//! oturum açılışında çalıştırma, otomatik sanal host, paketler, cacert.pem, phpMyAdmin ayarı.

use crate::i18n::{t, tf};
use crate::kcore::{emit, log, paths, settings, update_settings};
use crate::packages::{self, Package};
use crate::util;
#[cfg(windows)]
use crate::util::{hidden, run_capture};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[cfg(windows)]
const VC_URL: &str = "https://aka.ms/vc14/vc_redist.x64.exe";

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SetupOptions {
    pub run_at_startup: bool,
    pub auto_vhosts: bool,
    pub context_menu: bool,
    pub add_path: bool,
    pub import_laragon: bool,
    pub install_vc: bool,
    pub packages: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SetupPlan {
    /// "windows" | "macos" (arayüz bileşen satırlarını buna göre gösterir)
    pub platform: String,
    pub root: String,
    /// Setup ile kurulmuş: veri klasörü sihirbazdan değiştirilebilir.
    pub installed: bool,
    pub root_has_space: bool,
    pub laragon: Option<String>,
    /// Windows: VC++ çalışma zamanı sürümü. macOS: Homebrew öneki (ör. /opt/homebrew).
    pub vc_version: Option<String>,
    /// Windows: VC++ yeterli. macOS: Homebrew kurulu.
    pub vc_ok: bool,
    /// macOS: Xcode komut satırı araçları kurulu (Homebrew ister). Windows'ta her zaman true.
    pub clt_ok: bool,
    pub packages: Vec<Package>,
    pub busy_ports: Vec<String>,
}

/// HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64 (2015-2026 ortak çalışma zamanı).
#[cfg(windows)]
pub fn vc_runtime() -> Option<(u32, u32, String)> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};
    let k = winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64", KEY_READ | KEY_WOW64_64KEY)
        .ok()?;
    let installed: u32 = k.get_value("Installed").unwrap_or(0);
    if installed != 1 {
        return None;
    }
    let major: u32 = k.get_value("Major").ok()?;
    let minor: u32 = k.get_value("Minor").ok()?;
    let ver: String = k.get_value("Version").unwrap_or_default();
    Some((major, minor, ver))
}

/// macOS'ta VC++ yerine Homebrew: (sürüm yok, öneki gösterilir).
#[cfg(not(windows))]
pub fn vc_runtime() -> Option<(u32, u32, String)> {
    crate::platform::brew_prefix().map(|p| (0, 0, p.display().to_string()))
}

/// VS18 (Visual Studio 2026) ile derlenmiş Apache/PHP 14.50+ ister; diğerleri 14.40+ ile çalışır.
#[cfg(windows)]
fn vc_required_minor(pkgs: &[String]) -> u32 {
    let vs18 = pkgs.iter().any(|p| p.to_lowercase().contains("vs18"))
        || crate::versions::installed("apache").iter().chain(crate::versions::installed("php").iter()).any(|v| v.to_lowercase().contains("vs18"));
    if vs18 { 50 } else { 40 }
}

#[cfg(not(windows))]
pub fn vc_ok_for(_pkg_urls: &[String]) -> bool {
    crate::platform::brew().is_some()
}

#[cfg(windows)]
pub fn vc_ok_for(pkg_urls: &[String]) -> bool {
    match vc_runtime() {
        Some((major, minor, _)) => major > 14 || (major == 14 && minor >= vc_required_minor(pkg_urls)),
        None => false,
    }
}

pub fn plan() -> SetupPlan {
    let root = paths().root.display().to_string();
    let pkgs = packages::list();
    let urls: Vec<String> = pkgs.iter().filter(|p| p.recommended).map(|p| p.url.clone()).collect();
    let mut busy = Vec::new();
    for (port, name) in [(80u16, "Apache"), (443, "Apache SSL"), (3306, "MySQL"), (1025, "Mailpit")] {
        if util::port_open(port) {
            let owner = util::listeners()
                .get(&port)
                .and_then(|pid| util::process_path(*pid))
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "?".into());
            busy.push(format!("{name} :{port} → {owner}"));
        }
    }
    SetupPlan {
        platform: if cfg!(windows) { "windows" } else { "macos" }.into(),
        #[cfg(windows)]
        clt_ok: true,
        #[cfg(not(windows))]
        clt_ok: crate::platform::clt_installed(),
        root_has_space: root.contains(' '),
        installed: paths().installed,
        root,
        laragon: crate::importer::find_laragon().map(|p| p.display().to_string()),
        vc_version: vc_runtime().map(|v| v.2),
        vc_ok: vc_ok_for(&urls),
        packages: pkgs,
        busy_ports: busy,
    }
}

/// Kurulumlu sürümde veri klasörünü değiştirir ve uygulamayı yeniden başlatır. Bu açılışta oluşturulmuş ve
/// henüz boş olan eski klasör silinir. Var olan bir KaradelikDev klasörü seçilirse (ör. eski taşınabilir kurulum)
/// projeler, veritabanları ve ayarlar olduğu gibi kullanılır.
pub fn set_data_root(path: &str) -> Result<()> {
    let p = paths();
    let new = std::path::PathBuf::from(path.trim());
    if !new.is_absolute() {
        bail!("{path}?");
    }
    if path.contains(' ') {
        bail!(t("setup.space_warning"));
    }
    let exe_dir = crate::updater::exe_path()?.parent().map(Path::to_path_buf).unwrap_or_default();
    if new.starts_with(&exe_dir) {
        bail!(t("setup.root_in_install_dir"));
    }
    if new == p.root {
        return Ok(());
    }
    std::fs::create_dir_all(&new).with_context(|| new.display().to_string())?;
    crate::paths::set_data_root(&new)?;
    let empty = |d: std::path::PathBuf| std::fs::read_dir(d).map(|mut r| r.next().is_none()).unwrap_or(true);
    if p.created && empty(p.bin()) && empty(p.default_data()) {
        let _ = util::remove_dir_force(&p.root);
    }
    log(&format!("Veri klasörü değişti: {} -> {}", p.root.display(), new.display()));
    crate::updater::restart()
}

fn step(msg: &str) {
    log(&format!("Kurulum: {msg}"));
    emit("setup-log", msg.to_string());
    crate::kcore::status(msg);
}

/// İndirilen yükleyicinin Microsoft imzalı olduğunu doğrular (yönetici olarak çalıştırılmadan önce).
#[cfg(windows)]
fn verify_microsoft_signature(file: &Path) -> Result<()> {
    let mut c = hidden("powershell.exe");
    c.args(["-NoProfile", "-Command"]).arg(format!(
        "$s = Get-AuthenticodeSignature -LiteralPath '{}'; \"$($s.Status)|$($s.SignerCertificate.Subject)\"",
        file.display()
    ));
    let (_, out) = run_capture(c)?;
    let out = out.trim();
    if !(out.starts_with("Valid|") && out.contains("O=Microsoft Corporation")) {
        bail!("{}: {out}", t("setup.bad_signature"));
    }
    Ok(())
}

/// macOS: Homebrew'un resmi kurulum betiği Terminal'de açılır (parola ve onay ister). Kurulum bitince
/// sihirbaz yeniden açılıp paketler kurulur.
#[cfg(not(windows))]
pub fn install_vc_runtime() -> Result<()> {
    let home = crate::platform::home();
    crate::platform::terminal_script("Homebrew", &home, None, crate::platform::BREW_INSTALL, true, None)?;
    bail!(t("setup.brew_in_terminal"))
}

#[cfg(windows)]
pub fn install_vc_runtime() -> Result<()> {
    let file = paths().tmp().join("downloads").join("VC_redist.x64.exe");
    packages::download(VC_URL, &file, "VC_redist.x64.exe")?;
    packages::progress_done();
    verify_microsoft_signature(&file)?;
    step(&t("setup.vc_installing"));
    // Laragon ile aynı parametreler; 3010 = yeniden başlatma gerekli, 1638 = daha yenisi kurulu
    let code = util::run_elevated_code(&file, "/install /quiet /norestart")?;
    if ![0, 3010, 1638].contains(&code) {
        bail!("VC_redist.x64.exe: {code}");
    }
    if code == 3010 {
        step(&t("setup.vc_reboot"));
    }
    Ok(())
}

/// phpMyAdmin'i Laragon gibi ayarlar: boş parolalı root girişi, port, uzun oturum, rastgele blowfish anahtarı.
/// Laragon'dan bağlanan (junction) kopyalar değiştirilmez.
pub fn configure_phpmyadmin(dir: &Path) -> Result<bool> {
    if std::fs::symlink_metadata(dir).map(|m| m.file_type().is_symlink()).unwrap_or(true) {
        return Ok(false);
    }
    let cfg = dir.join("config.inc.php");
    if cfg.exists() {
        return Ok(false);
    }
    let port = settings().svc("mysql").port;
    let content = format!(
        "<?php\n/* KaradelikDev tarafından oluşturuldu */\n$cfg['blowfish_secret'] = '{}';\n$i = 0;\n$i++;\n$cfg['Servers'][$i]['auth_type'] = 'cookie';\n$cfg['Servers'][$i]['host'] = '127.0.0.1';\n$cfg['Servers'][$i]['port'] = '{port}';\n$cfg['Servers'][$i]['compress'] = false;\n$cfg['Servers'][$i]['AllowNoPassword'] = true;\n/* Oturum hemen düşmesin (Laragon ile aynı) */\n$cfg['LoginCookieValidity'] = 36000;\n$cfg['UploadDir'] = '';\n$cfg['SaveDir'] = '';\n$cfg['TempDir'] = '{}';\n",
        util::random_password(32),
        crate::paths::fwd(&paths().tmp())
    );
    std::fs::write(&cfg, content).with_context(|| cfg.display().to_string())?;
    Ok(true)
}

/// Paket kurulduktan sonra gereken ayarlar (Quick add ve kurulum sihirbazı ortak kullanır).
pub fn after_package_install(dest: &Path) {
    let name = dest.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    if name.starts_with("phpmyadmin") {
        match configure_phpmyadmin(dest) {
            Ok(true) => log("phpMyAdmin config.inc.php oluşturuldu"),
            Ok(false) => {}
            Err(e) => log(&format!("phpMyAdmin ayarı: {e:#}")),
        }
    }
    if dest.starts_with(paths().bin().join("php")) {
        let _ = crate::php::ensure_ini();
    }
}

/// Seçilen adımları sırayla uygular. Hata veren adım diğerlerini durdurmaz; sonunda özet döner.
pub fn run(opts: &SetupOptions, autostart: Option<&dyn Fn(bool) -> Result<()>>) -> Vec<String> {
    let mut errors = Vec::new();
    let mut attempt = |label: &str, f: &mut dyn FnMut() -> Result<()>| {
        step(label);
        if let Err(e) = f() {
            let m = format!("{label}: {e:#}");
            log(&m);
            emit("setup-log", format!("✗ {m}"));
            errors.push(m);
        }
    };

    if opts.import_laragon {
        attempt(&t("menu.import_laragon"), &mut || {
            let lg = crate::importer::find_laragon().context("Laragon?")?;
            let list = crate::importer::import(&lg)?;
            step(&tf("msg.imported", &[&list.len().to_string()]));
            Ok(())
        });
    }

    let all = packages::list();
    let selected: Vec<&Package> = all.iter().filter(|p| opts.packages.contains(&p.name)).collect();
    let urls: Vec<String> = selected.iter().map(|p| p.url.clone()).collect();
    if opts.install_vc && !vc_ok_for(&urls) {
        attempt(&t("setup.vc"), &mut || install_vc_runtime());
    }
    let brew_missing = cfg!(target_os = "macos") && !vc_ok_for(&urls);

    for pkg in selected {
        if pkg.dest.exists() {
            step(&tf("msg.already_installed", &[&pkg.name]));
            continue;
        }
        if brew_missing && pkg.url.starts_with("brew:") {
            attempt(&tf("setup.installing", &[&pkg.name]), &mut || bail!(t("setup.brew_missing")));
            continue;
        }
        let name = pkg.name.clone();
        attempt(&tf("setup.installing", &[&name]), &mut || {
            let dest = packages::install(&name)?;
            after_package_install(&dest);
            Ok(())
        });
    }

    attempt("cacert.pem", &mut || crate::scheduler::update_cacert(false));

    // Laragon'dan bağlanmış phpMyAdmin dışında kurulu olanları ayarla
    for (alias, dir) in crate::vhosts::app_aliases() {
        if alias == "phpmyadmin" {
            after_package_install(&dir);
        }
    }

    update_settings(|s| {
        s.auto_vhosts = opts.auto_vhosts;
        s.run_at_startup = opts.run_at_startup;
    });
    if let Some(f) = autostart {
        attempt(&t("prefs.run_at_startup"), &mut || f(opts.run_at_startup));
    }
    if opts.context_menu {
        attempt(&t("menu.context_menu"), &mut || crate::winenv::add_context_menu());
    }
    if crate::php::dir().is_some() {
        attempt("php.ini", &mut || crate::php::ensure_ini().map(|_| ()));
    }
    if opts.add_path {
        attempt("PATH", &mut || {
            crate::winenv::add_to_path()?;
            update_settings(|s| s.in_path = true);
            Ok(())
        });
    }
    let _ = crate::terminal::write_env_cmd();
    update_settings(|s| s.first_run = false);
    step(&if errors.is_empty() { t("setup.done") } else { t("setup.done_with_errors") });
    emit("setup-done", errors.clone());
    errors
}

/// Varsayılan seçenekler: Laragon bulunduysa içe aktar, yoksa önerilen paketler.
pub fn default_options(plan: &SetupPlan) -> SetupOptions {
    SetupOptions {
        run_at_startup: false,
        auto_vhosts: true,
        context_menu: false,
        add_path: false,
        import_laragon: plan.laragon.is_some(),
        install_vc: !plan.vc_ok,
        packages: if plan.laragon.is_some() {
            vec![]
        } else {
            // macOS: Homebrew'da zaten kurulu olanlar da (yalnız bağlanır, indirme yapılmaz)
            plan.packages.iter().filter(|p| (p.recommended && !p.installed) || packages::brew_available(p)).map(|p| p.name.clone()).collect()
        },
    }
}

/// Komut satırından pencere açmadan kurulum: `karadelikdev --setup [--import-laragon] [--no-vc] [paket,...]`
pub fn run_cli(args: &[String]) -> i32 {
    let p = plan();
    let mut opts = default_options(&p);
    opts.import_laragon = args.iter().any(|a| a == "--import-laragon");
    if args.iter().any(|a| a == "--no-vc") {
        opts.install_vc = false;
    }
    if let Some(list) = args.iter().find(|a| !a.starts_with("--")) {
        opts.packages = list.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    } else if opts.import_laragon {
        opts.packages.clear();
    }
    let dep = if cfg!(windows) { "VC++" } else { "Homebrew" };
    println!("[KaradelikDev] kök: {}  {dep}: {}", p.root, p.vc_version.clone().unwrap_or_else(|| "yok".into()));
    println!("[KaradelikDev] paketler: {}", opts.packages.join(", "));
    let errs = run(&opts, None);
    for e in &errs {
        eprintln!("[KaradelikDev] HATA {e}");
    }
    if errs.is_empty() { 0 } else { 1 }
}
