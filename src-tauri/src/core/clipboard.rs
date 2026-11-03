//! Tulis teks ke clipboard sistem lewat Rust (satu pintu I/O, TDD 4.10 aturan 3).
//! Tulisan ini ditandai `self_write` ke Clipboard Manager agar tidak ikut masuk riwayat.

use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::plugins::clipboard::{privacy, ring, ClipState};

/// Batas wajar untuk satu salinan dari UI (prompt panjang pun jauh di bawah ini).
const MAX_BYTES: usize = 1024 * 1024;

#[tauri::command]
pub async fn clipboard_write(app: AppHandle, text: String) -> Result<(), String> {
    if text.len() > MAX_BYTES {
        return Err("Teks terlalu besar untuk disalin".into());
    }
    // Tandai SEBELUM menulis: event perubahan bisa tiba sebelum fungsi ini kembali. Hash dihitung atas teks
    // seperti yang akan disimpan watcher (dipotong 64 KB), jadi cocok dengan yang dibacanya.
    if let Some(st) = app.try_state::<ClipState>() {
        let (stored, _) = privacy::truncate_text(&text, ring::MAX_TEXT_BYTES);
        st.expect_self_write(ring::Kind::Text, ring::hash_text(&stored));
    }
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}
