//! Gambar clipboard di disk (TDD 4.5): PNG penuh disimpan TERENKRIPSI di folder cache aplikasi
//! (`<hash>.bin`); hanya thumbnail <= 20 KB yang boleh tinggal di RAM.
//!
//! Berkas selalu dienkripsi. Saat persistensi mati, kuncinya acak di memori saja, sehingga sisa berkas
//! setelah crash tidak bisa dibaca; saat hidup, kuncinya dari keychain.

use std::{
    collections::HashSet,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

use image::{DynamicImage, ImageFormat};

use super::crypto::Sealer;

/// PNG lebih besar dari ini tidak direkam (anggaran disk total 50 MB, TDD 5.5).
pub const MAX_PNG_BYTES: usize = 20 * 1024 * 1024;
/// Batas thumbnail di RAM per item (TDD 4.5).
pub const THUMB_MAX_BYTES: usize = 20 * 1024;
/// Sisi terpanjang dicoba berurutan sampai PNG-nya muat di `THUMB_MAX_BYTES` (UI menampilkan 40 px).
const THUMB_SIDES: [u32; 4] = [96, 72, 48, 32];

pub fn path_for(dir: &Path, hash: u64) -> PathBuf {
    dir.join(format!("{hash:016x}.bin"))
}

fn aad(hash: u64) -> Vec<u8> {
    let mut v = b"omniwidget.clip.img.v1".to_vec();
    v.extend_from_slice(&hash.to_le_bytes());
    v
}

pub fn exists(dir: &Path, hash: u64) -> bool {
    path_for(dir, hash).is_file()
}

/// Tulis atomik (tmp lalu rename): crash di tengah tidak meninggalkan berkas setengah jadi di nama final.
pub fn write(dir: &Path, hash: u64, sealer: &Sealer, png: &[u8]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("Gagal membuat folder cache clipboard: {e}"))?;
    let sealed = sealer.seal(&aad(hash), png)?;
    let tmp = dir.join(format!("{hash:016x}.tmp"));
    fs::write(&tmp, &sealed).map_err(|e| format!("Gagal menulis gambar: {e}"))?;
    fs::rename(&tmp, path_for(dir, hash)).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Gagal menyimpan gambar: {e}")
    })
}

pub fn read(dir: &Path, hash: u64, sealer: &Sealer) -> Result<Vec<u8>, String> {
    let blob = fs::read(path_for(dir, hash)).map_err(|e| format!("Berkas gambar tidak bisa dibaca: {e}"))?;
    sealer.open(&aad(hash), &blob)
}

pub fn delete(dir: &Path, hash: u64) {
    let _ = fs::remove_file(path_for(dir, hash));
}

/// Hapus semua berkas kecuali gambar yang hash-nya ada di `keep` (termasuk sisa `.tmp`).
/// Mengembalikan byte yang dibebaskan.
pub fn retain_only(dir: &Path, keep: &HashSet<u64>) -> u64 {
    sweep(dir, |h| keep.contains(&h))
}

pub fn purge_all(dir: &Path) -> u64 {
    sweep(dir, |_| false)
}

fn sweep(dir: &Path, keep: impl Fn(u64) -> bool) -> u64 {
    let Ok(rd) = fs::read_dir(dir) else { return 0 };
    let mut freed = 0;
    for e in rd.flatten() {
        let p = e.path();
        let is_bin = p.extension().map_or(false, |x| x == "bin");
        let hash = p.file_stem().and_then(|s| s.to_str()).and_then(|s| u64::from_str_radix(s, 16).ok());
        if is_bin && hash.map_or(false, &keep) {
            continue;
        }
        if let Ok(m) = e.metadata() {
            if m.is_file() {
                freed += m.len();
                let _ = fs::remove_file(&p);
            }
        }
    }
    freed
}

/// Enkripsi ulang semua berkas dengan kunci baru (saat persistensi dinyalakan/dimatikan).
/// Mengembalikan hash yang GAGAL dipindahkan (pemanggil membuang item-nya).
pub fn reseal_all(dir: &Path, hashes: &[u64], old: &Sealer, new: &Sealer) -> Vec<u64> {
    hashes
        .iter()
        .copied()
        .filter(|&h| read(dir, h, old).and_then(|png| write(dir, h, new, &png)).is_err())
        .collect()
}

pub struct Analysis {
    pub width: u32,
    pub height: u32,
    pub thumb: Vec<u8>,
}

/// Dekode sekali: ukuran asli + thumbnail PNG <= 20 KB.
pub fn analyze(png: &[u8]) -> Result<Analysis, String> {
    let img = image::load_from_memory_with_format(png, ImageFormat::Png).map_err(|e| format!("PNG tidak valid: {e}"))?;
    Ok(Analysis { width: img.width(), height: img.height(), thumb: make_thumb(&img)? })
}

/// Bangun ulang thumbnail dari PNG penuh (setelah dilepas dari RAM).
pub fn thumb_from_png(png: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory_with_format(png, ImageFormat::Png).map_err(|e| format!("PNG tidak valid: {e}"))?;
    make_thumb(&img)
}

