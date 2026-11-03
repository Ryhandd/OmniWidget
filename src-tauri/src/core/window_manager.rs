//! Window manager: mode Pill/Expanded, snap top-center pada WORK AREA (taskbar/menu bar dikecualikan),
//! posisi bebas, dan resize berjangkar-tengah. Matematika murni ada di `geometry.rs` (teruji).
//! Mode ketiga, Deep Sleep, ada di `lifecycle.rs` (WebView dihancurkan).

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, State, WebviewWindow};

use crate::core::geometry::{self, Area, Settle};
use crate::core::lifecycle;
use crate::core::state::ShellState;

pub const TOP_MARGIN_DIP: f64 = 8.0;
pub const SNAP_RADIUS_DIP: f64 = 96.0;
const SETTLE_MS: u64 = 140;
const SNAP_ANIM_MS: u64 = 140;
const FRAME_MS: u64 = 16;

/// Pill (Dynamic-Island style): tinggi tetap, lebar MENGIKUTI isi (diukur di UI) agar tidak makan tempat.
pub const PILL_H_DIP: f64 = 36.0;
pub const PILL_W_MIN_DIP: u32 = 112;
pub const PILL_W_MAX_DIP: u32 = 340;
pub const EXPANDED_DIP: (f64, f64) = (420.0, 520.0);
/// Lebar pill terakhir yang dilaporkan UI (DIP). Bertahan selama proses hidup, termasuk Deep Sleep.
static PILL_W_DIP: AtomicU32 = AtomicU32::new(200);

/// true selama gerakan berasal dari kode kita sendiri (agar tidak memicu snap lagi).
static PROGRAMMATIC: AtomicBool = AtomicBool::new(false);
/// Generasi debounce event Moved.
static MOVE_GEN: AtomicU64 = AtomicU64::new(0);
/// Generasi animasi: animasi baru membatalkan yang lama (tanpa jitter).
static ANIM_GEN: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Pill,
    Expanded,
}

impl Mode {
    pub fn size_dip(self) -> (f64, f64) {
        match self {
            Mode::Pill => (PILL_W_DIP.load(Ordering::Relaxed) as f64, PILL_H_DIP),
            Mode::Expanded => EXPANDED_DIP,
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pill" => Some(Mode::Pill),
            "expanded" => Some(Mode::Expanded),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Pill => "pill",
            Mode::Expanded => "expanded",
        }
    }
}

/// Di mana jendela ditempatkan saat ditampilkan.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Selalu top-center (dipakai setelah bangun dari Deep Sleep dan saat reset).
    TopCenter,
    /// Pulihkan posisi bebas terakhir bila ada dan lock-snap mati; selain itu top-center.
    RestoreFree,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn monitor_area(m: &Monitor) -> Area {
    let wa = m.work_area();
    Area { x: wa.position.x, y: wa.position.y, w: wa.size.width as i32, h: wa.size.height as i32, scale: m.scale_factor() }
}

fn all_areas(app: &AppHandle) -> Vec<Area> {
    app.available_monitors().map(|v| v.iter().map(monitor_area).collect()).unwrap_or_default()
}

/// Work area monitor yang memuat titik tengah jendela (terdekat bila di celah antar monitor).
fn active_area(win: &WebviewWindow) -> Result<Area, String> {
    let pos = win.outer_position().map_err(err)?;
    let size = win.outer_size().map_err(err)?;
    let (cx, cy) = (pos.x + size.width as i32 / 2, pos.y + size.height as i32 / 2);
    if let Some(a) = geometry::nearest_area(&all_areas(win.app_handle()), cx, cy) {
        return Ok(a);
    }
    let m = win.primary_monitor().map_err(err)?.ok_or("Tidak ada monitor")?;
    Ok(monitor_area(&m))
}

/// Monitor tempat kursor berada (untuk hotkey/tray); None bila tak diketahui.
fn cursor_area(app: &AppHandle) -> Option<Area> {
    let c = app.cursor_position().ok()?;
    geometry::nearest_area(&all_areas(app), c.x as i32, c.y as i32)
}

fn release_programmatic_later() {
    // Event Moved dari gerakan kita tiba asinkron; beri jeda sebelum flag dilepas.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(250));
        PROGRAMMATIC.store(false, Ordering::Relaxed);
    });
}

