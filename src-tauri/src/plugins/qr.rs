//! QR Generator & Scanner (TDD 4.7). Generator: crate `qrcode` -> string SVG (tanpa library JS).
//! Scanner: `rqrr` (QR-only, kecil) dari file, clipboard, atau area layar (lewat overlay capture).
//! UI hanya menerima teks hasil; tautan dibuka setelah konfirmasi dan hanya http/https.

use std::path::PathBuf;
use std::sync::Mutex;

use image::{GrayImage, Rgba, RgbaImage};
use qrcode::render::svg;
use qrcode::{Color, EcLevel, QrCode};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

const MAX_TEXT: usize = 2953; // kapasitas byte QR versi 40-L
const MAX_SIDE: u32 = 2400; // gambar lebih besar diperkecil dulu (dekode tetap andal, RAM kecil)
const MAX_FILE_BYTES: u64 = 30 * 1024 * 1024;

#[derive(Default)]
pub struct QrState {
    last: Mutex<Option<ScanResult>>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    texts: Vec<String>,
    source: String,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn ec(level: &str) -> EcLevel {
    match level {
        "L" => EcLevel::L,
        "Q" => EcLevel::Q,
        "H" => EcLevel::H,
        _ => EcLevel::M,
    }
}

/// Warna #rrggbb saja (mencegah injeksi atribut SVG).
fn hex(c: &str) -> Result<[u8; 3], String> {
    let b = c.strip_prefix('#').filter(|s| s.len() == 6 && s.bytes().all(|x| x.is_ascii_hexdigit())).ok_or("Warna harus #rrggbb")?;
    let p = |i: usize| u8::from_str_radix(&b[i..i + 2], 16).unwrap();
    Ok([p(0), p(2), p(4)])
}

fn build(text: &str, level: &str) -> Result<QrCode, String> {
    if text.is_empty() {
        return Err("Isi QR kosong".into());
    }
    if text.len() > MAX_TEXT {
        return Err(format!("Teks terlalu panjang untuk QR (maks {MAX_TEXT} byte)"));
    }
    QrCode::with_error_correction_level(text.as_bytes(), ec(level)).map_err(|e| format!("Gagal membuat QR: {e}"))
}

pub fn decode_gray(img: &GrayImage) -> Vec<String> {
    let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(img.width() as usize, img.height() as usize, |x, y| {
        img.get_pixel(x as u32, y as u32).0[0]
    });
    let mut out: Vec<String> = prepared.detect_grids().into_iter().filter_map(|g| g.decode().ok().map(|(_, t)| t)).collect();
    out.dedup();
    out
}

pub fn decode_rgba(img: &RgbaImage) -> Vec<String> {
    let mut gray = image::DynamicImage::ImageRgba8(img.clone());
    if img.width().max(img.height()) > MAX_SIDE {
        gray = gray.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Triangle);
    }
    decode_gray(&gray.to_luma8())
}

pub fn remember(app: &AppHandle, texts: Vec<String>, source: &str) {
    if let Some(st) = app.try_state::<QrState>() {
        *st.last.lock().unwrap() = Some(ScanResult { texts, source: source.into() });
    }
}

// ---------------------------------------------------------------- Commands

#[tauri::command]
pub fn qr_generate(text: String, level: String, fg: String, bg: String, quiet: bool) -> Result<String, String> {
    let (f, b) = (hex(&fg)?, hex(&bg)?);
    let code = build(&text, &level)?;
    Ok(code
        .render::<svg::Color>()
        .min_dimensions(220, 220)
        .quiet_zone(quiet)
        .dark_color(svg::Color(&format!("#{:02x}{:02x}{:02x}", f[0], f[1], f[2])))
        .light_color(svg::Color(&format!("#{:02x}{:02x}{:02x}", b[0], b[1], b[2])))
        .build())
}

