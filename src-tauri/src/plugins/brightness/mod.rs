//! Kontrol brightness hardware monitor. Di Windows memakai WMI `WmiMonitorBrightnessMethods`.
//! Tidak semua monitor eksternal mendukung; UI menampilkan pesan bila tidak tersedia.

#[cfg(windows)]
mod win;
#[cfg(not(windows))]
mod other;

#[cfg(windows)]
use win as imp;
#[cfg(not(windows))]
use other as imp;

/// Level brightness sekarang (0–100), atau `None` bila monitor tidak mendukung.
#[tauri::command]
pub async fn brightness_get() -> Result<Option<u8>, String> {
    tokio::task::spawn_blocking(imp::get).await.map_err(|e| e.to_string())?
}

/// Set brightness hardware (0–100).
#[tauri::command]
pub async fn brightness_set(level: u8) -> Result<(), String> {
    tokio::task::spawn_blocking(move || imp::set(level)).await.map_err(|e| e.to_string())?
}
