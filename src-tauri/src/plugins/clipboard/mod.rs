//! Plugin Clipboard Manager (TDD 4.5). Sumber kebenaran riwayat ada di sini (Rust), bukan di WebView:
//! UI hanya view yang meminta `clipboard_snapshot` setiap menerima event `clipboard://changed`.
//!
//! Pembagian berkas:
//!   ring.rs     ring buffer, hash, anggaran RAM/disk (murni)
//!   privacy.rs  format sensitif, denylist aplikasi, potong/pratinjau teks (murni)
//!   crypto.rs   ChaCha20-Poly1305 + kunci di keychain OS
//!   images.rs   PNG terenkripsi di disk + thumbnail
//!   store.rs    persistensi SQLite terenkripsi (opsional, default mati)
//!   watcher.rs  adapter OS (clipboard-rs): satu-satunya yang menyentuh clipboard sistem
//!   mod.rs      state bersama, ingest, command Tauri, event, siklus hidup
//!
//! Urutan kunci (hindari deadlock): `sealer` -> `ring`. `settings`/`self_write` tidak pernah dipegang
//! bersamaan dengan keduanya.

pub mod crypto;
pub mod images;
pub mod privacy;
pub mod ring;
pub mod store;
mod watcher;

use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
        Arc, Mutex, MutexGuard,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::core::storage::{blocking, Storage};
use self::crypto::{KeyStore, Sealer};
use self::ring::{Item, Kind, PinError, Ring, CAPACITY, MAX_PINNED, MAX_TEXT_BYTES};

/// Event Rust -> UI (di-debounce 50 ms, payload diabaikan UI).
pub const EVENT: &str = "clipboard://changed";
const SETTINGS_KEY: &str = "clipboard.settings";
const DEBOUNCE: Duration = Duration::from_millis(50);
/// Setelah kita sendiri menulis clipboard, kejadian perubahan dalam jendela ini diabaikan (`self_write`).
const SELF_WRITE_WINDOW: Duration = Duration::from_millis(1500);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

// ---------------------------------------------------------------- Tipe IPC (cermin src/modules/clipboard/types.ts)

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClipSettings {
    pub paused: bool,
    pub persist: bool,
    pub clear_on_exit: bool,
}

/// Perubahan parsial; field yang tidak ada dibiarkan.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClipSettingsPatch {
    pub paused: Option<bool>,
    pub persist: Option<bool>,
    pub clear_on_exit: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipItemDto {
    /// u64 sebagai string desimal: angka JSON dibulatkan JS di atas 2^53.
    id: String,
    kind: Kind,
    preview: String,
    truncated: bool,
    ts: i64,
    pinned: bool,
    width: Option<u32>,
    height: Option<u32>,
    bytes: Option<u64>,
}

