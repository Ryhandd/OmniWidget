//! To-do & Reminder (TDD 4.3). SQLite (tabel `task`) + satu sumber tenggat di scheduler bersama.
//! Pengulangan (harian/hari kerja/mingguan/bulanan) dihitung di UI karena butuh zona waktu lokal;
//! Rust hanya menyimpan dan menembakkan pengingat (termasuk yang terlewat saat laptop tidur).

use std::sync::Arc;

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;

use crate::core::scheduler::{self, now_ms, Scheduler, Timed};
use crate::core::storage::{blocking, Storage};

pub const EVENT: &str = "todo://changed";
const MAX_TITLE: usize = 300;
const MAX_NOTE: usize = 4000;
const REPEATS: [&str; 4] = ["daily", "weekdays", "weekly", "monthly"];

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS task(
        id           INTEGER PRIMARY KEY,
        title        TEXT NOT NULL,
        note         TEXT NOT NULL DEFAULT '',
        done         INTEGER NOT NULL DEFAULT 0,
        priority     INTEGER NOT NULL DEFAULT 0,
        due_at       INTEGER,
        remind_at    INTEGER,
        reminded     INTEGER NOT NULL DEFAULT 0,
        repeat       TEXT,
        created_at   INTEGER NOT NULL,
        completed_at INTEGER
    );
    CREATE INDEX IF NOT EXISTS idx_task_remind ON task(remind_at) WHERE reminded = 0 AND done = 0;
";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    id: i64,
    title: String,
    note: String,
    done: bool,
    priority: i64,
    due_at: Option<i64>,
    remind_at: Option<i64>,
    reminded: bool,
    repeat: Option<String>,
    created_at: i64,
    completed_at: Option<i64>,
}

/// Isi lengkap dari UI (tambah/ubah). Nilai null = kosongkan.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    title: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    priority: i64,
    due_at: Option<i64>,
    remind_at: Option<i64>,
    repeat: Option<String>,
}

impl TaskInput {
    fn clean(mut self) -> Result<Self, String> {
        self.title = self.title.trim().chars().take(MAX_TITLE).collect();
        if self.title.is_empty() {
            return Err("Judul tugas kosong".into());
        }
        self.note = self.note.chars().take(MAX_NOTE).collect();
        self.priority = self.priority.clamp(0, 2);
        if let Some(r) = &self.repeat {
            if !REPEATS.contains(&r.as_str()) {
                self.repeat = None;
            }
        }
        Ok(self)
    }
}

fn row(r: &Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: r.get(0)?,
        title: r.get(1)?,
        note: r.get(2)?,
        done: r.get::<_, i64>(3)? != 0,
        priority: r.get(4)?,
        due_at: r.get(5)?,
        remind_at: r.get(6)?,
        reminded: r.get::<_, i64>(7)? != 0,
        repeat: r.get(8)?,
        created_at: r.get(9)?,
        completed_at: r.get(10)?,
    })
}

const COLS: &str = "id, title, note, done, priority, due_at, remind_at, reminded, repeat, created_at, completed_at";

fn get(c: &Connection, id: i64) -> Result<Task, String> {
    c.query_row(&format!("SELECT {COLS} FROM task WHERE id = ?1"), params![id], row)
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Tugas tidak ditemukan".into())
}

pub fn init(storage: &Storage) -> Result<(), String> {
    storage.with_conn(|c| c.execute_batch(SCHEMA).map_err(|e| e.to_string()))
}

fn list(c: &Connection) -> Result<Vec<Task>, String> {
    // Aktif dulu (tenggat terdekat, prioritas), lalu 50 tugas selesai terbaru.
    let mut out = Vec::new();
    let mut st = c
        .prepare(&format!(
            "SELECT {COLS} FROM task WHERE done = 0
             ORDER BY due_at IS NULL, due_at, priority DESC, id DESC"
        ))
        .map_err(|e| e.to_string())?;
    out.extend(st.query_map([], row).map_err(|e| e.to_string())?.filter_map(Result::ok));
    let mut st = c
        .prepare(&format!("SELECT {COLS} FROM task WHERE done = 1 ORDER BY completed_at DESC LIMIT 50"))
        .map_err(|e| e.to_string())?;
    out.extend(st.query_map([], row).map_err(|e| e.to_string())?.filter_map(Result::ok));
    Ok(out)
}

