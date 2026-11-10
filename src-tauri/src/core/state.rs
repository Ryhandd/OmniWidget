//! State shell bersama (sumber kebenaran di Rust). WebView hanyalah view yang boleh dibuang:
//! mode, posisi bebas, dan pengaturan sleep tetap hidup di sini saat Deep Sleep.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread::Thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::core::window_manager::Mode;

/// Default: Deep Sleep otomatis nonaktif (0 = nonaktif) agar widget tidak hilang sendiri.
/// Pengguna tetap bisa mengaktifkannya lewat Pengaturan, dan tidur manual tetap tersedia.
pub const DEFAULT_SLEEP_SECS: u64 = 0;

pub struct ShellState {
    pub mode: Mutex<Mode>,
    /// Posisi bebas sebagai rasio 0..1 dari ruang gerak work area; None = top-center.
    pub free_pos: Mutex<Option<(f64, f64)>>,
    pub lock_snap: AtomicBool,
    pub sleep_after_secs: AtomicU64,
    pub asleep: AtomicBool,
    /// Jumlah pekerjaan latar yang membutuhkan WebView tetap hidup (mis. konversi). > 0 = jangan tidur.
    holds: AtomicUsize,
    last_activity: Mutex<Instant>,
    watcher: Mutex<Option<Thread>>,
    /// Mode tampilan pill saat ciut. Default = hover-only (cuma jam).
    island_open_behavior: Mutex<String>,
}

impl ShellState {
    pub fn new() -> Self {
        Self {
            mode: Mutex::new(Mode::Pill),
            free_pos: Mutex::new(None),
            lock_snap: AtomicBool::new(false),
            sleep_after_secs: AtomicU64::new(DEFAULT_SLEEP_SECS),
            asleep: AtomicBool::new(false),
            holds: AtomicUsize::new(0),
            last_activity: Mutex::new(Instant::now()),
            watcher: Mutex::new(None),
            island_open_behavior: Mutex::new(Self::ISLAND_HOVER_ONLY.to_string()),
        }
    }

    pub fn touch(&self) {
        *self.last_activity.lock().unwrap() = Instant::now();
    }

    pub fn idle_for(&self) -> Duration {
        self.last_activity.lock().unwrap().elapsed()
    }

    pub fn set_watcher(&self, t: Thread) {
        *self.watcher.lock().unwrap() = Some(t);
    }

    /// Bangunkan thread idle-watcher agar menghitung ulang tenggat (setelah wake / ubah pengaturan).
    pub fn wake_watcher(&self) {
        if let Some(t) = self.watcher.lock().unwrap().as_ref() {
            t.unpark();
        }
    }

    /// Ada pekerjaan latar yang menahan Deep Sleep?
    pub fn is_busy(&self) -> bool {
        self.holds.load(Ordering::SeqCst) > 0
    }

    pub fn acquire_hold(&self) {
        self.holds.fetch_add(1, Ordering::SeqCst);
    }

    /// Lepas satu tahanan. Saat yang terakhir lepas, hitung mundur idle dimulai ulang dari SEKARANG
    /// (bukan dari interaksi terakhir yang bisa sudah puluhan menit lalu), lalu watcher dibangunkan.
    pub fn release_hold(&self) {
        let prev = self.holds.fetch_sub(1, Ordering::SeqCst);
        debug_assert!(prev > 0, "release_hold tanpa acquire_hold");
        if prev == 1 {
            self.touch();
            self.wake_watcher();
        }
    }

    pub fn lock_snap(&self) -> bool {
        self.lock_snap.load(Ordering::Relaxed)
    }

    pub fn island_behavior(&self) -> String {
        self.island_open_behavior.lock().unwrap().clone()
    }

    pub fn set_island_behavior(&self, v: &str) {
        *self.island_open_behavior.lock().unwrap() = v.to_string();
        self.touch();
    }

    /// Ambang perilaku pill saat widget ciut. `island_auto_open` = island langsung mekar semua
    /// modul kapan saja tanpa perlu hover. `island_hover_only` (default) = cuma jam terlihat
    /// sampai kursor ditaruh ke atas widget, baru island modul muncul.
    pub const ISLAND_AUTO_OPEN: &str = "island_auto_open";
    pub const ISLAND_HOVER_ONLY: &str = "island_hover_only";
}

/// Tahanan Deep Sleep berbasis RAII: selama objek ini hidup, widget tidak akan menghancurkan WebView.
/// Dilepas otomatis saat di-drop (selesai, error, dibatalkan, atau task di-abort) sehingga tidak ada
/// jalur yang lupa melepas. Dapat dipakai modul lain (unduhan, dsb.).
pub struct AwakeGuard {
    app: AppHandle,
}

impl AwakeGuard {
    pub fn acquire(app: &AppHandle) -> Self {
        app.state::<ShellState>().acquire_hold();
        Self { app: app.clone() }
    }
}

impl Drop for AwakeGuard {
    fn drop(&mut self) {
        // try_state: aman bila proses sedang keluar dan state sudah dilepas.
        if let Some(st) = self.app.try_state::<ShellState>() {
            st.release_hold();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn hold_dihitung_dan_idle_diulang_saat_yang_terakhir_lepas() {
        let st = ShellState::new();
        assert!(!st.is_busy());

        st.acquire_hold();
        st.acquire_hold();
        assert!(st.is_busy());

        // Simulasikan konversi panjang: tidak ada interaksi selama beberapa saat.
        std::thread::sleep(Duration::from_millis(60));
        assert!(st.idle_for() >= Duration::from_millis(60));

        st.release_hold();
        assert!(st.is_busy(), "masih ada satu tahanan");
        assert!(st.idle_for() >= Duration::from_millis(60), "belum direset");

        st.release_hold();
        assert!(!st.is_busy());
        assert!(st.idle_for() < Duration::from_millis(50), "idle harus mulai dari nol setelah selesai");
    }
}
