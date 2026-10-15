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
//! - `EXE_SUFFIX`

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