fn make_thumb(img: &DynamicImage) -> Result<Vec<u8>, String> {
    let opaque = !img.color().has_alpha();
    for side in THUMB_SIDES {
        // `thumbnail` bisa memperbesar gambar kecil; thumbnail tidak boleh lebih besar dari aslinya.
        let t = if img.width() <= side && img.height() <= side { img.clone() } else { img.thumbnail(side, side) };
        let t = if opaque { DynamicImage::ImageRgb8(t.to_rgb8()) } else { DynamicImage::ImageRgba8(t.to_rgba8()) };
        let mut cur = Cursor::new(Vec::new());
        t.write_to(&mut cur, ImageFormat::Png).map_err(|e| format!("Gagal membuat thumbnail: {e}"))?;
        let buf = cur.into_inner();
        if buf.len() <= THUMB_MAX_BYTES {
            return Ok(buf);
        }
    }
    Err("Thumbnail melebihi 20 KB".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, noisy: bool) -> Vec<u8> {
        let img = image::RgbaImage::from_fn(w, h, |x, y| {
            if noisy {
                let v = x.wrapping_mul(2_654_435_761).wrapping_add(y.wrapping_mul(40_503)) >> 5;
                image::Rgba([v as u8, (v >> 8) as u8, (v >> 16) as u8, 255])
            } else {
                image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
            }
        });
        let mut cur = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut cur, ImageFormat::Png).unwrap();
        cur.into_inner()
    }

    #[test]
    fn analyze_memberi_ukuran_asli_dan_thumbnail_dalam_batas() {
        let a = analyze(&png(400, 300, false)).unwrap();
        assert_eq!((a.width, a.height), (400, 300));
        assert!(a.thumb.len() <= THUMB_MAX_BYTES);
        let t = image::load_from_memory(&a.thumb).unwrap();
        assert!(t.width() <= 96 && t.height() <= 96);
        assert_eq!(t.width() * 3, t.height() * 4, "rasio 4:3 terjaga"); // 96x72
    }

    #[test]
    fn gambar_derau_turun_ke_sisi_lebih_kecil_agar_muat() {
        let a = analyze(&png(512, 512, true)).unwrap();
        assert!(a.thumb.len() <= THUMB_MAX_BYTES, "thumb {} byte", a.thumb.len());
    }

    #[test]
    fn gambar_kecil_tidak_diperbesar_dan_png_rusak_ditolak() {
        let a = analyze(&png(20, 10, false)).unwrap();
        let t = image::load_from_memory(&a.thumb).unwrap();
        assert_eq!((t.width(), t.height()), (20, 10));
        assert!(analyze(b"bukan png").is_err());
        assert!(thumb_from_png(&[]).is_err());
    }

    #[test]
    fn tulis_baca_terenkripsi_dan_tidak_terbaca_dengan_kunci_lain() {
        let dir = tempfile::tempdir().unwrap();
        let s = Sealer::random().unwrap();
        let data = png(30, 30, false);
        write(dir.path(), 42, &s, &data).unwrap();
        assert!(exists(dir.path(), 42));
        let raw = fs::read(path_for(dir.path(), 42)).unwrap();
        assert!(!raw.windows(4).any(|w| w == b"PNG\r"), "isi di disk tidak boleh berupa PNG polos");
        assert_eq!(read(dir.path(), 42, &s).unwrap(), data);
        assert!(read(dir.path(), 42, &Sealer::random().unwrap()).is_err());
        // Berkas dipindah ke nama hash lain: AAD menolak.
        fs::copy(path_for(dir.path(), 42), path_for(dir.path(), 43)).unwrap();
        assert!(read(dir.path(), 43, &s).is_err());
        delete(dir.path(), 42);
        assert!(!exists(dir.path(), 42));
    }

    #[test]
    fn reseal_retain_dan_purge() {
        let dir = tempfile::tempdir().unwrap();
        let (old, new) = (Sealer::random().unwrap(), Sealer::random().unwrap());
        for h in [1u64, 2, 3] {
            write(dir.path(), h, &old, &png(8, 8, false)).unwrap();
        }
        fs::write(dir.path().join("sisa.tmp"), b"x").unwrap();
        fs::remove_file(path_for(dir.path(), 3)).unwrap(); // hilang di disk -> gagal dipindahkan
        let failed = reseal_all(dir.path(), &[1, 2, 3], &old, &new);
        assert_eq!(failed, vec![3]);
        assert!(read(dir.path(), 1, &new).is_ok());
        assert!(read(dir.path(), 1, &old).is_err());

        let keep: HashSet<u64> = [1u64].into_iter().collect();
        assert!(retain_only(dir.path(), &keep) > 0);
        assert!(exists(dir.path(), 1));
        assert!(!exists(dir.path(), 2));
        assert!(!dir.path().join("sisa.tmp").exists());
        assert!(purge_all(dir.path()) > 0);
        assert!(!exists(dir.path(), 1));
        assert_eq!(purge_all(&dir.path().join("tidak-ada")), 0);
    }
}
