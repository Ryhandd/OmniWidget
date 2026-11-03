//! Plugin Spotify (kontrol pemutaran lewat Web API, Authorization Code + PKCE, tanpa client secret).
//!
//! Pembagian tugas:
//! - `pkce`     : verifier/challenge/state + URL otorisasi (murni).
//! - `callback` : server loopback sekali pakai penerima redirect OAuth.
//! - `api`      : request/respons Spotify (murni di sisi pembangunan & penguraian).
//! - berkas ini : glue Tauri: state, penyimpanan token, refresh otomatis, dan command IPC.
//!
//! Keamanan (selaras `plugins/llm`): semua jaringan lewat Rust (CSP WebView hanya `ipc:`); refresh token
//! disimpan di kv berprefiks `secret.` (ditolak oleh `kv_*` dari WebView); access token HANYA di memori
//! dan tidak pernah dikirim ke UI. Login menahan Deep Sleep (`AwakeGuard`) selama menunggu browser.

pub mod api;
pub mod callback;
pub mod error;
pub mod pkce;

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{Client, Url};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tokio::{net::TcpListener, sync::Notify};

use crate::core::{
    state::AwakeGuard,
    storage::{blocking, Storage},
};
use error::SpotifyError;

const KEY_CLIENT_ID: &str = "spotify.client_id";
const KEY_REFRESH: &str = "secret.spotify_refresh_token";

const LOGIN_TIMEOUT: Duration = Duration::from_secs(180);
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
/// Perbarui access token bila sisa umurnya kurang dari ini.
const EXPIRY_SKEW: Duration = Duration::from_secs(60);
const MAX_COVER_BYTES: usize = 1024 * 1024;

struct Access {
    token: String,
    expires_at: Instant,
}

pub struct SpotifyState {
    http: Client,
    /// Mutex async ditahan selama refresh: dua pemanggil bersamaan tidak memicu dua refresh
    /// (refresh token bisa dirotasi, jadi refresh ganda berisiko mencabut sesi).
    access: tokio::sync::Mutex<Option<Access>>,
    /// Ada login berjalan? Menyimpan pemicu pembatalannya. Maksimal satu login sekaligus.
    login: Mutex<Option<Arc<Notify>>>,
    /// Satu entri cache sampul: (url, data URI). Polling tidak mengunduh ulang sampul yang sama.
    cover: Mutex<Option<(String, String)>>,
}

impl SpotifyState {
    fn new() -> Result<Self, String> {
        // HTTPS saja, tanpa redirect: header Authorization tidak boleh ikut ke host lain.
        let http = Client::builder()
            .timeout(HTTP_TIMEOUT)
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| format!("Gagal menyiapkan klien HTTP Spotify: {e}"))?;
        Ok(Self { http, access: tokio::sync::Mutex::new(None), login: Mutex::new(None), cover: Mutex::new(None) })
    }
}

pub fn setup(app: &AppHandle) -> Result<(), String> {
    app.manage(SpotifyState::new()?);
    Ok(())
}

// ---------- Status ----------

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub client_id: Option<String>,
    /// Ada refresh token tersimpan (tidak memanggil jaringan).
    pub connected: bool,
    /// Redirect URI yang harus didaftarkan persis di dashboard Spotify.
    pub redirect_uri: &'static str,
}

async fn store<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, SpotifyError> {
    blocking(f).await.map_err(SpotifyError::Storage)
}

fn storage(app: &AppHandle) -> Storage {
    app.state::<Storage>().inner().clone()
}

async fn load_status(app: &AppHandle) -> Result<Status, SpotifyError> {
    let s = storage(app);
    let (client_id, refresh) = store(move || Ok((s.get(KEY_CLIENT_ID)?, s.get(KEY_REFRESH)?))).await?;
    Ok(Status {
        client_id: client_id.filter(|c| pkce::is_client_id(c)),
        connected: refresh.map_or(false, |t| !t.trim().is_empty()),
        redirect_uri: pkce::REDIRECT_URI,
    })
}

async fn forget_session(app: &AppHandle) -> Result<(), SpotifyError> {
    let s = storage(app);
    store(move || s.delete(KEY_REFRESH)).await?;
    *app.state::<SpotifyState>().access.lock().await = None;
    Ok(())
}

// ---------- Token ----------

fn remember(guard: &mut Option<Access>, t: &api::Tokens) {
    *guard = Some(Access { token: t.access.clone(), expires_at: Instant::now() + Duration::from_secs(t.expires_in) });
}

