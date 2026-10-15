//! "Servis olarak çalıştır": Windows'ta Windows servisi (sc), macOS'ta kullanıcı LaunchAgent'ı (launchd).

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use self::macos::*;
