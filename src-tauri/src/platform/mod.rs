//! İşletim sistemine bağlı ayrıntılar tek yerde: süreç başlatma/sonlandırma, port sahipleri, yönetici izni,
//! konsol, kullanıcı yapılandırması (Windows: kayıt defteri, macOS: Application Support), hosts dosyası.
//! Her platform aynı adlı işlevleri sağlar; kodun geri kalanı `crate::platform::...` ya da `util` üzerinden kullanır.
//!
//! Sağlanan işlevler (imzalar iki platformda aynıdır):
//! - `hidden(program) -> Command`              arka planda, pencere açmadan çalışacak komut
//! - `shell(cmdline) -> Command`               komut satırını sistem kabuğunda çalıştıran gizli komut
//! - `open_console(title, cwd, cmdline, keep_open) -> Result<Child>`   görünür terminalde komut
//! - `listeners() -> HashMap<u16, u32>`        dinlenen TCP portu -> PID
//! - `kill_tree(pid)`                          süreci alt süreçleriyle sonlandırır
//! - `which(name) -> Option<PathBuf>`
//! - `run_elevated(program, args, wait) -> Result<()>`, `run_elevated_code(program, args) -> Result<u32>`
//! - `config_get(name)`, `config_set(name, value)`, `install_dirs()`, `default_data_root()`
//! - `hosts_path()`, `attach_console()`, `protect_std_handles()`
//! - `EXE_SUFFIX`, `PATH_SEP`, `LINE_END`
//!
//! Yalnız macOS'ta: `brew_prefix()`, `brew()`, `cellar_prefix()`, `terminal_script()`, `run_elevated_shell()`,
//! `app_bundle()`, `extend_gui_path()`, `sh_quote()`.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use self::macos::*;

/// Çalıştırılabilir dosya adı: Windows'ta `.exe` eklenir (`exe("httpd")` -> `httpd.exe` / `httpd`).
pub fn exe(name: &str) -> String {
    format!("{name}{EXE_SUFFIX}")
}

/// Dizinleri PATH biçiminde birleştirir (Windows `;`, macOS `:`).
pub fn join_path<I: IntoIterator<Item = S>, S: AsRef<str>>(parts: I) -> String {
    parts.into_iter().map(|s| s.as_ref().to_string()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(PATH_SEP)
}

/// Kullanıcıya gösterilen klavye kısayolu (terminal): Windows `Ctrl+Alt+T`, macOS `⌥⌘T`.
pub const TERMINAL_SHORTCUT: &str = if cfg!(target_os = "macos") { "⌥⌘T" } else { "Ctrl+Alt+T" };

/// POSIX kabuğu için tek tırnaklı dizge (macOS betikleri; Windows'ta kullanılmaz).
#[allow(dead_code)]
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Menü etiketlerinde göreli yol (ör. `bin\php` / `bin/php`).
pub fn rel(parts: &[&str]) -> String {
    parts.join(if cfg!(windows) { "\\" } else { "/" })
}
