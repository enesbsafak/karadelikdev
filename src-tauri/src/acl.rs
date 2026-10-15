//! Kök klasör izinleri: başka yerel kullanıcıların KaradelikDev klasörüne yazamaması denetlenir.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use self::macos::*;
