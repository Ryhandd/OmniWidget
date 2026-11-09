//! Implementasi Windows: SMTC + Core Audio. Semua state COM hidup di satu thread pekerja.

use std::collections::{HashMap, VecDeque};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tokio::sync::{mpsc, oneshot};

use windows::core::Interface;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionMediaProperties,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
use windows::Media::MediaPlaybackAutoRepeatMode;
use windows::Storage::Streams::DataReader;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
    ISimpleAudioVolume, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

/// Tunggu `IAsyncOperation<bool>` tanpa menyebut tipe masa depannya (crate windows-future tidak
/// diimpor langsung). Makro dipakai di dalam fungsi async, jadi `.await` tetap valid.
macro_rules! finish {
    ($op:expr) => {{
        match $op {
            Ok(op) => match op.await {
                Ok(true) => Ok(()),
                Ok(false) => Err("Spotify tidak mengeksekusi perintah itu saat ini".to_string()),
                Err(e) => Err(format!("Perintah gagal: {e}")),
            },
            Err(e) => Err(format!("Spotify menolak perintah: {e}")),
        }
    }};
}

/// Batas ukuran sampul yang mau diunduh (data URI membengkak ~33% saat base64).
const MAX_ART_BYTES: u64 = 8 * 1024 * 1024;
/// Jumlah sampul yang disimpan di memori pekerja (masing-masing puluhan–ratusan KB).
const ART_CACHE: usize = 12;

// ---------------------------------------------------------------- DTO

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Ada sesi media yang bisa dibaca. `false` -> UI menampilkan ajakan membuka Spotify.
    pub available: bool,
    /// AUMID aplikasi pemilik sesi (mis. `SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify`).
    pub app: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub shuffle: bool,
    /// `off` | `track` | `list`
    pub repeat: String,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_next: bool,
    pub can_prev: bool,
    pub can_seek: bool,
    pub can_shuffle: bool,
    pub can_repeat: bool,
    /// Identitas sampul stabil (`app|judul|artis|album`). UI memakainya untuk tahu kapan sampul berganti.
    pub art_key: String,
    /// Data URI. Hanya dikirim bila `art_key` berbeda dari `knownArt` yang dikirim UI.
    pub art: Option<String>,
}

// ---------------------------------------------------------------- Worker