/// Simpan ke file yang dipilih pengguna lewat dialog simpan. `format`: "svg" | "png".
#[tauri::command]
pub async fn qr_export(path: PathBuf, text: String, level: String, fg: String, bg: String, quiet: bool, format: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        if format == "svg" {
            let svg = qr_generate(text, level, fg, bg, quiet)?;
            return std::fs::write(&path, svg).map_err(err);
        }
        let (f, b) = (hex(&fg)?, hex(&bg)?);
        let code = build(&text, &level)?;
        let n = code.width() as u32;
        let q = if quiet { 4 } else { 0 };
        let scale = (1024 / (n + 2 * q)).max(4);
        let side = (n + 2 * q) * scale;
        let colors = code.to_colors();
        let img = RgbaImage::from_fn(side, side, |x, y| {
            let (mx, my) = (x / scale, y / scale);
            let dark = mx >= q && my >= q && mx < n + q && my < n + q && colors[((my - q) * n + (mx - q)) as usize] == Color::Dark;
            let c = if dark { f } else { b };
            Rgba([c[0], c[1], c[2], 255])
        });
        img.save_with_format(&path, image::ImageFormat::Png).map_err(err)
    })
    .await
    .map_err(err)?
}

#[tauri::command]
pub async fn qr_scan_file(app: AppHandle, path: PathBuf) -> Result<Vec<String>, String> {
    let texts = tokio::task::spawn_blocking(move || -> Result<Vec<String>, String> {
        let meta = std::fs::metadata(&path).map_err(err)?;
        if meta.len() > MAX_FILE_BYTES {
            return Err("File terlalu besar".into());
        }
        let mut reader = image::ImageReader::open(&path).map_err(err)?.with_guessed_format().map_err(err)?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(12_000);
        limits.max_image_height = Some(12_000);
        reader.limits(limits);
        let img = reader.decode().map_err(|e| format!("Bukan gambar yang didukung: {e}"))?;
        Ok(decode_rgba(&img.to_rgba8()))
    })
    .await
    .map_err(err)??;
    remember(&app, texts.clone(), "file");
    Ok(texts)
}

#[tauri::command]
pub async fn qr_scan_clipboard(app: AppHandle) -> Result<Vec<String>, String> {
    let image = app.clipboard().read_image().map_err(|_| "Clipboard tidak berisi gambar".to_string())?;
    let (w, h) = (image.width(), image.height());
    let rgba = image.rgba().to_vec();
    let texts = tokio::task::spawn_blocking(move || {
        RgbaImage::from_raw(w, h, rgba).map(|img| decode_rgba(&img)).ok_or("Gambar clipboard rusak".to_string())
    })
    .await
    .map_err(err)??;
    remember(&app, texts.clone(), "clipboard");
    Ok(texts)
}

/// Hasil pindai terakhir (mis. dari area layar saat panel tertutup). Diambil sekali lalu dikosongkan.
#[tauri::command]
pub fn qr_take_last(state: State<'_, QrState>) -> Option<ScanResult> {
    state.last.lock().unwrap().take()
}

/// Buka tautan hasil pindai, hanya http/https (UI sudah meminta konfirmasi dan menampilkan URL utuh).
#[tauri::command]
pub fn qr_open_url(url: String) -> Result<(), String> {
    let lower = url.trim().to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) || url.chars().any(|c| c.is_control()) {
        return Err("Hanya tautan http/https yang boleh dibuka".into());
    }
    open::that_detached(url.trim()).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warna_divalidasi() {
        assert_eq!(hex("#ff0080").unwrap(), [255, 0, 128]);
        assert!(hex("red").is_err());
        assert!(hex("#fff\" onload=\"x").is_err());
    }

    #[test]
    fn generate_lalu_scan_bolak_balik() {
        let text = "https://polimdo.ac.id/?q=omni";
        let code = build(text, "M").unwrap();
        let n = code.width() as u32;
        let colors = code.to_colors();
        let (scale, q) = (6u32, 4u32);
        let side = (n + 2 * q) * scale;
        let img = GrayImage::from_fn(side, side, |x, y| {
            let (mx, my) = (x / scale, y / scale);
            let dark = mx >= q && my >= q && mx < n + q && my < n + q && colors[((my - q) * n + (mx - q)) as usize] == Color::Dark;
            image::Luma([if dark { 0 } else { 255 }])
        });
        assert_eq!(decode_gray(&img), vec![text.to_string()]);
    }

    #[test]
    fn svg_dan_batas_input() {
        let svg = qr_generate("halo".into(), "H".into(), "#000000".into(), "#ffffff".into(), true).unwrap();
        assert!(svg.starts_with("<?xml") && svg.contains("<svg"));
        assert!(qr_generate(String::new(), "M".into(), "#000000".into(), "#ffffff".into(), true).is_err());
        assert!(qr_open_url("javascript:alert(1)".into()).is_err());
        assert!(qr_open_url("file:///etc/passwd".into()).is_err());
    }
}
