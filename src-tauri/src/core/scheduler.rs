//! Scheduler tunggal (TDD 4.3/4.4): SATU thread yang tidur sampai tenggat terdekat dari semua sumber
//! (reminder to-do, akhir fase Pomodoro). Perubahan data memanggil `poke()` untuk menghitung ulang.
//!
//! Tidur dibatasi `MAX_PARK` agar tenggat yang terlewat saat laptop sleep (jam monoton bisa berhenti)
//! tetap terdeteksi segera setelah resume: waktu dihitung ulang dari jam dinding (epoch).

use std::sync::{Arc, Mutex};
use std::thread::Thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};

const MAX_PARK: Duration = Duration::from_secs(60);

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Sumber tenggat. `next_due` murah (biasanya satu query/field); `fire` dipanggil bila `next_due <= now`.
pub trait Timed: Send + Sync {
    fn next_due(&self, app: &AppHandle) -> Option<i64>;
    fn fire(&self, app: &AppHandle, now: i64);
}

pub struct Scheduler {
    sources: Mutex<Vec<Arc<dyn Timed>>>,
    thread: Mutex<Option<Thread>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self { sources: Mutex::new(Vec::new()), thread: Mutex::new(None) }
    }

    pub fn register(&self, s: Arc<dyn Timed>) {
        self.sources.lock().unwrap().push(s);
        self.poke();
    }

    /// Data berubah: bangunkan thread agar menghitung ulang tenggat terdekat.
    pub fn poke(&self) {
        if let Some(t) = self.thread.lock().unwrap().as_ref() {
            t.unpark();
        }
    }
}

/// Helper: `poke` dari mana saja yang memegang AppHandle.
pub fn poke(app: &AppHandle) {
    if let Some(s) = app.try_state::<Scheduler>() {
        s.poke();
    }
}

pub fn start(app: &AppHandle) {
    let a = app.clone();
    let handle = std::thread::Builder::new()
        .name("scheduler".into())
        .spawn(move || loop {
            let sources: Vec<Arc<dyn Timed>> = a.state::<Scheduler>().sources.lock().unwrap().clone();
            let now = now_ms();
            let mut next: Option<i64> = None;
            for s in &sources {
                match s.next_due(&a) {
                    Some(due) if due <= now => s.fire(&a, now),
                    Some(due) => next = Some(next.map_or(due, |n| n.min(due))),
                    None => {}
                }
            }
            // Setelah fire, sumber mungkin punya tenggat baru yang sudah lewat: hitung ulang segera.
            let now2 = now_ms();
            if sources.iter().any(|s| s.next_due(&a).is_some_and(|d| d <= now2)) {
                continue;
            }
            let wait = match next {
                Some(due) => Duration::from_millis((due - now2).max(0) as u64).min(MAX_PARK),
                None => MAX_PARK,
            };
            std::thread::park_timeout(wait);
        })
        .expect("gagal menjalankan scheduler");
    *app.state::<Scheduler>().thread.lock().unwrap() = Some(handle.thread().clone());
}
