//! Eyedropper, Pixel Ruler, dan pindai QR area layar (TDD 4.6) berbagi satu OVERLAY WINDOW on-demand:
//! dibuat saat aktif, dihancurkan saat selesai. Snapshot hanya monitor di bawah kursor, tetap di Rust,
//! dan dibuang begitu overlay ditutup. WebView hanya menerima potongan 15x15 (900 byte) untuk loupe.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::RgbaImage;
use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindowBuilder};

use crate::core::lifecycle;

pub const OVERLAY: &str = "overlay";
const PATCH: i64 = 15;
const RECENT: usize = 8;

struct Snap {
    img: RgbaImage,
    /// Ukuran overlay dalam piksel fisik (= monitor). Gambar bisa berbeda ukuran di beberapa OS.
    phys_w: f64,
    phys_h: f64,
    scale: f64,
}

#[derive(Default)]
pub struct CaptureState {
    snap: Mutex<Option<Snap>>,
    recent: Mutex<VecDeque<[u8; 3]>>,
    main_was_visible: Mutex<bool>,
    mode: Mutex<Option<String>>,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

impl Snap {
    /// Koordinat CSS overlay -> piksel gambar.
    fn map(&self, x: f64, y: f64) -> (i64, i64) {
        let px = x * self.scale * self.img.width() as f64 / self.phys_w;
        let py = y * self.scale * self.img.height() as f64 / self.phys_h;
        (px.floor() as i64, py.floor() as i64)
    }

    fn pixel(&self, x: i64, y: i64) -> Option<[u8; 4]> {
        if x < 0 || y < 0 || x >= self.img.width() as i64 || y >= self.img.height() as i64 {
            return None;
        }
        Some(self.img.get_pixel(x as u32, y as u32).0)
    }

    fn patch(&self, cx: i64, cy: i64) -> Vec<u8> {
        let half = PATCH / 2;
        let mut out = Vec::with_capacity((PATCH * PATCH * 4) as usize);
        for y in cy - half..=cy + half {
            for x in cx - half..=cx + half {
                out.extend_from_slice(&self.pixel(x, y).unwrap_or([0, 0, 0, 0]));
            }
        }
        out
    }