fn changed(app: &AppHandle) {
    let _ = app.emit(EVENT, ());
    scheduler::poke(app);
}

// ---------------------------------------------------------------- Sumber tenggat

pub struct Reminders;

impl Timed for Reminders {
    fn next_due(&self, app: &AppHandle) -> Option<i64> {
        let s = app.try_state::<Storage>()?;
        s.with_conn(|c| {
            c.query_row(
                "SELECT MIN(remind_at) FROM task WHERE remind_at IS NOT NULL AND reminded = 0 AND done = 0",
                [],
                |r| r.get::<_, Option<i64>>(0),
            )
            .map_err(|e| e.to_string())
        })
        .ok()
        .flatten()
    }

    fn fire(&self, app: &AppHandle, now: i64) {
        let Some(s) = app.try_state::<Storage>() else { return };
        let due: Vec<(i64, String)> = s
            .with_conn(|c| {
                let mut st = c
                    .prepare(
                        "SELECT id, title FROM task WHERE remind_at IS NOT NULL AND remind_at <= ?1
                         AND reminded = 0 AND done = 0 ORDER BY remind_at",
                    )
                    .map_err(|e| e.to_string())?;
                let v = st
                    .query_map(params![now], |r| Ok((r.get(0)?, r.get(1)?)))
                    .map_err(|e| e.to_string())?
                    .filter_map(Result::ok)
                    .collect::<Vec<_>>();
                for (id, _) in &v {
                    c.execute("UPDATE task SET reminded = 1 WHERE id = ?1", params![id]).map_err(|e| e.to_string())?;
                }
                Ok(v)
            })
            .unwrap_or_default();
        if due.is_empty() {
            return;
        }
        // Beberapa sekaligus (mis. terlewat saat laptop tidur) -> satu notifikasi gabungan.
        let (title, body) = if due.len() == 1 {
            ("Pengingat".to_string(), due[0].1.clone())
        } else {
            (
                format!("{} pengingat", due.len()),
                due.iter().take(4).map(|(_, t)| format!("• {t}")).collect::<Vec<_>>().join("\n"),
            )
        };
        let _ = app.notification().builder().title(title).body(body).show();
        let label: String = if due.len() == 1 { due[0].1.chars().take(24).collect() } else { format!("{} pengingat", due.len()) };
        let _ = app.emit(
            "shell://indicator",
            serde_json::json!({ "id": "todo", "icon": "🔔", "label": label, "tone": "warn", "tab": "todo" }),
        );
        let _ = app.emit(EVENT, ());
    }
}

pub fn setup(app: &AppHandle) -> Result<(), String> {
    init(&app.state::<Storage>())?;
    app.state::<Scheduler>().register(Arc::new(Reminders));
    Ok(())
}

// ---------------------------------------------------------------- Commands

#[tauri::command]
pub async fn todo_list(state: State<'_, Storage>) -> Result<Vec<Task>, String> {
    let s = state.inner().clone();
    blocking(move || s.with_conn(list)).await
}

#[tauri::command]
pub async fn todo_add(app: AppHandle, state: State<'_, Storage>, task: TaskInput) -> Result<Task, String> {
    let t = task.clean()?;
    let s = state.inner().clone();
    let out = blocking(move || {
        s.with_conn(|c| {
            c.execute(
                "INSERT INTO task(title, note, priority, due_at, remind_at, repeat, created_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![t.title, t.note, t.priority, t.due_at, t.remind_at, t.repeat, now_ms()],
            )
            .map_err(|e| e.to_string())?;
            get(c, c.last_insert_rowid())
        })
    })
    .await?;
    changed(&app);
    Ok(out)
}

