//! Timer Pomodoro (TDD 4.4). Timer hidup di Rust (bukan JS yang bisa di-throttle saat WebView tersembunyi):
//! hanya `ends_at` (epoch) yang disimpan, sisa waktu = ends_at - now, jadi tidak ada drift dan pulih
//! setelah restart. Akhir fase dijadwalkan lewat scheduler bersama (satu timer untuk semua fitur).
//!
//!   idle --start--> focus --selesai--> short --selesai--> focus ... (tiap `long_every` fokus -> long)

use std::sync::{Arc, Mutex};

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;

use crate::core::scheduler::{self, now_ms, Scheduler, Timed};
use crate::core::storage::Storage;

pub const EVENT: &str = "pomodoro://changed";
const STATE_KEY: &str = "pomodoro.state";

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS pomodoro_session(
        id         INTEGER PRIMARY KEY,
        kind       TEXT NOT NULL,
        started_at INTEGER NOT NULL,
        ended_at   INTEGER NOT NULL,
        completed  INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_pomo_started ON pomodoro_session(started_at);
";

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    #[default]
    Idle,
    Focus,
    Short,
    Long,
}

impl Phase {
    fn as_str(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Focus => "focus",
            Phase::Short => "short",
            Phase::Long => "long",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub focus_min: u32,
    pub short_min: u32,
    pub long_min: u32,
    pub long_every: u32,
    pub auto_start: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { focus_min: 25, short_min: 5, long_min: 15, long_every: 4, auto_start: false }
    }
}

