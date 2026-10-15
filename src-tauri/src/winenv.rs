//! Kullanıcı ortamı: PATH, dosya yöneticisi sağ tık menüsü, otomatik başlatma kaydı.
//! Windows: kayıt defteri (HKCU\Environment, HKCU\Software\Classes). macOS: `~/.zprofile` bloğu, Finder Hızlı Eylemi.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use self::macos::*;