enum Cmd {
    State {
        known_art: Option<String>,
        reply: oneshot::Sender<Result<Snapshot, String>>,
    },
    Control {
        action: String,
        value: Option<serde_json::Value>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Volume {
        reply: oneshot::Sender<Result<Option<f32>, String>>,
    },
    SetVolume {
        level: f32,
        reply: oneshot::Sender<Result<f32, String>>,
    },
}

pub struct NowPlayingState {
    tx: mpsc::UnboundedSender<Cmd>,
}

pub fn setup(app: &AppHandle) -> Result<(), String> {
    let (tx, rx) = mpsc::unbounded_channel();
    std::thread::Builder::new()
        .name("nowplaying".into())
        .spawn(move || {
            // MTA: SMTC & Core Audio aman dipanggil dari thread ini saja.
            unsafe {
                let _ = RoInitialize(RO_INIT_MULTITHREADED);
            }
            let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() else {
                return;
            };
            rt.block_on(worker(rx));
        })
        .map_err(|e| format!("Gagal memulai pemantau media: {e}"))?;
    app.manage(NowPlayingState { tx });
    Ok(())
}

async fn worker(mut rx: mpsc::UnboundedReceiver<Cmd>) {
    let mut manager: Option<GlobalSystemMediaTransportControlsSessionManager> = None;
    let mut art = ArtCache::default();
    while let Some(cmd) = rx.recv().await {
        match cmd {
            Cmd::State { known_art, reply } => {
                let _ = reply.send(collect(&mut manager, &mut art, known_art).await);
            }
            Cmd::Control { action, value, reply } => {
                let _ = reply.send(control(manager.as_ref(), &action, value).await);
            }
            Cmd::Volume { reply } => {
                let _ = reply.send(Ok(volume_get()));
            }
            Cmd::SetVolume { level, reply } => {
                let _ = reply.send(volume_set(level));
            }
        }
    }
}

async fn manager(
    slot: &mut Option<GlobalSystemMediaTransportControlsSessionManager>,
) -> Result<GlobalSystemMediaTransportControlsSessionManager, String> {
    if let Some(m) = slot.as_ref() {
        return Ok(m.clone());
    }
    let m = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .map_err(|e| format!("Windows Media Control tidak tersedia: {e}"))?
        .await
        .map_err(|e| format!("Gagal membaca sesi media: {e}"))?;
    *slot = Some(m.clone());
    Ok(m)
}

fn is_playing(s: &GlobalSystemMediaTransportControlsSession) -> bool {
    s.GetPlaybackInfo()
        .ok()
        .and_then(|i| i.PlaybackStatus().ok())
        .map(|st| st == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
        .unwrap_or(false)
}

/// Pilih sesi Spotify yang sedang main lebih dulu; kalau tidak ada, sesi media lain.
fn pick(m: &GlobalSystemMediaTransportControlsSessionManager) -> Option<GlobalSystemMediaTransportControlsSession> {
    let Ok(list) = m.GetSessions() else {
        return m.GetCurrentSession().ok();
    };
    let count = list.Size().unwrap_or(0);
    let mut spotify_playing = None;
    let mut spotify_any = None;
    let mut playing_any = None;
    for i in 0..count {
        let Ok(s) = list.GetAt(i) else { continue };
        let aumid = s
            .SourceAppUserModelId()
            .map(|h| h.to_string().to_lowercase())
            .unwrap_or_default();
        let playing = is_playing(&s);
        if aumid.contains("spotify") {
            if playing && spotify_playing.is_none() {
                spotify_playing = Some(s.clone());
            }
            if spotify_any.is_none() {
                spotify_any = Some(s.clone());
            }
        }
        if playing && playing_any.is_none() {
            playing_any = Some(s);
        }
    }
    spotify_playing
        .or(spotify_any)
        .or(playing_any)
        .or_else(|| m.GetCurrentSession().ok())
}

async fn collect(
    slot: &mut Option<GlobalSystemMediaTransportControlsSessionManager>,
    art: &mut ArtCache,
    known_art: Option<String>,
) -> Result<Snapshot, String> {
    let Ok(m) = manager(slot).await else {
        return Ok(Snapshot::default());
    };
    let Some(session) = pick(&m) else {
        return Ok(Snapshot::default());
    };

    let app = session
        .SourceAppUserModelId()
        .map(|h| h.to_string())
        .unwrap_or_default();

    let props = match session.TryGetMediaPropertiesAsync() {
        Ok(op) => match op.await {
            Ok(p) => p,
            Err(_) => return Ok(Snapshot::default()),
        },
        Err(_) => return Ok(Snapshot::default()),
    };

    let title = props.Title().map(|h| h.to_string()).unwrap_or_default();
    let artist = props.Artist().map(|h| h.to_string()).unwrap_or_default();
    let album = props.AlbumTitle().map(|h| h.to_string()).unwrap_or_default();

    let info = session.GetPlaybackInfo().ok();
    let status = info.as_ref().and_then(|i| i.PlaybackStatus().ok());
    let playing = status == Some(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing);
    let controls = info.as_ref().and_then(|i| i.Controls().ok());
    let flag = |f: fn(&windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackControls) -> windows::core::Result<bool>| {
        controls.as_ref().and_then(|c| f(c).ok()).unwrap_or(true)
    };

    let shuffle = info
        .as_ref()
        .and_then(|i| i.IsShuffleActive().ok())
        .and_then(|r| r.Value().ok())
        .unwrap_or(false);
    let repeat = info
        .as_ref()
        .and_then(|i| i.AutoRepeatMode().ok())
        .and_then(|r| r.Value().ok())
        .map(|m: MediaPlaybackAutoRepeatMode| match m.0 {
            1 => "track",
            2 => "list",
            _ => "off",
        })
        .unwrap_or("off")
        .to_string();

    let timeline = session.GetTimelineProperties().ok();
    let (position_ms, duration_ms) = timeline
        .map(|t| {
            let start = t.StartTime().map(|v| v.Duration).unwrap_or(0);
            let end = t.EndTime().map(|v| v.Duration).unwrap_or(0);
            let pos = t.Position().map(|v| v.Duration).unwrap_or(0);
            (ticks_to_ms(pos - start), ticks_to_ms(end - start))
        })
        .unwrap_or((0, 0));

    let art_key = format!("{app}|{title}|{artist}|{album}");
    let art_uri = if title.is_empty() {
        None
    } else if known_art.as_deref() == Some(art_key.as_str()) {
        art.get(&art_key)
    } else {
        match art.get(&art_key) {
            Some(v) => Some(v),
            None => match download_art(&props).await {
                Some(uri) => {
                    art.put(art_key.clone(), uri.clone());
                    Some(uri)
                }
                None => None,
            },
        }
    };

    // `knownArt` sama -> jangan kirim ulang data URI (bisa ratusan KB tiap detik).
    let send_art = if known_art.as_deref() == Some(art_key.as_str()) { None } else { art_uri };

    Ok(Snapshot {
        available: true,
        app,
        title,
        artist,
        album,
        playing,
        position_ms,
        duration_ms,
        shuffle,
        repeat,
        can_play: flag(|c| c.IsPlayEnabled()),
        can_pause: flag(|c| c.IsPauseEnabled()),
        can_next: flag(|c| c.IsNextEnabled()),
        can_prev: flag(|c| c.IsPreviousEnabled()),
        can_seek: flag(|c| c.IsPlaybackPositionEnabled()),
        can_shuffle: flag(|c| c.IsShuffleEnabled()),
        can_repeat: flag(|c| c.IsRepeatEnabled()),
        art_key,
        art: send_art,
    })
}

fn ticks_to_ms(ticks: i64) -> u64 {
    if ticks <= 0 {
        0
    } else {
        (ticks as u64) / 10_000
    }
}

async fn download_art(props: &GlobalSystemMediaTransportControlsSessionMediaProperties) -> Option<String> {
    let thumbs = props.Thumbnail().ok()?;
    let stream = thumbs.OpenReadAsync().ok()?.await.ok()?;
    let size = stream.Size().ok()?;
    if size == 0 || size > MAX_ART_BYTES {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    let loaded = reader.LoadAsync(size as u32).ok()?.await.ok()?;
    let mut buf = vec![0u8; loaded as usize];
    reader.ReadBytes(&mut buf).ok()?;
    let _ = reader.Close();
    let mime = sniff(&buf);
    Some(format!("data:{mime};base64,{}", STANDARD.encode(&buf)))
}

fn sniff(b: &[u8]) -> &'static str {
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if b.starts_with(b"\x89PNG") {
        "image/png"
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        "image/webp"
    } else if b.starts_with(b"GIF8") {
        "image/gif"
    } else {
        "image/jpeg"
    }
}

#[derive(Default)]
struct ArtCache {
    map: HashMap<String, String>,
    order: VecDeque<String>,
}

impl ArtCache {
    fn get(&self, key: &str) -> Option<String> {
        self.map.get(key).cloned()
    }

    fn put(&mut self, key: String, uri: String) {
        if self.map.insert(key.clone(), uri).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > ART_CACHE {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
    }
}

// ---------------------------------------------------------------- Kontrol

async fn control(
    manager: Option<&GlobalSystemMediaTransportControlsSessionManager>,
    action: &str,
    value: Option<serde_json::Value>,
) -> Result<(), String> {
    let Some(m) = manager else {
        return Err("Pemantau media belum siap".into());
    };
    let Some(s) = pick(m) else {
        return Err("Tidak ada sesi media yang aktif. Buka Spotify dulu.".into());
    };
    let op = match action {
        "play" => s.TryPlayAsync(),
        "pause" => s.TryPauseAsync(),
        "toggle" => {
            if is_playing(&s) {
                s.TryPauseAsync()
            } else {
                s.TryPlayAsync()
            }
        }
        "next" => s.TrySkipNextAsync(),
        "prev" => s.TrySkipPreviousAsync(),
        "shuffle" => {
            let on = value.and_then(|v| v.as_bool()).unwrap_or(false);
            return finish!(s.TryChangeShuffleActiveAsync(on));
        }
        "repeat" => {
            let mode = match value.and_then(|v| v.as_str().map(str::to_string)) {
                Some(m) if m == "track" => MediaPlaybackAutoRepeatMode::Track,
                Some(m) if m == "list" => MediaPlaybackAutoRepeatMode::List,
                _ => MediaPlaybackAutoRepeatMode::None,
            };
            return finish!(s.TryChangeAutoRepeatModeAsync(mode));
        }
        "seek" => {
            let ms = value.and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0) as i64;
            let ticks = ms.saturating_mul(10_000);
            return finish!(s.TryChangePlaybackPositionAsync(ticks));
        }
        other => return Err(format!("Perintah tidak dikenal: {other}")),
    };
    finish!(op)
}



// ---------------------------------------------------------------- Volume (Core Audio)

fn volume_get() -> Option<f32> {
    unsafe {
        if let Some(v) = session_volume(|v| v.GetMasterVolume()) {
            return Some(v.clamp(0.0, 1.0));
        }
        endpoint_volume()?.GetMasterVolumeLevelScalar().ok().map(|v| v.clamp(0.0, 1.0))
    }
}

fn volume_set(level: f32) -> Result<f32, String> {
    let level = level.clamp(0.0, 1.0);
    unsafe {
        if let Some(()) = session_volume(|v| v.SetMasterVolume(level, std::ptr::null())) {
            return Ok(level);
        }
        let ep = endpoint_volume().ok_or("Tidak menemukan perangkat audio aktif")?;
        ep.SetMasterVolumeLevelScalar(level, std::ptr::null())
            .map_err(|e| format!("Gagal mengatur volume sistem: {e}"))?;
        Ok(level)
    }
}

unsafe fn session_volume<T>(
    f: impl FnOnce(&ISimpleAudioVolume) -> windows::core::Result<T>,
) -> Option<T> {
    let pid = spotify_pid()?;
    let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
    let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia).ok()?;
    let mgr: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None).ok()?;
    let sessions = mgr.GetSessionEnumerator().ok()?;
    let count = sessions.GetCount().ok()?;
    for i in 0..count as i32 {
        let Ok(control) = sessions.GetSession(i) else { continue };
        let Ok(control2) = control.cast::<IAudioSessionControl2>() else { continue };
        if control2.GetProcessId().ok() != Some(pid) {
            continue;
        }
        let Ok(vol) = control.cast::<ISimpleAudioVolume>() else { continue };
        return f(&vol).ok();
    }
    None
}

