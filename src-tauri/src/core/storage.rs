//! Penyimpanan lokal: SQLite (WAL) dengan tabel key-value untuk pengaturan kecil dan draft.
//! Semua I/O dari UI lewat `bridge` (kv_get / kv_set / kv_delete): WebView tidak menyentuh
//! `localStorage` untuk data penting, jadi state tetap utuh saat Deep Sleep menghancurkannya.
//! Tabel khusus (to-do, reminder, pomodoro) akan ditambahkan di berkas ini sebagai migrasi.

use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{params, Connection, OptionalExtension};
use tauri::State;

pub const MAX_KEY_LEN: usize = 128;
/// Batas per nilai: menjaga RAM & ukuran DB tetap kecil (draft prompt jauh di bawah ini).
pub const MAX_VALUE_BYTES: usize = 256 * 1024;

const SCHEMA: &str = "
    PRAGMA journal_mode = WAL;
    PRAGMA synchronous = NORMAL;
    CREATE TABLE IF NOT EXISTS kv(
        key        TEXT PRIMARY KEY,
        value      TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    ) WITHOUT ROWID;
    -- Riwayat clipboard terenkripsi (hanya terisi bila pengguna mengaktifkan persistensi). Isi = blob
    -- ChaCha20-Poly1305; `seq` hanya urutan (lama -> baru), tidak membocorkan isi.
    CREATE TABLE IF NOT EXISTS clip_items(
        seq  INTEGER PRIMARY KEY AUTOINCREMENT,
        blob BLOB NOT NULL
    );
";

#[derive(Clone)]
pub struct Storage {
    conn: Arc<Mutex<Connection>>,
}

impl Storage {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| format!("Gagal membuka database: {e}"))?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, String> {
        Self::init(Connection::open_in_memory().map_err(|e| e.to_string())?)
    }

    fn init(conn: Connection) -> Result<Self, String> {
        conn.execute_batch(SCHEMA).map_err(|e| format!("Gagal menyiapkan skema: {e}"))?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.conn.lock().map_err(|_| "Database sedang tidak tersedia".to_string())
    }

    /// Akses koneksi untuk modul bertabel khusus (clipboard, nanti to-do). Kunci dilepas saat `f` selesai.
    pub(crate) fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
        let guard = self.lock()?;
        f(&guard)
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, String> {
        validate_key(key)?;
        self.lock()?
            .query_row("SELECT value FROM kv WHERE key = ?1", params![key], |r| r.get::<_, String>(0))
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), String> {
        validate_key(key)?;
        if value.len() > MAX_VALUE_BYTES {
            return Err(format!("Nilai terlalu besar ({} KB, maks {} KB)", value.len() / 1024, MAX_VALUE_BYTES / 1024));
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        self.lock()?
            .execute(
                "INSERT INTO kv(key, value, updated_at) VALUES(?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![key, value, now],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn delete(&self, key: &str) -> Result<(), String> {
        validate_key(key)?;
        self.lock()?
            .execute("DELETE FROM kv WHERE key = ?1", params![key])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// Kunci berprefiks `secret.` (mis. API key) hanya boleh dibaca/ditulis oleh kode Rust. Perintah `kv_*`
/// yang dipanggil dari WebView menolaknya, jadi isi WebView yang disusupi pun tidak bisa membocorkannya.
pub const SECRET_PREFIX: &str = "secret.";

fn reject_secret(key: &str) -> Result<(), String> {
    if key.starts_with(SECRET_PREFIX) {
        Err(format!("Kunci dicadangkan: '{key}'"))
    } else {
        Ok(())
    }
}

/// Kunci berbentuk `modul.nama` (huruf kecil, angka, `.`, `_`, `-`). Menolak input aneh dari UI.
fn validate_key(key: &str) -> Result<(), String> {
    let ok = !key.is_empty()
        && key.len() <= MAX_KEY_LEN
        && key.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'));
    if ok { Ok(()) } else { Err(format!("Kunci tidak valid: '{key}'")) }
}

// ---------- Commands ----------
// Operasi SQLite singkat, tetapi tetap di thread blocking agar tidak menahan event loop IPC.

pub(crate) async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn kv_get(state: State<'_, Storage>, key: String) -> Result<Option<String>, String> {
    reject_secret(&key)?;
    let s = state.inner().clone();
    blocking(move || s.get(&key)).await
}

#[tauri::command]
pub async fn kv_set(state: State<'_, Storage>, key: String, value: String) -> Result<(), String> {
    reject_secret(&key)?;
    let s = state.inner().clone();
    blocking(move || s.set(&key, &value)).await
}

#[tauri::command]
pub async fn kv_delete(state: State<'_, Storage>, key: String) -> Result<(), String> {
    reject_secret(&key)?;
    let s = state.inner().clone();
    blocking(move || s.delete(&key)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_timpa_dan_hapus() {
        let s = Storage::open_in_memory().unwrap();
        assert_eq!(s.get("anatomy.draft").unwrap(), None);
        s.set("anatomy.draft", "{\"a\":1}").unwrap();
        assert_eq!(s.get("anatomy.draft").unwrap().as_deref(), Some("{\"a\":1}"));
        s.set("anatomy.draft", "baru").unwrap();
        assert_eq!(s.get("anatomy.draft").unwrap().as_deref(), Some("baru"));
        s.delete("anatomy.draft").unwrap();
        assert_eq!(s.get("anatomy.draft").unwrap(), None);
    }

    #[test]
    fn validasi_kunci_dan_ukuran() {
        let s = Storage::open_in_memory().unwrap();
        assert!(s.get("").is_err());
        assert!(s.get("Besar").is_err());
        assert!(s.get("a b").is_err());
        assert!(s.set("ok.key", &"x".repeat(MAX_VALUE_BYTES + 1)).is_err());
        assert!(s.set(&"k".repeat(MAX_KEY_LEN + 1), "v").is_err());
        assert!(s.set("converter.out_dir", "C:\\Users\\x").is_ok());
    }

    #[test]
    fn kunci_secret_ditolak_untuk_perintah_webview_tetapi_boleh_untuk_kode_rust() {
        assert!(reject_secret("secret.llm_api_key").is_err());
        assert!(reject_secret("anatomy.draft").is_ok());
        let s = Storage::open_in_memory().unwrap();
        assert!(s.set("secret.llm_api_key", "x").is_ok());
        assert_eq!(s.get("secret.llm_api_key").unwrap().as_deref(), Some("x"));
    }
}
