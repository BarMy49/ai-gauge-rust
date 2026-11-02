#[cfg(windows)]
#[path = "tray_windows.rs"]
mod platform;
#[cfg(target_os = "linux")]
#[path = "tray_linux.rs"]
mod platform;
pub use platform::Tray;
