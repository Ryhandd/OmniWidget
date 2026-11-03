//! Anggaran render halaman PDF (murni aritmetika, tanpa dependensi -> mudah diuji).
//!
//! Masalah: bitmap RGBA = 4 byte/piksel. Poster 2xA0 pada 150 DPI sudah ~ 139 MP (556 MB), dan
//! `as_image()` membuat salinan kedua. PDF "poster/peta/CAD" atau PDF berbahaya bisa mendeklarasikan
//! halaman ribuan inci. Solusi: hitung ukuran piksel SEBELUM merender, turunkan DPI secara otomatis
//! agar muat di anggaran, dan tolak hanya bila hasilnya tak berguna (DPI efektif terlalu rendah).

/// DPI target (ketajaman baik untuk halaman ukuran dokumen biasa).
pub const TARGET_DPI: f32 = 150.0;
/// Anggaran piksel per halaman: 40 MP ~ 160 MB bitmap RGBA (+ salinan sementara saat encode).
pub const MAX_RENDER_PIXELS: u64 = 40_000_000;
/// Sisi terpanjang maksimum (batas aman encoder/dekoder gambar umum).
pub const MAX_SIDE_PX: u32 = 16_384;
/// Di bawah ini hasil render tidak lagi terbaca; lebih baik gagal dengan pesan jelas.
pub const MIN_DPI: f32 = 36.0;

const PT_PER_INCH: f32 = 72.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderPlan {
    pub dpi: f32,
    pub width_px: u32,
    pub height_px: u32,
    /// true bila DPI diturunkan dari `TARGET_DPI` demi anggaran memori.
    pub downscaled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BudgetError {
    /// Dimensi halaman bukan angka yang masuk akal (0, negatif, NaN, tak hingga).
    InvalidSize,
    /// Ukuran halaman (inci) terlalu besar: DPI efektif akan < `MIN_DPI`.
    TooLarge { width_in: f32, height_in: f32 },
}

/// Tentukan DPI & ukuran piksel untuk halaman berukuran `width_pt` x `height_pt` (satuan point, 1/72 inci).
pub fn plan_render(width_pt: f32, height_pt: f32) -> Result<RenderPlan, BudgetError> {
    if !width_pt.is_finite() || !height_pt.is_finite() || width_pt <= 0.0 || height_pt <= 0.0 {
        return Err(BudgetError::InvalidSize);
    }
    let (w_in, h_in) = (width_pt / PT_PER_INCH, height_pt / PT_PER_INCH);

    // Batas DPI dari anggaran piksel (luas) dan dari sisi terpanjang.
    let area_in = (w_in as f64) * (h_in as f64);
    let dpi_by_area = ((MAX_RENDER_PIXELS as f64) / area_in).sqrt() as f32;
    let dpi_by_side = MAX_SIDE_PX as f32 / w_in.max(h_in);
    let cap = dpi_by_area.min(dpi_by_side);

    let (dpi, downscaled) = if cap >= TARGET_DPI {
        (TARGET_DPI, false)
    } else {
        // Margin 0,1%: pembulatan ke atas piksel tidak boleh menembus anggaran.
        (cap * 0.999, true)
    };

    if dpi < MIN_DPI {
        return Err(BudgetError::TooLarge { width_in: w_in, height_in: h_in });
    }

    Ok(RenderPlan {
        dpi,
        width_px: (w_in * dpi).ceil().max(1.0) as u32,
        height_px: (h_in * dpi).ceil().max(1.0) as u32,
        downscaled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const A4: (f32, f32) = (595.0, 842.0);

    fn px(p: &RenderPlan) -> u64 {
        p.width_px as u64 * p.height_px as u64
    }

    #[test]
    fn halaman_normal_memakai_dpi_target() {
        let p = plan_render(A4.0, A4.1).unwrap();
        assert_eq!(p.dpi, TARGET_DPI);
        assert!(!p.downscaled);
        assert_eq!((p.width_px, p.height_px), (1240, 1755)); // A4 @150 DPI (1754,17 dibulatkan ke atas)
    }

    #[test]
    fn poster_besar_diturunkan_dpi_nya_dan_muat_anggaran() {
        // A0 (2384 x 3370 pt) pada 150 DPI ~ 35 MP: masih muat. Poster 2xA0 ~ 139 MP -> harus turun.
        assert!(!plan_render(2384.0, 3370.0).unwrap().downscaled);
        let p = plan_render(4768.0, 6740.0).unwrap();
        assert!(p.downscaled);
        assert!(p.dpi < TARGET_DPI && p.dpi >= MIN_DPI);
        assert!(px(&p) <= MAX_RENDER_PIXELS, "{} piksel melewati anggaran", px(&p));
    }

    #[test]
    fn halaman_tipis_panjang_dibatasi_sisi_terpanjang() {
        // 14400 x 100 pt (200 x 1,4 inci): luas kecil, tetapi 30000 px pada 150 DPI.
        let p = plan_render(14_400.0, 100.0).unwrap();
        assert!(p.downscaled);
        assert!(p.width_px.max(p.height_px) <= MAX_SIDE_PX);
    }

    #[test]
    fn halaman_raksasa_ditolak() {
        // 1 km x 1 km: DPI efektif jauh di bawah MIN_DPI.
        let r = plan_render(72.0 * 39_370.0, 72.0 * 39_370.0);
        assert!(matches!(r, Err(BudgetError::TooLarge { .. })));
    }

    #[test]
    fn batas_tepat_di_ambang_min_dpi() {
        // Halaman persegi yang tepat memberi ~MIN_DPI harus lolos; sedikit lebih besar harus ditolak.
        let side_in_ok = (MAX_RENDER_PIXELS as f64).sqrt() as f32 / (MIN_DPI * 1.01);
        assert!(plan_render(side_in_ok * 72.0, side_in_ok * 72.0).is_ok());
        let side_in_bad = (MAX_RENDER_PIXELS as f64).sqrt() as f32 / (MIN_DPI * 0.9);
        assert!(plan_render(side_in_bad * 72.0, side_in_bad * 72.0).is_err());
    }

    #[test]
    fn ukuran_tidak_valid_ditolak() {
        for (w, h) in [(0.0, 10.0), (10.0, 0.0), (-5.0, 10.0), (f32::NAN, 10.0), (10.0, f32::INFINITY)] {
            assert_eq!(plan_render(w, h), Err(BudgetError::InvalidSize), "({w}, {h})");
        }
    }

    #[test]
    fn halaman_sangat_kecil_tetap_minimal_1_piksel() {
        let p = plan_render(0.01, 0.01).unwrap();
        assert!(p.width_px >= 1 && p.height_px >= 1);
    }

    #[test]
    fn sapuan_ukuran_tidak_pernah_melewati_anggaran() {
        let mut s = 20.0_f32;
        while s < 200_000.0 {
            for ratio in [1.0_f32, 1.414, 4.0, 40.0] {
                if let Ok(p) = plan_render(s, s * ratio) {
                    assert!(px(&p) <= MAX_RENDER_PIXELS, "{s}x{} -> {} px", s * ratio, px(&p));
                    assert!(p.width_px.max(p.height_px) <= MAX_SIDE_PX + 1);
                }
            }
            s *= 1.37;
        }
    }
}