unsafe fn endpoint_volume() -> Option<IAudioEndpointVolume> {
    let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
    let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia).ok()?;
    device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None).ok()
}

fn spotify_pid() -> Option<u32> {
    let Ok(mut sys) = sysinfo::System::new() else {
        return None;
    };
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes().iter().find_map(|(pid, p)| {
        let name = p.name().to_string_lossy().to_lowercase();
        name.starts_with("spotify").then(|| pid.as_u32())
    })
}

// ---------------------------------------------------------------- Command Tauri

async fn send<T>(
    app: &AppHandle,
    make: impl FnOnce(oneshot::Sender<Result<T, String>>) -> Cmd,
) -> Result<T, String> {
    let tx = app.state::<NowPlayingState>().tx.clone();
    let (reply, rx) = oneshot::channel();
    tx.send(make(reply))
        .map_err(|_| "Pemantau media tidak berjalan".to_string())?;
    rx.await.map_err(|_| "Pemantau media berhenti".to_string())?
}

#[tauri::command]
pub async fn nowplaying_state(app: AppHandle, known_art: Option<String>) -> Result<Snapshot, String> {
    send(&app, |reply| Cmd::State { known_art, reply }).await
}

#[tauri::command]
pub async fn nowplaying_control(
    app: AppHandle,
    action: String,
    value: Option<serde_json::Value>,
) -> Result<(), String> {
    send(&app, |reply| Cmd::Control { action, value, reply }).await
}

#[tauri::command]
pub async fn nowplaying_volume(app: AppHandle) -> Result<Option<f32>, String> {
    send(&app, |reply| Cmd::Volume { reply }).await
}

#[tauri::command]
pub async fn nowplaying_set_volume(app: AppHandle, level: f32) -> Result<f32, String> {
    send(&app, |reply| Cmd::SetVolume { level, reply }).await
}

#[tauri::command]
pub fn nowplaying_open_uri(uri: String) -> Result<(), String> {
    let uri = uri.trim();
    let allowed = uri == "spotify:"
        || uri.starts_with("spotify:playlist:")
        || uri.starts_with("spotify:album:")
        || uri.starts_with("spotify:track:")
        || uri.starts_with("spotify:artist:")
        || uri.starts_with("spotify:episode:")
        || uri.starts_with("spotify:show:")
        || uri.starts_with("spotify:search:")
        || uri.starts_with("spotify:collection")
        || uri.starts_with("https://open.spotify.com/");
    if !allowed {
        return Err("Tautan itu bukan tautan Spotify yang dikenal".into());
    }
    open::that_detached(uri).map_err(|e| format!("Gagal membuka Spotify: {e}"))
}