fn animate_to(win: &WebviewWindow, tx: i32, ty: i32, ms: u64) -> Result<(), String> {
    let gen = ANIM_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    PROGRAMMATIC.store(true, Ordering::Relaxed);
    if ms == 0 {
        win.set_position(PhysicalPosition::new(tx, ty)).map_err(err)?;
        release_programmatic_later();
        return Ok(());
    }
    let from = win.outer_position().map_err(err)?;
    let steps = (ms / FRAME_MS).max(1);
    let win = win.clone();
    std::thread::spawn(move || {
        for i in 1..=steps {
            if ANIM_GEN.load(Ordering::SeqCst) != gen {
                return; // digantikan animasi lain
            }
            let e = geometry::ease_out_cubic(i as f64 / steps as f64);
            let x = from.x as f64 + (tx - from.x) as f64 * e;
            let y = from.y as f64 + (ty - from.y) as f64 * e;
            let _ = win.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
            std::thread::sleep(Duration::from_millis(FRAME_MS));
        }
        if ANIM_GEN.load(Ordering::SeqCst) == gen {
            let _ = win.set_position(PhysicalPosition::new(tx, ty));
            release_programmatic_later();
        }
    });
    Ok(())
}

pub fn snap_to_top_center(win: &WebviewWindow, anim_ms: u64) -> Result<(), String> {
    let area = active_area(win)?;
    let size = win.outer_size().map_err(err)?;
    let (tx, ty) = geometry::top_center(area, size.width as i32, TOP_MARGIN_DIP);
    win.state::<ShellState>().free_pos.lock().unwrap().take();
    animate_to(win, tx, ty, anim_ms)
}

/// Dipanggil dari WindowEvent::Moved; di-debounce, lalu snap atau simpan posisi bebas.
pub fn on_moved(win: &WebviewWindow) {
    if PROGRAMMATIC.load(Ordering::Relaxed) {
        return;
    }
    let gen = MOVE_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let win = win.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(SETTLE_MS));
        if MOVE_GEN.load(Ordering::SeqCst) != gen || PROGRAMMATIC.load(Ordering::Relaxed) {
            return;
        }
        let _ = settle_after_drag(&win);
    });
}

fn settle_after_drag(win: &WebviewWindow) -> Result<(), String> {
    let st = win.state::<ShellState>();
    st.touch();
    let area = active_area(win)?;
    let pos = win.outer_position().map_err(err)?;
    let size = win.outer_size().map_err(err)?;
    let wh = (size.width as i32, size.height as i32);

    match geometry::settle(area, (pos.x, pos.y), wh, st.lock_snap(), SNAP_RADIUS_DIP, TOP_MARGIN_DIP) {
        Settle::Snap(x, y) => {
            st.free_pos.lock().unwrap().take();
            if (pos.x - x).abs() > 1 || (pos.y - y).abs() > 1 {
                animate_to(win, x, y, SNAP_ANIM_MS)?;
            }
        }
        Settle::Free(x, y) => {
            *st.free_pos.lock().unwrap() = Some(geometry::to_relative(area, (x, y), wh));
            if (x, y) != (pos.x, pos.y) {
                animate_to(win, x, y, SNAP_ANIM_MS)?;
            }
        }
    }
    Ok(())
}

/// Resolusi/DPI berubah: bila tidak sedang di posisi bebas, kembali ke top-center.
pub fn on_display_changed(win: &WebviewWindow) {
    let st = win.state::<ShellState>();
    let free = st.free_pos.lock().unwrap().is_some() && !st.lock_snap();
    let _ = if free { place(win, None, Placement::RestoreFree) } else { snap_to_top_center(win, 0) };
}

