//! Geometri murni (tanpa Tauri) untuk snapping, clamp, dan resize berjangkar-tengah.
//! Semua nilai dalam piksel fisik; konstanta DIP dikalikan `Area::scale` monitor tujuan.
//! Dipisah agar bisa diuji dengan `cargo test` tanpa membuka jendela.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub scale: f64,
}

/// Hasil keputusan setelah drag selesai.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Settle {
    /// Dalam radius magnet (atau lock aktif): tempel ke top-center. Posisi bebas dihapus.
    Snap(i32, i32),
    /// Di luar radius: posisi bebas, sudah di-clamp ke dalam area. Simpan sebagai rasio.
    Free(i32, i32),
}

/// Titik snap default: tengah horizontal work area, `margin_dip` dari tepi atas.
pub fn top_center(area: Area, win_w: i32, margin_dip: f64) -> (i32, i32) {
    let x = area.x + (area.w - win_w) / 2;
    let y = area.y + (margin_dip * area.scale).round() as i32;
    (x, y)
}

/// Jaga jendela tetap di dalam area. Jika jendela lebih besar dari area, tempel ke sisi kiri/atas.
pub fn clamp_into(area: Area, x: i32, y: i32, w: i32, h: i32) -> (i32, i32) {
    let max_x = (area.x + area.w - w).max(area.x);
    let max_y = (area.y + area.h - h).max(area.y);
    (x.clamp(area.x, max_x), y.clamp(area.y, max_y))
}

/// Posisi X baru saat lebar berubah, mempertahankan titik tengah horizontal lalu di-clamp.
pub fn anchored_x(area: Area, old_x: i32, old_w: i32, new_w: i32) -> i32 {
    let x = old_x + (old_w - new_w) / 2;
    x.clamp(area.x, (area.x + area.w - new_w).max(area.x))
}

/// Putuskan snap vs posisi bebas setelah drag selesai.
pub fn settle(
    area: Area,
    pos: (i32, i32),
    size: (i32, i32),
    lock_snap: bool,
    radius_dip: f64,
    margin_dip: f64,
) -> Settle {
    let (tx, ty) = top_center(area, size.0, margin_dip);
    let dist = (((pos.0 - tx) as f64).powi(2) + ((pos.1 - ty) as f64).powi(2)).sqrt();
    if lock_snap || dist <= radius_dip * area.scale {
        Settle::Snap(tx, ty)
    } else {
        let (cx, cy) = clamp_into(area, pos.0, pos.1, size.0, size.1);
        Settle::Free(cx, cy)
    }
}

/// Posisi → rasio 0..1 dari ruang gerak (area dikurangi ukuran jendela). Tahan terhadap perubahan resolusi.
pub fn to_relative(area: Area, pos: (i32, i32), size: (i32, i32)) -> (f64, f64) {
    let span_x = (area.w - size.0).max(1) as f64;
    let span_y = (area.h - size.1).max(1) as f64;
    (
        ((pos.0 - area.x) as f64 / span_x).clamp(0.0, 1.0),
        ((pos.1 - area.y) as f64 / span_y).clamp(0.0, 1.0),
    )
}

pub fn from_relative(area: Area, rel: (f64, f64), size: (i32, i32)) -> (i32, i32) {
    let span_x = (area.w - size.0).max(0) as f64;
    let span_y = (area.h - size.1).max(0) as f64;
    (area.x + (rel.0 * span_x).round() as i32, area.y + (rel.1 * span_y).round() as i32)
}

/// Area yang memuat titik (cx, cy); bila di celah antar monitor, yang terdekat.
pub fn nearest_area(areas: &[Area], cx: i32, cy: i32) -> Option<Area> {
    areas
        .iter()
        .map(|a| {
            let dx = (a.x - cx).max(0).max(cx - (a.x + a.w)) as i64;
            let dy = (a.y - cy).max(0).max(cy - (a.y + a.h)) as i64;
            (dx * dx + dy * dy, *a)
        })
        .min_by_key(|(d, _)| *d)
        .map(|(_, a)| a)
}