/// Access token yang masih berlaku; memperbarui lewat refresh token bila perlu (`force` = abaikan cache).
async fn bearer(app: &AppHandle, force: bool) -> Result<String, SpotifyError> {
    let st = app.state::<SpotifyState>();
    let mut guard = st.access.lock().await;
    if !force {
        if let Some(a) = guard.as_ref() {
            if a.expires_at > Instant::now() + EXPIRY_SKEW {
                return Ok(a.token.clone());
            }
        }
    }
    let s = storage(app);
    let (client_id, refresh) = store(move || Ok((s.get(KEY_CLIENT_ID)?, s.get(KEY_REFRESH)?))).await?;
    let client_id = client_id.filter(|c| pkce::is_client_id(c)).ok_or(SpotifyError::NoClientId)?;
    let refresh = refresh.filter(|t| !t.trim().is_empty()).ok_or(SpotifyError::NotConnected)?;

    match api::refresh(&st.http, &client_id, &refresh).await {
        Ok(t) => {
            if let Some(new_rt) = t.refresh.clone() {
                let s = storage(app);
                store(move || s.set(KEY_REFRESH, &new_rt)).await?;
            }
            remember(&mut guard, &t);
            Ok(t.access)
        }
        Err(SpotifyError::SessionExpired) => {
            // Refresh token mati (dicabut / kedaluwarsa): buang agar UI kembali ke layar "Hubungkan".
            *guard = None;
            drop(guard);
            forget_session(app).await?;
            Err(SpotifyError::SessionExpired)
        }
        Err(e) => Err(e),
    }
}

/// Panggil Web API; pada 401 perbarui token SATU kali lalu ulangi.
async fn call(app: &AppHandle, req: &api::Req) -> Result<api::Reply, SpotifyError> {
    let http = app.state::<SpotifyState>().http.clone();
    let token = bearer(app, false).await?;
    match api::send(&http, &token, req).await {
        Err(SpotifyError::Unauthorized) => {
            let token = bearer(app, true).await?;
            match api::send(&http, &token, req).await {
                Err(SpotifyError::Unauthorized) => Err(SpotifyError::SessionExpired),
                other => other,
            }
        }
        other => other,
    }
}

// ---------- Commands: pengaturan & sesi ----------

#[tauri::command]
pub async fn spotify_status(app: AppHandle) -> Result<Status, SpotifyError> {
    load_status(&app).await
}

/// String kosong = hapus Client ID. Mengganti Client ID memutus sesi (token terikat ke client-nya).
#[tauri::command]
pub async fn spotify_set_client_id(app: AppHandle, client_id: String) -> Result<Status, SpotifyError> {
    let id = client_id.trim().to_ascii_lowercase();
    if !id.is_empty() && !pkce::is_client_id(&id) {
        return Err(SpotifyError::BadClientId);
    }
    let s = storage(&app);
    let new_id = id.clone();
    let changed = store(move || {
        let old = s.get(KEY_CLIENT_ID)?;
        if new_id.is_empty() {
            s.delete(KEY_CLIENT_ID)?;
        } else {
            s.set(KEY_CLIENT_ID, &new_id)?;
        }
        Ok(old.as_deref() != Some(new_id.as_str()))
    })
    .await?;
    if changed {
        forget_session(&app).await?;
    }
    load_status(&app).await
}

struct LoginSlot<'a>(&'a SpotifyState);

impl Drop for LoginSlot<'_> {
    fn drop(&mut self) {
        if let Ok(mut g) = self.0.login.lock() {
            *g = None;
        }
    }
}

/// Login penuh: buka browser -> tunggu callback -> tukar kode -> simpan refresh token.
/// Perintah ini baru kembali setelah selesai/gagal/dibatalkan; WebView tetap hidup berkat `AwakeGuard`.
#[tauri::command]
pub async fn spotify_login(app: AppHandle) -> Result<Status, SpotifyError> {
    let st = app.state::<SpotifyState>();
    let client_id = load_status(&app).await?.client_id.ok_or(SpotifyError::NoClientId)?;

    let cancel = Arc::new(Notify::new());
    {
        let mut slot = st.login.lock().map_err(|_| SpotifyError::Api("State login rusak".into()))?;
        if slot.is_some() {
            return Err(SpotifyError::Invalid("login sedang berjalan".into()));
        }
        *slot = Some(cancel.clone());
    }
    let _slot = LoginSlot(&st);
    let _awake = AwakeGuard::acquire(&app);

    let flow = pkce::Pkce::generate()?;

    // Bind SEBELUM membuka browser: bila port bentrok, gagal cepat tanpa membuang waktu pengguna.
    let listener = TcpListener::bind(("127.0.0.1", pkce::REDIRECT_PORT))
        .await
        .map_err(|_| SpotifyError::PortBusy(pkce::REDIRECT_PORT))?;

    let url = pkce::authorize_url(&client_id, &flow.challenge, &flow.state);
    open::that_detached(&url).map_err(|e| SpotifyError::Browser(e.to_string()))?;

    let code = callback::wait_for_code(listener, &flow.state, &cancel, LOGIN_TIMEOUT).await?;
    let tokens = api::exchange_code(&st.http, &client_id, &code, &flow.verifier, pkce::REDIRECT_URI).await?;
    let refresh = tokens.refresh.clone().ok_or_else(|| SpotifyError::Api("Spotify tidak mengembalikan refresh token".into()))?;

    let s = storage(&app);
    store(move || s.set(KEY_REFRESH, &refresh)).await?;
    remember(&mut *st.access.lock().await, &tokens);
    load_status(&app).await
}

