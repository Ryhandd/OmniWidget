//! Adapter OS untuk Clipboard Manager: SATU-SATUNYA berkas yang menyentuh `clipboard-rs`.
//! Semua keputusan (privasi, dedup, ring, enkripsi) ada di modul murni; di sini hanya membaca/menulis
//! clipboard sistem dan mendengarkan perubahannya.
//!
//! Mekanisme perubahan (TDD 4.5), ditangani `clipboard-rs`:
//! Windows `AddClipboardFormatListener`, macOS perbandingan `changeCount`, Linux X11 XFixes.
//! Tanpa polling di sisi kita. Wayland tanpa `wlr-data-control` (mis. GNOME) tidak punya event
//! lintas-aplikasi: perubahan dari aplikasi lain bisa tidak terdeteksi (batasan platform, bukan bug).

use std::time::Duration;

use clipboard_rs::{
    common::{RustImage, RustImageData},
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext, ContentFormat,
};

use super::{privacy, ClipState};

// ---------------------------------------------------------------- Tulis (tempel kembali)

pub fn write_text(text: &str) -> Result<(), String> {
    let ctx = ClipboardContext::new().map_err(|e| e.to_string())?;
    ctx.set_text(text.to_string()).map_err(|e| e.to_string())
}

pub fn write_image(png: &[u8]) -> Result<(), String> {
    let ctx = ClipboardContext::new().map_err(|e| e.to_string())?;
    let img = RustImageData::from_bytes(png).map_err(|e| e.to_string())?;
    ctx.set_image(img).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- Baca

/// Nama proses pemilik clipboard, bila OS memberikannya. Saat ini tidak diimplementasikan di semua
/// platform (butuh API khusus OS, mis. `GetClipboardOwner` di Windows): daftar hitam aplikasi di
/// `privacy::is_denied_app` sudah siap dipakai begitu fungsi ini mengembalikan nama.
fn source_app() -> Option<String> {
    None
}

fn read_png(ctx: &ClipboardContext) -> Option<Vec<u8>> {
    let img = ctx.get_image().ok()?;
    let png = img.to_png().ok()?;
    Some(png.get_bytes().to_vec())
}

/// Dipanggil tiap clipboard berubah. Konteks dibuat per kejadian (murah, jarang) agar thread watcher
/// tidak memegang handle OS yang bisa basi.
fn capture(st: &ClipState) {
    // Dijeda: jangan baca isinya sama sekali.
    if st.settings().paused {
        return;
    }
    let Ok(ctx) = ClipboardContext::new() else { return };

    // Privasi lebih dulu, sebelum satu byte konten dibaca.
    let formats = ctx.available_formats().unwrap_or_default();
    if privacy::blocks_recording(&formats, |name| ctx.get_buffer(name).ok()) {
        return;
    }
    if let Some(app) = source_app() {
        if privacy::is_denied_app(&app) {
            return;
        }
    }

    // Teks didahulukan: menyalin sel Excel / halaman web membawa teks DAN gambar render, yang dicari teksnya.
    if ctx.has(ContentFormat::Text) {
        if let Ok(text) = ctx.get_text() {
            st.ingest_text(&text);
            return;
        }
    }
    if ctx.has(ContentFormat::Image) {
        if let Some(png) = read_png(&ctx) {
            st.ingest_image(png);
        }
    }
}

struct Handler(ClipState);

impl ClipboardHandler for Handler {
    fn on_clipboard_change(&mut self) {
        capture(&self.0);
    }
}

/// Jalankan watcher di thread sendiri (resident, TDD 5.3). `start_watch` memblokir; bila berhenti karena
/// galat dicoba lagi dengan jeda yang makin panjang, lalu menyerah (riwayat yang ada tetap berfungsi).
pub fn spawn(st: ClipState) {
    let spawned = std::thread::Builder::new().name("clip-watch".into()).spawn(move || {
        for attempt in 0..5u32 {
            match ClipboardWatcherContext::new() {
                Ok(mut watcher) => {
                    watcher.add_handler(Handler(st.clone()));
                    watcher.start_watch();
                }
                Err(e) => eprintln!("[omniwidget] clipboard: watcher gagal dibuat: {e}"),
            }
            std::thread::sleep(Duration::from_secs(2u64 << attempt));
        }
        eprintln!("[omniwidget] clipboard: watcher berhenti; perubahan clipboard tidak lagi direkam");
    });
    if let Err(e) = spawned {
        eprintln!("[omniwidget] clipboard: gagal menjalankan thread watcher: {e}");
    }
}