    fn crop(&self, x: f64, y: f64, w: f64, h: f64) -> Option<RgbaImage> {
        let (x0, y0) = self.map(x, y);
        let (x1, y1) = self.map(x + w, y + h);
        let (iw, ih) = (self.img.width() as i64, self.img.height() as i64);
        let (x0, y0, x1, y1) = (x0.clamp(0, iw), y0.clamp(0, ih), x1.clamp(0, iw), y1.clamp(0, ih));
        if x1 - x0 < 8 || y1 - y0 < 8 {
            return None;
        }
        Some(image::imageops::crop_imm(&self.img, x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32).to_image())
    }
}

/// Bersihkan semua state transien + tampilkan lagi widget. Aman dipanggil berulang.
pub fn cleanup(app: &AppHandle) {
    let Some(st) = app.try_state::<CaptureState>() else { return };
    st.snap.lock().unwrap().take();
    st.mode.lock().unwrap().take();
    if std::mem::take(&mut *st.main_was_visible.lock().unwrap()) {
        if let Some(w) = app.get_webview_window(lifecycle::MAIN) {
            let _ = w.show();
        }
    }
}

fn close_overlay(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(OVERLAY) {
        let _ = w.destroy();
    }
    cleanup(app);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recent {
    colors: Vec<[u8; 3]>,
}

// ---------------------------------------------------------------- Commands

/// Mulai mode capture: "eyedropper" | "ruler" | "qr" | "draw".
#[tauri::command]
pub async fn capture_start(app: AppHandle, state: State<'_, CaptureState>, mode: String) -> Result<(), String> {
    if !matches!(mode.as_str(), "eyedropper" | "ruler" | "qr" | "draw") {
        return Err("Mode tidak dikenal".into());
    }
    if let Some(w) = app.get_webview_window(OVERLAY) {
        let _ = w.set_focus();
        return Ok(());
    }

    // Monitor tempat kursor berada (fallback: monitor utama).
    let cursor = app.cursor_position().map_err(err)?;
    let monitors = app.available_monitors().map_err(err)?;
    let mon = monitors
        .iter()
        .find(|m| {
            let (p, s) = (m.position(), m.size());
            cursor.x >= p.x as f64 && cursor.y >= p.y as f64 && cursor.x < (p.x + s.width as i32) as f64 && cursor.y < (p.y + s.height as i32) as f64
        })
        .cloned()
        .or(app.primary_monitor().map_err(err)?)
        .ok_or("Monitor tidak ditemukan")?;
    let (pos, size, scale) = (*mon.position(), *mon.size(), mon.scale_factor());

    // Sembunyikan widget agar tidak ikut terekam / menutupi area ukur.
    let main = app.get_webview_window(lifecycle::MAIN);
    let visible = main.as_ref().and_then(|w| w.is_visible().ok()).unwrap_or(false);
    *state.main_was_visible.lock().unwrap() = visible;
    if let (true, Some(w)) = (visible, &main) {
        let _ = w.hide();
        tokio::time::sleep(Duration::from_millis(160)).await;
    }

    if mode != "ruler" {
        let (cx, cy) = (cursor.x as i32, cursor.y as i32);
        let shot = tokio::task::spawn_blocking(move || -> Result<RgbaImage, String> {
            let m = xcap::Monitor::from_point(cx, cy).or_else(|_| {
                xcap::Monitor::all().map_err(err)?.into_iter().find(|m| m.is_primary().unwrap_or(false)).ok_or("Monitor tidak ditemukan".to_string())
            });
            let m = m.map_err(err)?;
            m.capture_image().map_err(|e| {
                format!("Gagal menangkap layar: {e}. Di macOS, izinkan Screen Recording untuk OmniWidget; di Wayland gunakan sesi X11.")
            })
        })
        .await
        .map_err(err)?;
        match shot {
            Ok(img) => {
                *state.snap.lock().unwrap() = Some(Snap { img, phys_w: size.width as f64, phys_h: size.height as f64, scale });
            }
            Err(e) => {
                cleanup(&app);
                return Err(e);
            }
        }
    }
    *state.mode.lock().unwrap() = Some(mode.clone());

    let init = format!("window.__OVERLAY__ = {{ mode: {:?}, scale: {} }};", mode, scale);
    let win = WebviewWindowBuilder::new(&app, OVERLAY, WebviewUrl::App("overlay.html".into()))
        .title("OmniWidget Capture")
        .initialization_script(&init)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .visible(false)
        .build()
        .map_err(|e| {
            cleanup(&app);
            err(e)
        })?;
    let _ = win.set_position(PhysicalPosition::new(pos.x, pos.y));
    let _ = win.set_size(PhysicalSize::new(size.width, size.height));
    let _ = win.show();
    let _ = win.set_focus();
    Ok(())
}

/// Potongan 15x15 RGBA (900 byte, biner mentah) di sekitar titik CSS (x, y) overlay.
#[tauri::command]
pub fn capture_patch(state: State<'_, CaptureState>, x: f64, y: f64) -> Response {
    let g = state.snap.lock().unwrap();
    let bytes = match g.as_ref() {
        Some(s) => {
            let (px, py) = s.map(x, y);
            s.patch(px, py)
        }
        None => vec![0; (PATCH * PATCH * 4) as usize],
    };
    Response::new(bytes)
}

/// Ambil warna di titik (x, y), catat ke riwayat, beri kilasan di pill. Mengembalikan [r, g, b].
#[tauri::command]
pub fn capture_pick(app: AppHandle, state: State<'_, CaptureState>, x: f64, y: f64, label: String) -> Result<[u8; 3], String> {
    let rgb = {
        let g = state.snap.lock().unwrap();
        let s = g.as_ref().ok_or("Snapshot tidak ada")?;
        let (px, py) = s.map(x, y);
        let p = s.pixel(px, py).ok_or("Di luar layar")?;
        [p[0], p[1], p[2]]
    };
    let mut r = state.recent.lock().unwrap();
    r.retain(|c| *c != rgb);
    r.push_front(rgb);
    r.truncate(RECENT);
    drop(r);
    let label: String = label.chars().take(32).collect();
    let _ = app.emit(
        "shell://indicator",
        serde_json::json!({ "id": "capture", "icon": "◉", "label": label, "tone": "ok", "tab": "capture", "ttl": 2500 }),
    );
    Ok(rgb)
}

/// Pindai QR di area terpilih (koordinat CSS overlay). Hasil disimpan untuk modul QR.
#[tauri::command]
pub async fn capture_qr(app: AppHandle, x: f64, y: f64, w: f64, h: f64) -> Result<Vec<String>, String> {
    let crop = {
        let st = app.state::<CaptureState>();
        let g = st.snap.lock().unwrap();
        g.as_ref().ok_or("Snapshot tidak ada")?.crop(x, y, w, h).ok_or("Area terlalu kecil")?
    };
    let found = tokio::task::spawn_blocking(move || crate::plugins::qr::decode_rgba(&crop)).await.map_err(err)?;
    crate::plugins::qr::remember(&app, found.clone(), "area layar");
    Ok(found)
}

/// Salin hasil coret layar + screenshot ke clipboard sebagai satu PNG.
/// Frontend mengirim PNG transparan berisi coretan; Rust menimpanya ke snapshot layar asli.
#[tauri::command]
pub async fn capture_annotate(app: AppHandle, state: State<'_, CaptureState>, png: String) -> Result<(), String> {
    // Batas string base64 (PNG transparan 4K biasanya jauh di bawah ini).
    const MAX_B64: usize = 48 * 1024 * 1024;
    let raw = png.split_once(',').map(|(_, d)| d).unwrap_or(png.as_str());
    if raw.len() > MAX_B64 {
        return Err("Coretan terlalu besar untuk disalin".into());
    }
    let bytes = STANDARD.decode(raw).map_err(|e| format!("Gambar coretan tidak sah: {e}"))?;
    let base = {
        let g = state.snap.lock().unwrap();
        g.as_ref().ok_or("Snapshot layar tidak tersedia")?.img.clone()
    };
    let out = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let layer = image::load_from_memory(&bytes).map_err(err)?.to_rgba8();
        let mut merged = base;
        let layer = if layer.dimensions() != merged.dimensions() {
            image::imageops::resize(&layer, merged.width(), merged.height(), image::imageops::FilterType::Triangle)
        } else {
            layer
        };
        image::imageops::overlay(&mut merged, &layer, 0, 0);
        let mut png = Vec::new();
        merged.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(err)?;
        Ok(png)
    })
    .await
    .map_err(err)??;
    // Tandai sebagai tulisan kita sendiri agar hasil coretan tidak masuk riwayat Clipboard Manager.
    if let Some(st) = app.try_state::<crate::plugins::clipboard::ClipState>() {
        st.expect_self_write(crate::plugins::clipboard::ring::Kind::Image, 0);
    }
    tokio::task::spawn_blocking(move || crate::plugins::clipboard::watcher::write_image(&out))
        .await
        .map_err(err)??;
    let _ = app.emit(
        "shell://indicator",
        serde_json::json!({ "id": "capture", "icon": "✎", "label": "Coretan disalin", "tone": "ok", "tab": "capture", "ttl": 2500 }),
    );
    Ok(())
}

