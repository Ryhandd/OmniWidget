//! Mode ke-3: Deep Sleep. WebView dihancurkan total; yang tersisa hanya core Rust + tray + hotkey.
//! Bangun kembali = membangun ulang jendela dari tauri.conf.json (Pill, top-center).
//!
//! Idle watcher memakai SATU thread yang `park_timeout` tepat sampai tenggat (tanpa polling berkala).

use std::sync::atomic::Ordering;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager, State, WebviewWindow, WebviewWindowBuilder};

use crate::core::state::ShellState;
use crate::core::window_manager::{self, Mode, Placement};
use crate::core::tray;

pub const MAIN: &str = "main";
/// Saat Expanded sedang terlihat, jangan tidur; cek ulang berkala (hanya bila sudah lewat tenggat).
const RECHECK: Duration = Duration::from_secs(30);

fn build_main(app: &AppHandle) -> Result<WebviewWindow, String> {
    let cfg = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN)
        .cloned()
        .ok_or("Konfigurasi jendela 'main' tidak ada")?;
    // Tersembunyi dulu: posisi diatur sebelum tampil agar tidak berkedip di tengah layar.
    WebviewWindowBuilder::from_config(app, &cfg)
        .map_err(|e| e.to_string())?
        .visible(false)
        .build()
        .map_err(|e| e.to_string())
}

pub fn is_asleep(app: &AppHandle) -> bool {
    app.state::<ShellState>().asleep.load(Ordering::SeqCst)
}

/// Hancurkan WebView. State (tema di localStorage, free_pos, pengaturan) tetap terjaga.
///
/// Mengembalikan `false` bila DITOLAK karena ada pekerjaan latar (mis. konversi) yang masih butuh
/// WebView; `true` bila sudah/akan tidur. Pemeriksaan ada di sini (bukan hanya di idle watcher) agar
/// SEMUA jalur (idle otomatis, tray, tombol "Tidur sekarang") tunduk pada aturan yang sama.
pub fn enter_deep_sleep(app: &AppHandle) -> bool {
    let st = app.state::<ShellState>();
    if st.is_busy() {
        return false;
    }
    if st.asleep.swap(true, Ordering::SeqCst) {
        return true;
    }
    // Cek ulang setelah menandai tidur: menutup celah bila job baru terdaftar di antara dua baris di atas.
    if st.is_busy() {
        st.asleep.store(false, Ordering::SeqCst);
        return false;
    }
    *st.mode.lock().unwrap() = Mode::Pill;
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.destroy();
    }
    // Tanpa WebView tak ada yang menampilkan thumbnail: lepas dari RAM (dimuat ulang dari disk bila perlu).
    crate::plugins::clipboard::release_thumbnails(app);
    tray::set_sleeping(app, true);
    true
}

/// Bangun/tampilkan jendela. Dijalankan di luar main thread agar pembuatan window tidak deadlock di Windows.
pub fn wake(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<ShellState>();
        let win = match app.get_webview_window(MAIN) {
            Some(w) => w,
            None => match build_main(&app) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("[omniwidget] gagal membangun jendela: {e}");
                    return;
                }
            },
        };
        let was_asleep = st.asleep.swap(false, Ordering::SeqCst);
        if was_asleep {
            *st.mode.lock().unwrap() = Mode::Pill;
            st.free_pos.lock().unwrap().take();
            tray::set_sleeping(&app, false);
        }
        window_manager::show(&win, if was_asleep { Placement::TopCenter } else { Placement::RestoreFree });
        st.wake_watcher();
    });
}

/// Eligible tidur: Pill, atau jendela sedang disembunyikan. Expanded yang terlihat tidak pernah tidur.
fn eligible(app: &AppHandle) -> bool {
    let st = app.state::<ShellState>();
    let pill = *st.mode.lock().unwrap() == Mode::Pill;
    let visible = app.get_webview_window(MAIN).and_then(|w| w.is_visible().ok()).unwrap_or(false);
    pill || !visible
}

pub fn spawn_idle_watcher(app: AppHandle) {
    let a = app.clone();
    let handle = std::thread::Builder::new()
        .name("idle-watcher".into())
        .spawn(move || loop {
            let st = a.state::<ShellState>();
            let limit = st.sleep_after_secs.load(Ordering::Relaxed);
            if limit == 0 || st.asleep.load(Ordering::SeqCst) {
                std::thread::park(); // tidak ada yang ditunggu; dibangunkan oleh wake()/pengaturan
                continue;
            }
            if st.is_busy() {
                // Konversi/job latar berjalan: tidak ada tenggat. Tanpa polling; `release_hold` yang
                // terakhir membangunkan thread ini dan menghitung ulang dari waktu selesai.
                std::thread::park();
                continue;
            }
            let remaining = Duration::from_secs(limit).saturating_sub(st.idle_for());
            if remaining.is_zero() {
                if eligible(&a) {
                    enter_deep_sleep(&a);
                } else {
                    std::thread::park_timeout(RECHECK);
                }
            } else {
                std::thread::park_timeout(remaining);
            }
        })
        .expect("gagal menjalankan idle-watcher");
    app.state::<ShellState>().set_watcher(handle.thread().clone());
}

// ---------------------------------------------------------------- Commands

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellSnapshot {
    mode: &'static str,
    lock_snap: bool,
    sleep_after_min: u64,
}

/// Ping aktivitas dari UI (sudah di-throttle di frontend). Hanya mengubah timestamp.
#[tauri::command]
pub fn shell_activity(state: State<'_, ShellState>) {
    state.touch();
}

#[tauri::command]
pub fn shell_get_state(state: State<'_, ShellState>) -> ShellSnapshot {
    ShellSnapshot {
        mode: state.mode.lock().unwrap().as_str(),
        lock_snap: state.lock_snap(),
        sleep_after_min: state.sleep_after_secs.load(Ordering::Relaxed) / 60,
    }
}

/// minutes = 0 menonaktifkan Deep Sleep otomatis.
#[tauri::command]
pub fn shell_set_sleep_after(state: State<'_, ShellState>, minutes: u64) {
    state.sleep_after_secs.store(minutes.min(24 * 60) * 60, Ordering::Relaxed);
    state.touch();
    state.wake_watcher();
}

/// Tidur manual dari UI. Mengembalikan `false` (dan tidak melakukan apa-apa) bila ada konversi/job
/// yang sedang berjalan, supaya UI dapat memberi tahu pengguna.
/// Eksekusi ditunda ke thread lain: jendela tidak boleh menghancurkan dirinya di tengah IPC.
#[tauri::command]
pub fn shell_deep_sleep(app: AppHandle) -> bool {
    if app.state::<ShellState>().is_busy() {
        return false;
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        enter_deep_sleep(&app);
    });
    true
}