pub fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Layar 1920x1080 dengan taskbar vertikal 48px di kiri (seperti pada screenshot pengguna).
    const WORK_LEFT_BAR: Area = Area { x: 48, y: 0, w: 1872, h: 1080, scale: 1.0 };
    const FULL: Area = Area { x: 0, y: 0, w: 1920, h: 1080, scale: 1.0 };

    #[test]
    fn top_center_uses_work_area_not_full_screen() {
        let (x_work, y) = top_center(WORK_LEFT_BAR, 280, 8.0);
        let (x_full, _) = top_center(FULL, 280, 8.0);
        assert_eq!(x_work, 48 + (1872 - 280) / 2); // 844
        assert_eq!(x_full, 820);
        assert_ne!(x_work, x_full);
        assert_eq!(y, 8);
    }

    #[test]
    fn margin_scales_with_dpi() {
        let a = Area { scale: 1.5, ..FULL };
        assert_eq!(top_center(a, 280, 8.0).1, 12);
    }

    #[test]
    fn settle_snaps_inside_radius_and_frees_outside() {
        let (tx, ty) = top_center(FULL, 280, 8.0);
        assert_eq!(settle(FULL, (tx + 60, ty + 40), (280, 36), false, 96.0, 8.0), Settle::Snap(tx, ty));
        assert_eq!(settle(FULL, (tx + 400, ty + 300), (280, 36), false, 96.0, 8.0), Settle::Free(tx + 400, ty + 300));
    }

    #[test]
    fn lock_always_snaps() {
        let (tx, ty) = top_center(FULL, 280, 8.0);
        assert_eq!(settle(FULL, (5, 900), (280, 36), true, 96.0, 8.0), Settle::Snap(tx, ty));
    }

    #[test]
    fn free_position_is_clamped_inside_area() {
        match settle(FULL, (-500, 5000), (280, 36), false, 96.0, 8.0) {
            Settle::Free(x, y) => assert_eq!((x, y), (0, 1080 - 36)),
            other => panic!("harus Free, dapat {:?}", other),
        }
    }

    #[test]
    fn anchored_resize_keeps_center() {
        // Pill 280 -> Expanded 420 di top-center: tengah tetap, X bergeser setengah selisih.
        let (px, _) = top_center(FULL, 280, 8.0);
        let nx = anchored_x(FULL, px, 280, 420);
        assert_eq!(nx + 420 / 2, px + 280 / 2);
        assert_eq!(nx, (1920 - 420) / 2);
    }

    #[test]
    fn anchored_resize_clamps_at_edges() {
        assert_eq!(anchored_x(FULL, 0, 280, 420), 0);
        assert_eq!(anchored_x(FULL, 1920 - 280, 280, 420), 1920 - 420);
    }

    #[test]
    fn relative_roundtrip_survives_resolution_change() {
        let rel = to_relative(FULL, (1000, 400), (280, 36));
        let small = Area { x: 0, y: 0, w: 1280, h: 720, scale: 1.0 };
        let (x, y) = from_relative(small, rel, (280, 36));
        assert!(x >= 0 && x <= 1280 - 280 && y >= 0 && y <= 720 - 36);
        let back = from_relative(FULL, rel, (280, 36));
        assert_eq!(back, (1000, 400));
    }

    #[test]
    fn nearest_area_picks_containing_then_closest() {
        let right = Area { x: 1920, y: 0, w: 1920, h: 1080, scale: 1.0 };
        let all = [FULL, right];
        assert_eq!(nearest_area(&all, 100, 100), Some(FULL));
        assert_eq!(nearest_area(&all, 2000, 100), Some(right));
        assert_eq!(nearest_area(&all, 1919, 2000), Some(FULL)); // di luar bawah: terdekat
        assert_eq!(nearest_area(&[], 0, 0), None);
    }

    #[test]
    fn easing_endpoints() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert!(ease_out_cubic(0.5) > 0.5);
    }
}