/// Tutup overlay. `open_tab`: buka panel widget ke modul tertentu sesudahnya (mis. hasil QR).
#[tauri::command]
pub async fn capture_close(app: AppHandle, open_tab: Option<String>) -> Result<(), String> {
    close_overlay(&app);
    if let Some(tab) = open_tab {
        if let Some(w) = app.get_webview_window(lifecycle::MAIN) {
            let _ = w.show();
            let _ = w.set_focus();
            let _ = w.emit("shell://open", serde_json::json!({ "tab": tab }));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn capture_recent(state: State<'_, CaptureState>) -> Recent {
    Recent { colors: state.recent.lock().unwrap().iter().copied().collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap() -> Snap {
        // Gambar 40x20 fisik untuk monitor 40x20 dengan skala 2 (overlay CSS 20x10).
        let mut img = RgbaImage::new(40, 20);
        img.put_pixel(10, 6, image::Rgba([1, 2, 3, 255]));
        Snap { img, phys_w: 40.0, phys_h: 20.0, scale: 2.0 }
    }

    #[test]
    fn pemetaan_css_ke_piksel_memperhitungkan_dpi() {
        let s = snap();
        assert_eq!(s.map(5.0, 3.0), (10, 6));
        assert_eq!(s.pixel(10, 6), Some([1, 2, 3, 255]));
        assert_eq!(s.pixel(-1, 0), None);
    }

    #[test]
    fn patch_selalu_15x15_dan_tepi_transparan() {
        let s = snap();
        let p = s.patch(0, 0);
        assert_eq!(p.len(), 900);
        assert_eq!(&p[0..4], &[0, 0, 0, 0], "di luar gambar = transparan");
        let c = s.patch(10, 6);
        let mid = (7 * 15 + 7) * 4;
        assert_eq!(&c[mid..mid + 4], &[1, 2, 3, 255]);
    }

    #[test]
    fn crop_menolak_area_kecil() {
        let s = snap();
        assert!(s.crop(0.0, 0.0, 2.0, 2.0).is_none());
        assert_eq!(s.crop(0.0, 0.0, 10.0, 10.0).unwrap().dimensions(), (20, 20));
    }
}