#[tauri::command]
pub fn spotify_login_cancel(app: AppHandle) -> bool {
    let st = app.state::<SpotifyState>();
    let cancel = st.login.lock().ok().and_then(|g| g.clone());
    match cancel {
        Some(n) => {
            n.notify_one();
            true
        }
        None => false,
    }
}

#[tauri::command]
pub async fn spotify_logout(app: AppHandle) -> Result<Status, SpotifyError> {
    forget_session(&app).await?;
    load_status(&app).await
}

// ---------- Commands: pemutaran ----------

#[tauri::command]
pub async fn spotify_player(app: AppHandle) -> Result<Option<api::Player>, SpotifyError> {
    let r = call(&app, &api::player_req()).await?;
    Ok(api::parse_player(&r.body))
}

#[tauri::command]
pub async fn spotify_devices(app: AppHandle) -> Result<Vec<api::Device>, SpotifyError> {
    let r = call(&app, &api::devices_req()).await?;
    Ok(api::parse_devices(&r.body))
}

#[tauri::command]
pub async fn spotify_control(app: AppHandle, command: api::Control) -> Result<(), SpotifyError> {
    let req = api::control_req(&command)?;
    call(&app, &req).await.map(|_| ())
}

// ---------- Sampul album ----------
// CSP WebView sengaja `img-src 'self' data:`; sampul diunduh Rust lalu dikirim sebagai data URI.

/// Hanya CDN gambar Spotify, lewat HTTPS, tanpa userinfo/port kustom (mencegah SSRF lewat UI).
pub fn cover_url_ok(raw: &str) -> bool {
    let Ok(u) = Url::parse(raw) else { return false };
    let host = u.host_str().unwrap_or("");
    let allowed = host == "i.scdn.co" || host.ends_with(".scdn.co") || host.ends_with(".spotifycdn.com");
    u.scheme() == "https" && u.username().is_empty() && u.password().is_none() && u.port().is_none() && allowed
}

fn image_mime(ct: &str) -> Option<&'static str> {
    match ct.split(';').next().unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "image/jpeg" => Some("image/jpeg"),
        "image/png" => Some("image/png"),
        "image/webp" => Some("image/webp"),
        _ => None,
    }
}

#[tauri::command]
pub async fn spotify_cover(app: AppHandle, url: String) -> Result<String, SpotifyError> {
    if !cover_url_ok(&url) {
        return Err(SpotifyError::Invalid("URL sampul tidak diizinkan".into()));
    }
    let st = app.state::<SpotifyState>();
    if let Some((u, data)) = st.cover.lock().ok().and_then(|g| g.clone()) {
        if u == url {
            return Ok(data);
        }
    }
    let res = st.http.get(&url).send().await?;
    if !res.status().is_success() {
        return Err(SpotifyError::Api(format!("Sampul tidak dapat diunduh (HTTP {})", res.status().as_u16())));
    }
    let mime = res
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .and_then(image_mime)
        .ok_or_else(|| SpotifyError::Api("Format sampul tidak didukung".into()))?;
    if res.content_length().map_or(false, |n| n as usize > MAX_COVER_BYTES) {
        return Err(SpotifyError::Api("Sampul terlalu besar".into()));
    }
    let bytes = res.bytes().await?;
    if bytes.len() > MAX_COVER_BYTES {
        return Err(SpotifyError::Api("Sampul terlalu besar".into()));
    }
    let data = format!("data:{mime};base64,{}", STANDARD.encode(&bytes));
    if let Ok(mut g) = st.cover.lock() {
        *g = Some((url, data.clone()));
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_sampul_hanya_cdn_spotify_https() {
        assert!(cover_url_ok("https://i.scdn.co/image/ab67616d00001e02abc"));
        assert!(cover_url_ok("https://image-cdn-ak.spotifycdn.com/image/abc"));
        assert!(!cover_url_ok("http://i.scdn.co/image/abc"));
        assert!(!cover_url_ok("https://evil.com/i.scdn.co"));
        assert!(!cover_url_ok("https://i.scdn.co.evil.com/x"));
        assert!(!cover_url_ok("https://user:pw@i.scdn.co/x"));
        assert!(!cover_url_ok("https://i.scdn.co:8443/x"));
        assert!(!cover_url_ok("https://127.0.0.1/x"));
        assert!(!cover_url_ok("file:///etc/passwd"));
        assert!(!cover_url_ok("bukan url"));
    }

    #[test]
    fn mime_sampul() {
        assert_eq!(image_mime("image/jpeg"), Some("image/jpeg"));
        assert_eq!(image_mime("IMAGE/PNG; charset=binary"), Some("image/png"));
        assert_eq!(image_mime("image/svg+xml"), None, "SVG ditolak (bisa memuat skrip)");
        assert_eq!(image_mime("text/html"), None);
    }

    #[test]
    fn status_tidak_memuat_rahasia() {
        let s = Status { client_id: Some("a".repeat(32)), connected: true, redirect_uri: pkce::REDIRECT_URI };
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains("\"redirectUri\"") && j.contains("\"connected\":true"));
        assert!(!j.to_lowercase().contains("token"));
    }
}