impl From<&Item> for ClipItemDto {
    fn from(it: &Item) -> Self {
        Self {
            id: it.hash.to_string(),
            kind: it.kind,
            preview: it.preview.clone(),
            truncated: it.truncated,
            ts: it.ts,
            pinned: it.pinned,
            width: it.width,
            height: it.height,
            bytes: it.bytes,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipSnapshot {
    items: Vec<ClipItemDto>,
    settings: ClipSettings,
    capacity: usize,
}

fn parse_id(id: &str) -> Result<u64, String> {
    id.parse::<u64>().map_err(|_| "ID item tidak valid".to_string())
}

fn data_uri(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
}

// ---------------------------------------------------------------- State

struct Inner {
    ring: Mutex<Ring>,
    settings: Mutex<ClipSettings>,
    /// Kunci enkripsi berkas gambar & riwayat persisten: acak di memori saat persistensi mati, dari keychain saat hidup.
    sealer: Mutex<Sealer>,
    /// (jenis, hash, berlaku sampai). Gambar dicocokkan per jenis saja: OS bisa mengodekan ulang bitmap
    /// sehingga byte PNG yang terbaca kembali tidak identik dengan yang kita tulis.
    self_write: Mutex<Option<(Kind, u64, Instant)>>,
    notify: Sender<()>,
    storage: Storage,
    img_dir: PathBuf,
    keys: Box<dyn KeyStore>,
}

#[derive(Clone)]
pub struct ClipState(Arc<Inner>);

impl ClipState {
    pub fn settings(&self) -> ClipSettings {
        *lock(&self.0.settings)
    }

    /// Minta emit event ke UI (digabung/di-debounce oleh thread notifier).
    fn changed(&self) {
        let _ = self.0.notify.send(());
    }

    /// Tandai: tulisan clipboard berikutnya berasal dari kita, jangan direkam. Harus dipanggil SEBELUM menulis.
    pub fn expect_self_write(&self, kind: Kind, hash: u64) {
        *lock(&self.0.self_write) = Some((kind, hash, Instant::now() + SELF_WRITE_WINDOW));
    }

    fn is_self_write(&self, kind: Kind, hash: u64) -> bool {
        let mut g = lock(&self.0.self_write);
        let cur = *g;
        match cur {
            Some((k, h, until)) if Instant::now() < until => k == kind && (kind == Kind::Image || h == hash),
            Some(_) => {
                *g = None;
                false
            }
            None => false,
        }
    }

    /// Hapus berkas gambar item yang tergusur/dihapus.
    fn release(&self, items: Vec<Item>) {
        for it in items.iter().filter(|i| i.kind == Kind::Image) {
            images::delete(&self.0.img_dir, it.hash);
        }
    }

    // ------------------------------------------------------------ Ingest (dipanggil watcher)

    pub(crate) fn ingest_text(&self, raw: &str) {
        if raw.trim().is_empty() {
            return;
        }
        let (text, truncated) = privacy::truncate_text(raw, MAX_TEXT_BYTES);
        let hash = ring::hash_text(&text);
        if self.is_self_write(Kind::Text, hash) {
            return;
        }
        let item = Item {
            hash,
            kind: Kind::Text,
            preview: privacy::make_preview(&text),
            text: Some(text),
            thumb: None,
            truncated,
            ts: now_ms(),
            pinned: false,
            width: None,
            height: None,
            bytes: None,
        };
        let evicted = lock(&self.0.ring).push(item);
        self.release(evicted);
        self.changed();
    }

    pub(crate) fn ingest_image(&self, png: Vec<u8>) {
        if png.is_empty() || png.len() > images::MAX_PNG_BYTES {
            return;
        }
        let hash = ring::hash_image(&png);
        if self.is_self_write(Kind::Image, hash) {
            return;
        }
        // Duplikat: cukup pindah ke depan (berkas & thumbnail sudah ada).
        if lock(&self.0.ring).touch(hash, now_ms()) {
            self.changed();
            return;
        }
        let a = match images::analyze(&png) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("[omniwidget] clipboard: gambar dilewati: {e}");
                return;
            }
        };
        let evicted = {
            // Pegang `sealer` sampai item masuk ring: `swap_sealer` tidak boleh menyela di antara tulis berkas & push.
            let sealer = lock(&self.0.sealer);
            if let Err(e) = images::write(&self.0.img_dir, hash, &sealer, &png) {
                eprintln!("[omniwidget] clipboard: {e}");
                return;
            }
            let item = Item {
                hash,
                kind: Kind::Image,
                preview: format!("Gambar {}×{}", a.width, a.height),
                text: None,
                thumb: Some(a.thumb),
                truncated: false,
                ts: now_ms(),
                pinned: false,
                width: Some(a.width),
                height: Some(a.height),
                bytes: Some(png.len() as u64),
            };
            lock(&self.0.ring).push(item)
        };
        self.release(evicted);
        self.changed();
    }

    // ------------------------------------------------------------ Operasi untuk command

    fn snapshot(&self) -> ClipSnapshot {
        let items = lock(&self.0.ring).iter().map(ClipItemDto::from).collect();
        ClipSnapshot { items, settings: self.settings(), capacity: CAPACITY }
    }

    fn text(&self, hash: u64) -> Result<String, String> {
        let r = lock(&self.0.ring);
        let it = r.get(hash).ok_or("Item sudah tidak ada")?;
        it.text.clone().ok_or_else(|| "Item ini bukan teks".to_string())
    }

    /// Thumbnail data URI. None = tak tersedia (item hilang / berkas gambar tak terbaca). Bila sudah dilepas
    /// dari RAM (trim), dibangun ulang dari berkas di disk.
    fn thumb(&self, hash: u64) -> Result<Option<String>, String> {
        let (kind, cached) = {
            let r = lock(&self.0.ring);
            match r.get(hash) {
                Some(it) => (it.kind, it.thumb.clone()),
                None => return Ok(None),
            }
        };
        if kind != Kind::Image {
            return Ok(None);
        }
        if let Some(t) = cached {
            return Ok(Some(data_uri(&t)));
        }
        let png = {
            let sealer = lock(&self.0.sealer);
            match images::read(&self.0.img_dir, hash, &sealer) {
                Ok(p) => p,
                Err(_) => return Ok(None),
            }
        };
        let Ok(t) = images::thumb_from_png(&png) else { return Ok(None) };
        let uri = data_uri(&t);
        lock(&self.0.ring).set_thumb(hash, t);
        Ok(Some(uri))
    }

    /// Tulis item ke clipboard sistem (ditandai `self_write`), lalu pindahkan ke depan ring.
    fn paste(&self, hash: u64) -> Result<(), String> {
        let (kind, text) = {
            let r = lock(&self.0.ring);
            let it = r.get(hash).ok_or("Item sudah tidak ada")?;
            (it.kind, it.text.clone())
        };
        match kind {
            Kind::Text => {
                let t = text.ok_or("Teks item hilang")?;
                self.expect_self_write(Kind::Text, hash);
                watcher::write_text(&t)?;
            }
            Kind::Image => {
                let png = {
                    let sealer = lock(&self.0.sealer);
                    images::read(&self.0.img_dir, hash, &sealer)?
                };
                self.expect_self_write(Kind::Image, hash);
                watcher::write_image(&png)?;
            }
        }
        lock(&self.0.ring).touch(hash, now_ms());
        self.changed();
        Ok(())
    }

    fn pin(&self, hash: u64, pinned: bool) -> Result<(), String> {
        let evicted = lock(&self.0.ring).set_pinned(hash, pinned).map_err(|e| match e {
            PinError::NotFound => "Item sudah tidak ada".to_string(),
            PinError::TooMany => format!("Maksimal {MAX_PINNED} item dapat disematkan"),
        })?;
        self.release(evicted);
        self.changed();
        Ok(())
    }

    /// Idempoten: item yang sudah tidak ada dianggap berhasil.
    fn remove(&self, hash: u64) {
        let removed = lock(&self.0.ring).remove(hash);
        if let Some(it) = removed {
            self.release(vec![it]);
            self.changed();
        }
    }

    fn clear(&self, keep_pinned: bool) -> usize {
        let gone = lock(&self.0.ring).clear(keep_pinned);
        let n = gone.len();
        self.release(gone);
        if n > 0 {
            self.changed();
        }
        n
    }

    // ------------------------------------------------------------ Pengaturan & persistensi

    fn save_settings(&self, s: &ClipSettings) -> Result<(), String> {
        let json = serde_json::to_string(s).map_err(|e| e.to_string())?;
        self.0.storage.set(SETTINGS_KEY, &json)
    }

    /// Operasi berat (keychain, enkripsi ulang berkas, SQLite): jalankan lewat `blocking`.
    fn apply_patch(&self, p: ClipSettingsPatch) -> Result<ClipSettings, String> {
        let mut cur = self.settings();
        if let Some(v) = p.paused {
            cur.paused = v;
        }
        if let Some(v) = p.clear_on_exit {
            cur.clear_on_exit = v;
        }
        if let Some(want) = p.persist {
            if want != cur.persist {
                if want {
                    self.enable_persist()?;
                } else {
                    self.disable_persist()?;
                }
                cur.persist = want;
            }
        }
        *lock(&self.0.settings) = cur;
        self.save_settings(&cur)?;
        self.changed();
        Ok(cur)
    }

    fn enable_persist(&self) -> Result<(), String> {
        let key = self.0.keys.load_or_create()?;
        self.swap_sealer(Sealer::new(&key));
        self.persist_now()
    }

    fn disable_persist(&self) -> Result<(), String> {
        store::wipe(&self.0.storage)?;
        self.swap_sealer(Sealer::random()?);
        // Kunci tak lagi dipakai; gagal menghapusnya tidak menghalangi pengguna mematikan fitur.
        let _ = self.0.keys.delete();
        Ok(())
    }

    /// Ganti kunci dan enkripsi ulang semua berkas gambar. Item yang berkasnya gagal dipindahkan dibuang.
    fn swap_sealer(&self, new: Sealer) {
        let mut slot = lock(&self.0.sealer);
        let hashes = lock(&self.0.ring).image_hashes();
        let failed = images::reseal_all(&self.0.img_dir, &hashes, &slot, &new);
        *slot = new;
        if !failed.is_empty() {
            let mut r = lock(&self.0.ring);
            for h in &failed {
                r.remove(*h);
                images::delete(&self.0.img_dir, *h);
            }
            drop(r);
            self.changed();
        }
    }

    fn persist_now(&self) -> Result<(), String> {
        let items = lock(&self.0.ring).snapshot();
        let sealer = lock(&self.0.sealer);
        store::save_all(&self.0.storage, &sealer, &items)
    }

    fn persist_if_enabled(&self) {
        if self.settings().persist {
            if let Err(e) = self.persist_now() {
                eprintln!("[omniwidget] clipboard: gagal menyimpan riwayat: {e}");
            }
        }
    }

    /// Dipanggil saat aplikasi benar-benar keluar (bukan Deep Sleep).
    fn shutdown(&self) {
        let s = self.settings();
        if s.clear_on_exit {
            let gone = lock(&self.0.ring).clear(true);
            self.release(gone);
        }
        if s.persist {
            if let Err(e) = self.persist_now() {
                eprintln!("[omniwidget] clipboard: gagal menyimpan riwayat saat keluar: {e}");
            }
        } else {
            // Riwayat hanya di memori: berkas gambar sementara tidak boleh tertinggal.
            images::purge_all(&self.0.img_dir);
        }
    }
}

// ---------------------------------------------------------------- Pembukaan (dipakai setup & uji)

/// Bangun state dari penyimpanan: pulihkan riwayat bila persistensi hidup & keychain bisa dibuka;
/// bila tidak, mulai kosong dan bersihkan sisa berkas.
pub(crate) fn open(
    storage: Storage,
    img_dir: PathBuf,
    keys: Box<dyn KeyStore>,
) -> Result<(ClipState, Receiver<()>), String> {
    let mut settings: ClipSettings = storage
        .get(SETTINGS_KEY)?
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let mut ring = Ring::new(CAPACITY);
    let mut sealer: Option<Sealer> = None;
    if settings.persist {
        match keys.load_or_create() {
            Ok(key) => {
                let s = Sealer::new(&key);
                match store::load_all(&storage, &s) {
                    Ok(items) => ring.restore(
                        items
                            .into_iter()
                            .filter(|it| it.kind == Kind::Text || images::exists(&img_dir, it.hash))
                            .collect(),
                    ),
                    Err(e) => eprintln!("[omniwidget] clipboard: riwayat tersimpan tak terbaca: {e}"),
                }
                sealer = Some(s);
            }
            Err(e) => {
                // Jangan turun ke penyimpanan tak aman: matikan persistensi dan mulai kosong.
                eprintln!("[omniwidget] clipboard: persistensi dimatikan: {e}");
                settings.persist = false;
            }
        }
    }
    let sealer = match sealer {
        Some(s) => s,
        None => {
            store::wipe(&storage)?;
            Sealer::random()?
        }
    };
    let keep: HashSet<u64> = ring.image_hashes().into_iter().collect();
    images::retain_only(&img_dir, &keep);

    let (tx, rx) = mpsc::channel();
    let st = ClipState(Arc::new(Inner {
        ring: Mutex::new(ring),
        settings: Mutex::new(settings),
        sealer: Mutex::new(sealer),
        self_write: Mutex::new(None),
        notify: tx,
        storage,
        img_dir,
        keys,
    }));
    st.save_settings(&settings)?;
    Ok((st, rx))
}

/// Thread notifier: menggabungkan rentetan perubahan jadi satu event (debounce 50 ms, TDD 5.6),
/// lalu menyimpan riwayat terenkripsi bila persistensi hidup. Tidur total saat tidak ada perubahan.
fn spawn_notifier(app: AppHandle, st: ClipState, rx: Receiver<()>) {
    let spawned = std::thread::Builder::new().name("clip-notify".into()).spawn(move || {
        while rx.recv().is_ok() {
            loop {
                match rx.recv_timeout(DEBOUNCE) {
                    Ok(()) => continue,
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            let _ = app.emit(EVENT, ());
            st.persist_if_enabled();
        }
    });
    if let Err(e) = spawned {
        eprintln!("[omniwidget] clipboard: gagal menjalankan thread notifier: {e}");
    }
}

/// Dipanggil sekali dari `setup` (setelah `Storage` didaftarkan).
pub fn setup(app: &AppHandle) -> Result<(), String> {
    let storage = app.state::<Storage>().inner().clone();
    let img_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("clipboard");
    let (st, rx) = open(storage, img_dir, Box::new(crypto::OsKeyStore))?;
    app.manage(st.clone());
    spawn_notifier(app.clone(), st.clone(), rx);
    watcher::spawn(st);
    Ok(())
}

/// Dari `RunEvent::Exit`: terapkan "bersihkan saat keluar" dan simpan/bersihkan.
pub fn on_exit(app: &AppHandle) {
    if let Some(st) = app.try_state::<ClipState>() {
        st.shutdown();
    }
}

/// Trim L2+ (TDD 5.4): lepas thumbnail dari RAM. Dipanggil saat Deep Sleep; Cache Governor kelak memanggil
/// fungsi yang sama untuk L2. Mengembalikan byte yang dibebaskan.
pub fn release_thumbnails(app: &AppHandle) -> usize {
    match app.try_state::<ClipState>() {
        Some(st) => {
            let freed = lock(&st.0.ring).release_thumbs();
            freed
        }
        None => 0,
    }
}

// ---------------------------------------------------------------- Command Tauri (kontrak: src/modules/clipboard/api.ts)

#[tauri::command]
pub async fn clipboard_snapshot(state: State<'_, ClipState>) -> Result<ClipSnapshot, String> {
    Ok(state.snapshot())
}

#[tauri::command]
pub async fn clipboard_thumb(state: State<'_, ClipState>, id: String) -> Result<Option<String>, String> {
    let hash = parse_id(&id)?;
    let st = state.inner().clone();
    blocking(move || st.thumb(hash)).await
}

#[tauri::command]
pub async fn clipboard_text(state: State<'_, ClipState>, id: String) -> Result<String, String> {
    state.text(parse_id(&id)?)
}

#[tauri::command]
pub async fn clipboard_paste(state: State<'_, ClipState>, id: String) -> Result<(), String> {
    let hash = parse_id(&id)?;
    let st = state.inner().clone();
    blocking(move || st.paste(hash)).await
}

#[tauri::command]
pub async fn clipboard_pin(state: State<'_, ClipState>, id: String, pinned: bool) -> Result<(), String> {
    state.pin(parse_id(&id)?, pinned)
}

#[tauri::command]
pub async fn clipboard_remove(state: State<'_, ClipState>, id: String) -> Result<(), String> {
    state.remove(parse_id(&id)?);
    Ok(())
}

#[tauri::command]
pub async fn clipboard_clear(state: State<'_, ClipState>, keep_pinned: bool) -> Result<usize, String> {
    let st = state.inner().clone();
    blocking(move || Ok(st.clear(keep_pinned))).await
}

#[tauri::command]
pub async fn clipboard_set_settings(
    state: State<'_, ClipState>,
    patch: ClipSettingsPatch,
) -> Result<ClipSettings, String> {
    let st = state.inner().clone();
    blocking(move || st.apply_patch(patch)).await
}

// ---------------------------------------------------------------- Uji

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Keychain palsu yang bisa dipakai bersama antar "restart" aplikasi.
    #[derive(Clone, Default)]
    struct MemKeys(Arc<Mutex<Option<[u8; 32]>>>);

    impl KeyStore for MemKeys {
        fn load_or_create(&self) -> Result<[u8; 32], String> {
            let mut g = self.0.lock().unwrap();
            if g.is_none() {
                *g = Some(crypto::random_key()?);
            }
            Ok(g.unwrap())
        }
        fn delete(&self) -> Result<(), String> {
            *self.0.lock().unwrap() = None;
            Ok(())
        }
    }

    struct NoKeychain;
    impl KeyStore for NoKeychain {
        fn load_or_create(&self) -> Result<[u8; 32], String> {
            Err("Keychain OS tidak tersedia".into())
        }
        fn delete(&self) -> Result<(), String> {
            Ok(())
        }
    }

    fn png(w: u32, h: u32, seed: u8) -> Vec<u8> {
        let img = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x as u8).wrapping_add(seed), y as u8, seed, 255]));
        let mut cur = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut cur, image::ImageFormat::Png).unwrap();
        cur.into_inner()
    }

    fn boot(storage: &Storage, dir: &std::path::Path, keys: &MemKeys) -> (ClipState, Receiver<()>) {
        open(storage.clone(), dir.to_path_buf(), Box::new(keys.clone())).unwrap()
    }

    fn texts(st: &ClipState) -> Vec<String> {
        lock(&st.0.ring).iter().filter_map(|i| i.text.clone()).collect()
    }

    #[test]
    fn teks_dedup_dipotong_dan_spasi_kosong_diabaikan() {
        let dir = tempfile::tempdir().unwrap();
        let (st, rx) = boot(&Storage::open_in_memory().unwrap(), dir.path(), &MemKeys::default());
        st.ingest_text("   \n\t ");
        assert!(rx.try_recv().is_err(), "teks kosong tidak memicu event");
        st.ingest_text("satu");
        st.ingest_text("dua");
        st.ingest_text("satu"); // salin ulang: pindah ke depan, tidak dobel
        assert_eq!(texts(&st), vec!["satu", "dua"]);
        assert!(rx.try_recv().is_ok());

        st.ingest_text(&"x".repeat(MAX_TEXT_BYTES + 100));
        let snap = st.snapshot();
        assert!(snap.items[0].truncated);
        assert_eq!(st.text(snap.items[0].id.parse().unwrap()).unwrap().len(), MAX_TEXT_BYTES);
        assert_eq!(snap.capacity, 20);
    }

    #[test]
    fn self_write_diabaikan_hanya_untuk_konten_yang_ditulis_sendiri() {
        let dir = tempfile::tempdir().unwrap();
        let (st, _rx) = boot(&Storage::open_in_memory().unwrap(), dir.path(), &MemKeys::default());
        st.expect_self_write(Kind::Text, ring::hash_text("dari-kita"));
        st.ingest_text("dari-kita");
        st.ingest_text("dari-kita"); // kejadian ganda untuk satu tulisan (umum di Windows)
        assert!(texts(&st).is_empty());
        st.ingest_text("salinan-pengguna"); // konten lain dalam jendela yang sama tetap direkam
        assert_eq!(texts(&st), vec!["salinan-pengguna"]);
        // Kedaluwarsa: setelah jendela lewat, konten yang sama direkam lagi.
        *lock(&st.0.self_write) = Some((Kind::Text, ring::hash_text("dari-kita"), Instant::now() - Duration::from_secs(1)));
        st.ingest_text("dari-kita");
        assert_eq!(texts(&st), vec!["dari-kita", "salinan-pengguna"]);
    }

    #[test]
    fn pin_remove_clear_dan_id_tidak_valid() {
        let dir = tempfile::tempdir().unwrap();
        let (st, _rx) = boot(&Storage::open_in_memory().unwrap(), dir.path(), &MemKeys::default());
        for t in ["a", "b", "c"] {
            st.ingest_text(t);
        }
        let id_a = ring::hash_text("a");
        st.pin(id_a, true).unwrap();
        assert!(st.pin(12345, true).unwrap_err().contains("tidak ada"));
        st.remove(ring::hash_text("b"));
        st.remove(ring::hash_text("b")); // idempoten
        assert_eq!(st.clear(true), 1); // hanya "c"; "a" tersemat
        assert_eq!(texts(&st), vec!["a"]);
        assert_eq!(st.clear(false), 1);
        assert!(st.snapshot().items.is_empty());
        assert!(parse_id("bukan-angka").is_err());
        assert_eq!(parse_id("18446744073709551615").unwrap(), u64::MAX, "u64 penuh lolos");
    }

    #[test]
    fn gambar_thumbnail_dilepas_dimuat_ulang_dan_berkas_ikut_terhapus() {
        let dir = tempfile::tempdir().unwrap();
        let (st, _rx) = boot(&Storage::open_in_memory().unwrap(), dir.path(), &MemKeys::default());
        let data = png(200, 100, 7);
        st.ingest_image(data.clone());
        st.ingest_image(data.clone()); // dedup
        let snap = st.snapshot();
        assert_eq!(snap.items.len(), 1);
        let it = &snap.items[0];
        assert_eq!((it.width, it.height), (Some(200), Some(100)));
        assert_eq!(it.preview, "Gambar 200×100");
        let id: u64 = it.id.parse().unwrap();
        assert!(images::exists(dir.path(), id));

        let uri = st.thumb(id).unwrap().unwrap();
        assert!(uri.starts_with("data:image/png;base64,"));
        // Trim L2: lepas dari RAM, lalu muat ulang dari disk.
        assert!(lock(&st.0.ring).release_thumbs() > 0);
        assert_eq!(st.thumb(id).unwrap().unwrap(), uri);
        assert_eq!(st.thumb(999).unwrap(), None);
        assert!(st.text(id).is_err(), "gambar bukan teks");

        st.remove(id);
        assert!(!images::exists(dir.path(), id));
    }

    #[test]
    fn persistensi_bolak_balik_restart_dan_dimatikan_menghapus_semua() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::open_in_memory().unwrap();
        let keys = MemKeys::default();

        let (st, _rx) = boot(&storage, dir.path(), &keys);
        st.ingest_text("sebelum-persist"); // hanya di memori
        let img = png(64, 64, 3);
        st.ingest_image(img.clone());
        let img_id = ring::hash_image(&img);

        // Nyalakan: kunci keychain dipakai, berkas gambar dienkripsi ulang, riwayat ditulis.
        let s = st.apply_patch(ClipSettingsPatch { persist: Some(true), ..Default::default() }).unwrap();
        assert!(s.persist);
        st.ingest_text("sesudah-persist");
        st.persist_if_enabled();
        assert!(keys.0.lock().unwrap().is_some());

        // "Restart": instance baru dari penyimpanan & keychain yang sama.
        let (st2, _rx2) = boot(&storage, dir.path(), &keys);
        assert!(st2.settings().persist);
        assert_eq!(texts(&st2), vec!["sesudah-persist", "sebelum-persist"]);
        assert!(st2.thumb(img_id).unwrap().is_some(), "gambar terbaca dengan kunci keychain");
        drop(st2);

        // Matikan: tabel kosong, kunci dihapus dari keychain, berkas tetap terbaca (kunci acak sesi).
        let s = st.apply_patch(ClipSettingsPatch { persist: Some(false), ..Default::default() }).unwrap();
        assert!(!s.persist);
        assert!(keys.0.lock().unwrap().is_none());
        assert!(store::load_all(&storage, &Sealer::random().unwrap()).unwrap().is_empty());
        assert!(st.thumb(img_id).unwrap().is_some());

        let (st3, _rx3) = boot(&storage, dir.path(), &keys);
        assert!(!st3.settings().persist);
        assert!(st3.snapshot().items.is_empty(), "mati = restart mulai kosong");
        assert!(!images::exists(dir.path(), img_id), "sisa berkas gambar dibersihkan");
    }

    #[test]
    fn keychain_tak_tersedia_menolak_persist_dan_start_tetap_aman() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::open_in_memory().unwrap();
        storage.set(SETTINGS_KEY, r#"{"persist":true,"paused":true}"#).unwrap();
        let (st, _rx) = open(storage.clone(), dir.path().to_path_buf(), Box::new(NoKeychain)).unwrap();
        let s = st.settings();
        assert!(!s.persist, "tanpa keychain persistensi dimatikan, bukan diturunkan ke penyimpanan tak aman");
        assert!(s.paused, "pengaturan lain tetap");

        let err = st.apply_patch(ClipSettingsPatch { persist: Some(true), ..Default::default() }).unwrap_err();
        assert!(err.contains("Keychain"));
        assert!(!st.settings().persist);
    }

    #[test]
    fn keluar_membersihkan_sesuai_pengaturan() {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::open_in_memory().unwrap();
        let (st, _rx) = boot(&storage, dir.path(), &MemKeys::default());
        st.ingest_text("biasa");
        st.ingest_text("penting");
        st.pin(ring::hash_text("penting"), true).unwrap();
        st.ingest_image(png(16, 16, 1));
        st.apply_patch(ClipSettingsPatch { clear_on_exit: Some(true), ..Default::default() }).unwrap();
        st.shutdown();
        assert_eq!(texts(&st), vec!["penting"], "yang tersemat bertahan di memori, sisanya dibuang");
        // Persistensi mati: berkas gambar sementara tidak tertinggal.
        assert_eq!(fs_count(dir.path()), 0);
    }

    fn fs_count(dir: &std::path::Path) -> usize {
        std::fs::read_dir(dir).map(|r| r.count()).unwrap_or(0)
    }

    #[test]
    fn kontrak_json_cocok_dengan_types_ts() {
        let dir = tempfile::tempdir().unwrap();
        let (st, _rx) = boot(&Storage::open_in_memory().unwrap(), dir.path(), &MemKeys::default());
        st.ingest_text("halo dunia");
        let v = serde_json::to_value(st.snapshot()).unwrap();
        assert_eq!(v["capacity"], 20);
        assert_eq!(v["settings"], serde_json::json!({ "paused": false, "persist": false, "clearOnExit": false }));
        let it = &v["items"][0];
        assert_eq!(it["kind"], "text");
        assert!(it["id"].is_string(), "id harus string");
        assert_eq!(it["preview"], "halo dunia");
        assert_eq!(it["truncated"], false);
        assert_eq!(it["pinned"], false);
        assert!(it["ts"].is_i64());
        assert!(it["width"].is_null() && it["height"].is_null() && it["bytes"].is_null());
        // Patch dari UI: camelCase, field boleh hilang.
        let p: ClipSettingsPatch = serde_json::from_str(r#"{"clearOnExit":true}"#).unwrap();
        assert_eq!((p.paused, p.persist, p.clear_on_exit), (None, None, Some(true)));
        let p: ClipSettingsPatch = serde_json::from_str("{}").unwrap();
        assert_eq!((p.paused, p.persist, p.clear_on_exit), (None, None, None));
    }
}