impl Config {
    fn clamped(self) -> Self {
        Self {
            focus_min: self.focus_min.clamp(1, 180),
            short_min: self.short_min.clamp(1, 60),
            long_min: self.long_min.clamp(1, 120),
            long_every: self.long_every.clamp(1, 12),
            auto_start: self.auto_start,
        }
    }
    fn duration_ms(&self, p: Phase) -> i64 {
        60_000
            * match p {
                Phase::Idle => 0,
                Phase::Focus => self.focus_min,
                Phase::Short => self.short_min,
                Phase::Long => self.long_min,
            } as i64
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Pomo {
    pub phase: Phase,
    pub running: bool,
    /// Saat `running`: kapan fase berakhir (epoch ms).
    pub ends_at: Option<i64>,
    /// Saat tidak `running` (jeda / menunggu fase berikut): sisa waktu.
    pub remaining_ms: i64,
    /// Kapan fase sekarang dimulai (untuk log sesi).
    pub started_at: Option<i64>,
    /// Jumlah fokus selesai sejak long break terakhir.
    pub cycle: u32,
    pub config: Config,
}

impl Pomo {
    /// Fase berikut setelah `self.phase` selesai (cycle sudah diperbarui oleh pemanggil).
    fn next_after(&self, ended: Phase, completed: bool) -> Phase {
        match ended {
            Phase::Focus if completed && self.cycle > 0 && self.cycle % self.config.long_every == 0 => Phase::Long,
            Phase::Focus => Phase::Short,
            _ => Phase::Focus,
        }
    }

    fn enter(&mut self, p: Phase, run: bool, now: i64) {
        self.phase = p;
        let d = self.config.duration_ms(p);
        if p == Phase::Idle {
            *self = Pomo { config: self.config, ..Default::default() };
        } else if run {
            self.running = true;
            self.ends_at = Some(now + d);
            self.remaining_ms = d;
            self.started_at = Some(now);
        } else {
            self.running = false;
            self.ends_at = None;
            self.remaining_ms = d;
            self.started_at = None;
        }
    }

    /// Akhiri fase sekarang. `completed` = selesai alami (bukan skip). Mengembalikan sesi untuk dicatat.
    fn finish(&mut self, now: i64, completed: bool) -> Option<(Phase, i64, i64, bool)> {
        let p = self.phase;
        if p == Phase::Idle {
            return None;
        }
        let log = self.started_at.map(|s| (p, s, now, completed));
        if p == Phase::Focus && completed {
            self.cycle += 1;
        }
        let next = self.next_after(p, completed);
        if p == Phase::Long {
            // Long break menutup satu set.
            self.cycle = 0;
        }
        let run = self.running && (!completed || self.config.auto_start);
        self.enter(next, run, now);
        log
    }
}

pub struct PomoState {
    inner: Mutex<Pomo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    #[serde(flatten)]
    pomo: Pomo,
    now: i64,
    today_focus: i64,
    today_minutes: i64,
}

fn save(app: &AppHandle, p: &Pomo) {
    if let (Some(s), Ok(json)) = (app.try_state::<Storage>(), serde_json::to_string(p)) {
        let _ = s.set(STATE_KEY, &json);
    }
}

fn log_session(app: &AppHandle, (kind, start, end, completed): (Phase, i64, i64, bool)) {
    if let Some(s) = app.try_state::<Storage>() {
        let _ = s.with_conn(|c| {
            c.execute(
                "INSERT INTO pomodoro_session(kind, started_at, ended_at, completed) VALUES(?1, ?2, ?3, ?4)",
                params![kind.as_str(), start, end, completed as i64],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
        });
    }
}

/// Fokus selesai hari ini (lokal ditentukan UI lewat `since`; default 24 jam terakhir).
fn today(app: &AppHandle, since: i64) -> (i64, i64) {
    app.try_state::<Storage>()
        .and_then(|s| {
            s.with_conn(|c| {
                c.query_row(
                    "SELECT COUNT(*), COALESCE(SUM(ended_at - started_at), 0) / 60000 FROM pomodoro_session
                     WHERE kind = 'focus' AND completed = 1 AND started_at >= ?1",
                    params![since],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(|e| e.to_string())
            })
            .ok()
        })
        .unwrap_or((0, 0))
}

fn snapshot(app: &AppHandle, p: Pomo, since: Option<i64>) -> Snapshot {
    let now = now_ms();
    let (today_focus, today_minutes) = today(app, since.unwrap_or(now - 86_400_000));
    Snapshot { pomo: p, now, today_focus, today_minutes }
}

/// Simpan, beri tahu UI, dan minta scheduler menghitung ulang.
fn commit(app: &AppHandle, p: Pomo) {
    save(app, &p);
    let _ = app.emit(EVENT, ());
    scheduler::poke(app);
}

fn mutate(app: &AppHandle, f: impl FnOnce(&mut Pomo, i64) -> Option<(Phase, i64, i64, bool)>) -> Pomo {
    let st = app.state::<PomoState>();
    let mut g = st.inner.lock().unwrap();
    let log = f(&mut g, now_ms());
    let p = *g;
    drop(g);
    if let Some(l) = log {
        log_session(app, l);
    }
    commit(app, p);
    p
}

struct PhaseEnd;

impl Timed for PhaseEnd {
    fn next_due(&self, app: &AppHandle) -> Option<i64> {
        let p = app.try_state::<PomoState>()?.inner.lock().ok()?.clone();
        if p.running { p.ends_at } else { None }
    }

    fn fire(&self, app: &AppHandle, now: i64) {
        let done = {
            let st = app.state::<PomoState>();
            let mut g = st.inner.lock().unwrap();
            if !g.running || g.ends_at.is_none_or(|e| e > now) {
                return;
            }
            let ended = g.phase;
            let log = g.finish(now, true);
            (ended, *g, log)
        };
        let (ended, p, log) = done;
        if let Some(l) = log {
            log_session(app, l);
        }
        let mins = |ms: i64| ms / 60_000;
        let (title, body) = match ended {
            Phase::Focus => (
                "Fokus selesai 🎉".to_string(),
                format!(
                    "Waktunya istirahat {} menit{}",
                    mins(p.config.duration_ms(p.phase)),
                    if p.running { "" } else { " — tekan Mulai." }
                ),
            ),
            _ => ("Istirahat selesai".to_string(), format!("Siap fokus {} menit lagi?", p.config.focus_min)),
        };
        let _ = app.notification().builder().title(title).body(body).show();
        commit(app, p);
    }
}

pub fn setup(app: &AppHandle) -> Result<(), String> {
    let storage = app.state::<Storage>();
    storage.with_conn(|c| c.execute_batch(SCHEMA).map_err(|e| e.to_string()))?;
    let mut p: Pomo = storage.get(STATE_KEY).ok().flatten().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    p.config = p.config.clamped();
    app.manage(PomoState { inner: Mutex::new(p) });
    // Fase yang berakhir saat aplikasi mati akan langsung ditembakkan oleh scheduler (ends_at <= now).
    app.state::<Scheduler>().register(Arc::new(PhaseEnd));
    Ok(())
}

// ---------------------------------------------------------------- Commands

/// `since` = awal hari lokal (epoch ms) dari UI, untuk statistik "hari ini".
#[tauri::command]
pub fn pomo_get(app: AppHandle, state: State<'_, PomoState>, since: Option<i64>) -> Snapshot {
    let p = *state.inner.lock().unwrap();
    snapshot(&app, p, since)
}

#[tauri::command]
pub fn pomo_start(app: AppHandle) -> Pomo {
    mutate(&app, |p, now| {
        match p.phase {
            Phase::Idle => p.enter(Phase::Focus, true, now),
            _ if !p.running => {
                p.running = true;
                p.ends_at = Some(now + p.remaining_ms.max(1000));
                p.started_at.get_or_insert(now);
            }
            _ => {}
        }
        None
    })
}

#[tauri::command]
pub fn pomo_pause(app: AppHandle) -> Pomo {
    mutate(&app, |p, now| {
        if p.running {
            p.remaining_ms = p.ends_at.map_or(0, |e| (e - now).max(0));
            p.running = false;
            p.ends_at = None;
        }
        None
    })
}

/// Lewati fase sekarang (tidak dihitung selesai) dan lanjut ke fase berikut.
#[tauri::command]
pub fn pomo_skip(app: AppHandle) -> Pomo {
    mutate(&app, |p, now| {
        // Skip saat berjalan -> fase berikut langsung berjalan; saat jeda -> menunggu.
        p.finish(now, false)
    })
}

#[tauri::command]
pub fn pomo_reset(app: AppHandle) -> Pomo {
    mutate(&app, |p, now| {
        let log = if p.running { p.started_at.map(|s| (p.phase, s, now, false)) } else { None };
        p.enter(Phase::Idle, false, now);
        log
    })
}

#[tauri::command]
pub fn pomo_set_config(app: AppHandle, config: Config) -> Pomo {
    mutate(&app, |p, _now| {
        let old = p.config;
        p.config = config.clamped();
        // Fase yang sedang menunggu (belum jalan) menyesuaikan durasi baru; yang sedang jalan dibiarkan.
        if !p.running && p.phase != Phase::Idle && p.remaining_ms == old.duration_ms(p.phase) {
            p.remaining_ms = p.config.duration_ms(p.phase);
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(auto: bool) -> Config {
        Config { focus_min: 25, short_min: 5, long_min: 15, long_every: 2, auto_start: auto }
    }

    #[test]
    fn siklus_fokus_istirahat_dan_long_break() {
        let mut p = Pomo { config: cfg(true), ..Default::default() };
        p.enter(Phase::Focus, true, 0);
        assert_eq!(p.ends_at, Some(25 * 60_000));

        let log = p.finish(25 * 60_000, true).unwrap();
        assert_eq!(log, (Phase::Focus, 0, 25 * 60_000, true));
        assert_eq!((p.phase, p.cycle, p.running), (Phase::Short, 1, true), "auto-start ke short break");

        p.finish(30 * 60_000, true);
        assert_eq!(p.phase, Phase::Focus);
        p.finish(55 * 60_000, true);
        assert_eq!((p.phase, p.cycle), (Phase::Long, 2), "tiap 2 fokus -> long break");
        p.finish(70 * 60_000, true);
        assert_eq!((p.phase, p.cycle), (Phase::Focus, 0), "long break menutup set");
    }

    #[test]
    fn tanpa_auto_start_fase_berikut_menunggu() {
        let mut p = Pomo { config: cfg(false), ..Default::default() };
        p.enter(Phase::Focus, true, 0);
        p.finish(25 * 60_000, true);
        assert_eq!((p.phase, p.running, p.remaining_ms), (Phase::Short, false, 5 * 60_000));
        assert_eq!(p.ends_at, None);
    }

    #[test]
    fn skip_tidak_menambah_cycle() {
        let mut p = Pomo { config: cfg(false), ..Default::default() };
        p.enter(Phase::Focus, true, 0);
        let log = p.finish(1000, false).unwrap();
        assert!(!log.3);
        assert_eq!((p.phase, p.cycle), (Phase::Short, 0));
    }
}
