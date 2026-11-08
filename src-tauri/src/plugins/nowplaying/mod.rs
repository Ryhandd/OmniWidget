//! Now-playing lintas aplikasi tanpa Premium, lewat Windows SMTC (`Windows.Media.Control`).
//!
//! Kenapa SMTC: Spotify Free tetap mendaftarkan sesi medianya ke Windows. Dari situ kita dapat
//! judul/artis/album/sampul, posisi, durasi, status putar, acak/ulang, dan perintah
//! play/pause/next/prev/seek/shuffle/repeat — tanpa API key, tanpa login, tanpa langganan.
//! Volume memakai Windows Core Audio (sesi audio Spotify, fallback volume sistem).
//!
//! Seluruh panggilan WinRT/COM dijalankan di satu thread pekerja (MTA) dengan runtime tokio
//! current-thread; command Tauri hanya mengirim pesan dan menunggu balasan oneshot. Ini menjaga
//! objek COM tetap di apartment yang sama dan tidak memblokir runtime utama.

#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
pub use win::*;

#[cfg(not(target_os = "windows"))]
mod other;
#[cfg(not(target_os = "windows"))]
pub use other::*;