/// Ubah isi tugas. Pengingat yang diubah otomatis "dipersenjatai" lagi (reminded = 0).
#[tauri::command]
pub async fn todo_update(app: AppHandle, state: State<'_, Storage>, id: i64, task: TaskInput) -> Result<Task, String> {
    let t = task.clean()?;
    let s = state.inner().clone();
    let out = blocking(move || {
        s.with_conn(|c| {
            let n = c
                .execute(
                    "UPDATE task SET title = ?2, note = ?3, priority = ?4, due_at = ?5, repeat = ?7,
                       reminded = CASE WHEN remind_at IS ?6 THEN reminded ELSE 0 END,
                       remind_at = ?6
                     WHERE id = ?1",
                    params![id, t.title, t.note, t.priority, t.due_at, t.remind_at, t.repeat],
                )
                .map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("Tugas tidak ditemukan".into());
            }
            get(c, id)
        })
    })
    .await?;
    changed(&app);
    Ok(out)
}

#[tauri::command]
pub async fn todo_set_done(app: AppHandle, state: State<'_, Storage>, id: i64, done: bool) -> Result<Task, String> {
    let s = state.inner().clone();
    let out = blocking(move || {
        s.with_conn(|c| {
            c.execute(
                "UPDATE task SET done = ?2, completed_at = CASE WHEN ?2 = 1 THEN ?3 ELSE NULL END WHERE id = ?1",
                params![id, done as i64, now_ms()],
            )
            .map_err(|e| e.to_string())?;
            get(c, id)
        })
    })
    .await?;
    if done {
        let _ = app.emit("shell://indicator", serde_json::json!({ "id": "todo", "clear": true }));
    }
    changed(&app);
    Ok(out)
}

#[tauri::command]
pub async fn todo_snooze(app: AppHandle, state: State<'_, Storage>, id: i64, minutes: i64) -> Result<Task, String> {
    let at = now_ms() + minutes.clamp(1, 24 * 60) * 60_000;
    let s = state.inner().clone();
    let out = blocking(move || {
        s.with_conn(|c| {
            c.execute("UPDATE task SET remind_at = ?2, reminded = 0 WHERE id = ?1", params![id, at])
                .map_err(|e| e.to_string())?;
            get(c, id)
        })
    })
    .await?;
    let _ = app.emit("shell://indicator", serde_json::json!({ "id": "todo", "clear": true }));
    changed(&app);
    Ok(out)
}

#[tauri::command]
pub async fn todo_delete(app: AppHandle, state: State<'_, Storage>, id: i64) -> Result<(), String> {
    let s = state.inner().clone();
    blocking(move || s.with_conn(|c| c.execute("DELETE FROM task WHERE id = ?1", params![id]).map(|_| ()).map_err(|e| e.to_string()))).await?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn todo_clear_done(app: AppHandle, state: State<'_, Storage>) -> Result<usize, String> {
    let s = state.inner().clone();
    let n = blocking(move || s.with_conn(|c| c.execute("DELETE FROM task WHERE done = 1", []).map_err(|e| e.to_string()))).await?;
    changed(&app);
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(title: &str, remind_at: Option<i64>) -> TaskInput {
        TaskInput { title: title.into(), note: String::new(), priority: 5, due_at: None, remind_at, repeat: Some("yearly".into()) }
    }

    #[test]
    fn clean_memvalidasi_input() {
        assert!(input("   ", None).clean().is_err());
        let t = input("  Beli kopi  ", None).clean().unwrap();
        assert_eq!(t.title, "Beli kopi");
        assert_eq!(t.priority, 2, "prioritas dijepit 0..2");
        assert_eq!(t.repeat, None, "pengulangan tak dikenal dibuang");
    }

    #[test]
    fn urutan_daftar_dan_tenggat_pengingat() {
        let s = Storage::open_in_memory().unwrap();
        init(&s).unwrap();
        s.with_conn(|c| {
            for (title, due, rem) in [("b", Some(200), Some(150)), ("a", Some(100), Some(90)), ("c", None, None)] {
                c.execute(
                    "INSERT INTO task(title, due_at, remind_at, created_at) VALUES(?1, ?2, ?3, 0)",
                    params![title, due, rem],
                )
                .unwrap();
            }
            let l = list(c)?;
            assert_eq!(l.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
            let min: Option<i64> = c
                .query_row("SELECT MIN(remind_at) FROM task WHERE remind_at IS NOT NULL AND reminded = 0 AND done = 0", [], |r| r.get(0))
                .unwrap();
            assert_eq!(min, Some(90));
            Ok(())
        })
        .unwrap();
    }
}