/// Ganti ukuran dengan menjaga titik tengah horizontal dan tepi atas (lalu clamp ke work area).
pub fn set_mode(win: &WebviewWindow, mode: Mode) -> Result<(), String> {
    let area = active_area(win)?;
    let (dw, dh) = mode.size_dip();
    let (nw, nh) = ((dw * area.scale).round() as i32, (dh * area.scale).round() as i32);
    let pos = win.outer_position().map_err(err)?;
    let size = win.outer_size().map_err(err)?;
    if size.width as i32 == nw && size.height as i32 == nh {
        // Sudah berukuran benar (mis. hover berulang): cukup perbarui state, tanpa operasi jendela.
        let st = win.state::<ShellState>();
        *st.mode.lock().unwrap() = mode;
        st.touch();
        return Ok(());
    }

    PROGRAMMATIC.store(true, Ordering::Relaxed);
    win.set_size(PhysicalSize::new(nw as u32, nh as u32)).map_err(err)?;
    // OS/toolkit bisa menolak ukuran tertentu (mis. GTK memberi minimum): jangkar tengah dihitung dari
    // ukuran yang BENAR-BENAR dipakai, bukan yang diminta, supaya pill tetap tepat di tengah.
    // Di Linux/GTK resize asinkron: tunggu sebentar (maks ~60 ms) sampai ukuran baru terlihat.
    let old = (size.width as i32, size.height as i32);
    let mut actual = old;
    for _ in 0..12 {
        actual = win.outer_size().map(|s| (s.width as i32, s.height as i32)).unwrap_or((nw, nh));
        if actual != old || old == (nw, nh) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let (aw, ah) = actual;
    let nx = geometry::anchored_x(area, pos.x, size.width as i32, aw);
    let (nx, ny) = geometry::clamp_into(area, nx, pos.y, aw, ah);
    win.set_position(PhysicalPosition::new(nx, ny)).map_err(err)?;
    release_programmatic_later();

    let st = win.state::<ShellState>();
    *st.mode.lock().unwrap() = mode;
    st.touch();
    Ok(())
}

fn place(win: &WebviewWindow, area: Option<Area>, placement: Placement) -> Result<(), String> {
    let area = match area {
        Some(a) => a,
        None => active_area(win)?,
    };
    let st = win.state::<ShellState>();
    let size = win.outer_size().map_err(err)?;
    // Jendela yang belum pernah tampil bisa melaporkan ukuran 0 (Linux/GTK): pakai ukuran mode yang diketahui.
    let wh = if size.width == 0 || size.height == 0 {
        let (dw, dh) = st.mode.lock().unwrap().size_dip();
        ((dw * area.scale).round() as i32, (dh * area.scale).round() as i32)
    } else {
        (size.width as i32, size.height as i32)
    };
    let free = if placement == Placement::RestoreFree && !st.lock_snap() { *st.free_pos.lock().unwrap() } else { None };
    let (x, y) = match free {
        Some(rel) => geometry::from_relative(area, rel, wh),
        None => {
            st.free_pos.lock().unwrap().take();
            geometry::top_center(area, wh.0, TOP_MARGIN_DIP)
        }
    };
    animate_to(win, x, y, 0)
}

/// Tampilkan di monitor tempat kursor berada (hotkey/tray), lalu fokus.
pub fn show(win: &WebviewWindow, placement: Placement) {
    let _ = place(win, cursor_area(win.app_handle()), placement);
    let _ = win.show();
    let _ = win.set_focus();
    win.state::<ShellState>().touch();
}

pub fn toggle(app: &AppHandle) {
    match app.get_webview_window(lifecycle::MAIN) {
        None => lifecycle::wake(app), // Deep Sleep: bangun lalu Pill di top-center
        Some(w) => {
            if w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false) {
                let _ = w.hide();
            } else {
                show(&w, Placement::RestoreFree);
                // Dipanggil lewat hotkey/tray (keyboard): UI langsung membuka panel & menahannya sampai blur/Esc.
                let _ = w.emit("shell://summon", ());
            }
        }
    }
}

/// `width` (DIP, opsional) = lebar pill hasil ukur isi di UI; diabaikan untuk Expanded.
/// Instance kedua dijalankan (klik ikon lagi): munculkan widget, bangunkan bila Deep Sleep.
pub fn toggle_show(app: &AppHandle) {
    match app.get_webview_window(lifecycle::MAIN) {
        None => lifecycle::wake(app),
        Some(w) => show(&w, Placement::RestoreFree),
    }
}

/// async: berjalan di luar main thread, jadi menunggu resize (lihat `set_mode`) tidak menahan event loop.
#[tauri::command]
pub async fn window_set_mode(window: WebviewWindow, mode: String, width: Option<f64>) -> Result<(), String> {
    let m = Mode::parse(&mode).ok_or("Mode tidak dikenal")?;
    if let (Mode::Pill, Some(w)) = (m, width) {
        if w.is_finite() {
            PILL_W_DIP.store((w.ceil() as u32).clamp(PILL_W_MIN_DIP, PILL_W_MAX_DIP), Ordering::Relaxed);
        }
    }
    set_mode(&window, m)
}

/// Pengaman auto-collapse: dipanggil SEKALI oleh UI bila event pointerleave mungkin terlewat
/// (kursor keluar sangat cepat di tepi jendela). Bukan polling.
#[tauri::command]
pub fn window_cursor_inside(window: WebviewWindow) -> bool {
    let (Ok(c), Ok(p), Ok(s)) = (window.cursor_position(), window.outer_position(), window.outer_size()) else {
        return true; // tidak tahu -> jangan menutup paksa
    };
    let (x, y) = (c.x as i32, c.y as i32);
    x >= p.x && y >= p.y && x < p.x + s.width as i32 && y < p.y + s.height as i32
}

#[tauri::command]
pub fn window_snap(window: WebviewWindow) -> Result<(), String> {
    snap_to_top_center(&window, SNAP_ANIM_MS)
}

#[tauri::command]
pub fn window_set_lock_snap(window: WebviewWindow, state: State<'_, ShellState>, locked: bool) {
    state.lock_snap.store(locked, Ordering::Relaxed);
    if locked {
        let _ = snap_to_top_center(&window, SNAP_ANIM_MS); // langsung patuhi lock
    }
}
