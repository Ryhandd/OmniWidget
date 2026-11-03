//! Klien tipis Spotify Accounts + Web API. Logika murni (membangun request, mengurai respons,
//! memetakan error) dipisah dari I/O sehingga seluruhnya teruji tanpa jaringan.

use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::error::SpotifyError;

pub const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
pub const API_BASE: &str = "https://api.spotify.com/v1";
const MAX_BODY_BYTES: usize = 1024 * 1024;

// ---------- Token ----------

#[derive(Debug, PartialEq, Eq)]
pub struct Tokens {
    pub access: String,
    pub expires_in: u64,
    /// Spotify boleh merotasi refresh token; bila ada, WAJIB menggantikan yang lama.
    pub refresh: Option<String>,
}

pub fn parse_tokens(body: &str) -> Result<Tokens, SpotifyError> {
    let v: Value = serde_json::from_str(body).map_err(|_| SpotifyError::Api("Respons token bukan JSON valid".into()))?;
    let access = v
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| SpotifyError::Api("Respons token tidak memuat access_token".into()))?
        .to_string();
    Ok(Tokens {
        access,
        expires_in: v.get("expires_in").and_then(Value::as_u64).unwrap_or(3600),
        refresh: v.get("refresh_token").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string),
    })
}

/// `invalid_grant` = refresh token/kode kedaluwarsa, dicabut, atau milik client lain -> login ulang.
pub fn token_error(status: u16, body: &str) -> SpotifyError {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let code = v.get("error").and_then(Value::as_str).unwrap_or("");
    let desc = v.get("error_description").and_then(Value::as_str).unwrap_or("");
    match (status, code) {
        (_, "invalid_grant") => SpotifyError::SessionExpired,
        (_, "invalid_client") => SpotifyError::Api(
            "Client ID ditolak Spotify. Periksa Client ID dan pastikan Redirect URI terdaftar persis".into(),
        ),
        (429, _) => SpotifyError::RateLimited(5),
        _ => {
            let d: String = desc.chars().take(200).collect();
            SpotifyError::Api(if d.is_empty() { format!("Spotify menolak permintaan token (HTTP {status})") } else { format!("Spotify: {d} (HTTP {status})") })
        }
    }
}

async fn post_token(http: &Client, form: &[(&str, &str)]) -> Result<Tokens, SpotifyError> {
    let res = http.post(TOKEN_URL).form(form).send().await?;
    let status = res.status();
    let body = read_body(res).await?;
    if status.is_success() { parse_tokens(&body) } else { Err(token_error(status.as_u16(), &body)) }
}

pub async fn exchange_code(http: &Client, client_id: &str, code: &str, verifier: &str, redirect_uri: &str) -> Result<Tokens, SpotifyError> {
    post_token(
        http,
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("code_verifier", verifier),
        ],
    )
    .await
}

pub async fn refresh(http: &Client, client_id: &str, refresh_token: &str) -> Result<Tokens, SpotifyError> {
    post_token(http, &[("grant_type", "refresh_token"), ("refresh_token", refresh_token), ("client_id", client_id)]).await
}

// ---------- Request Web API ----------

#[derive(Debug, PartialEq)]
pub struct Req {
    pub method: Method,
    pub path: &'static str,
    pub query: Vec<(&'static str, String)>,
    pub body: Option<Value>,
}

impl Req {
    fn new(method: Method, path: &'static str) -> Self {
        Self { method, path, query: Vec::new(), body: None }
    }
    fn q(mut self, k: &'static str, v: impl ToString) -> Self {
        self.query.push((k, v.to_string()));
        self
    }
}

pub struct Reply {
    pub status: u16,
    pub body: String,
}

pub fn player_req() -> Req {
    // `additional_types=episode`: tanpa ini podcast datang sebagai item null.
    Req::new(Method::GET, "/me/player").q("additional_types", "episode")
}

pub fn devices_req() -> Req {
    Req::new(Method::GET, "/me/player/devices")
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    Off,
    Context,
    Track,
}

impl RepeatMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Context => "context",
            Self::Track => "track",
        }
    }
}

/// Perintah kontrol dari UI. Field di dalamnya snake_case (mengikuti konvensi payload modul lain).
#[derive(Deserialize, Debug)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Control {
    Play,
    Pause,
    Next,
    Previous,
    Seek { position_ms: u64 },
    Volume { percent: u8 },
    Shuffle { on: bool },
    Repeat { mode: RepeatMode },
    Transfer { device_id: String, play: bool },
}

fn valid_device_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 100 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn control_req(c: &Control) -> Result<Req, SpotifyError> {
    Ok(match c {
        Control::Play => Req::new(Method::PUT, "/me/player/play"),
        Control::Pause => Req::new(Method::PUT, "/me/player/pause"),
        Control::Next => Req::new(Method::POST, "/me/player/next"),
        Control::Previous => Req::new(Method::POST, "/me/player/previous"),
        Control::Seek { position_ms } => {
            // Batas wajar (24 jam) menolak nilai tak masuk akal sebelum menyentuh jaringan.
            if *position_ms > 24 * 60 * 60 * 1000 {
                return Err(SpotifyError::Invalid("posisi di luar batas".into()));
            }
            Req::new(Method::PUT, "/me/player/seek").q("position_ms", position_ms)
        }
        Control::Volume { percent } => {
            if *percent > 100 {
                return Err(SpotifyError::Invalid("volume harus 0-100".into()));
            }
            Req::new(Method::PUT, "/me/player/volume").q("volume_percent", percent)
        }
        Control::Shuffle { on } => Req::new(Method::PUT, "/me/player/shuffle").q("state", on),
        Control::Repeat { mode } => Req::new(Method::PUT, "/me/player/repeat").q("state", mode.as_str()),
        Control::Transfer { device_id, play } => {
            if !valid_device_id(device_id) {
                return Err(SpotifyError::Invalid("ID perangkat tidak valid".into()));
            }
            let mut r = Req::new(Method::PUT, "/me/player");
            r.body = Some(json!({ "device_ids": [device_id], "play": play }));
            r
        }
    })
}

/// Kirim satu request. Tidak ada retry/refresh di sini: itu urusan pemanggil (mod.rs).
pub async fn send(http: &Client, token: &str, req: &Req) -> Result<Reply, SpotifyError> {
    let url = format!("{API_BASE}{}", req.path);
    let mut b = http.request(req.method.clone(), &url).bearer_auth(token);
    if !req.query.is_empty() {
        b = b.query(&req.query);
    }
    b = match &req.body {
        Some(v) => b.header("content-type", "application/json").body(v.to_string()),
        // Body kosong eksplisit: Spotify menolak PUT/POST tanpa Content-Length (HTTP 411).
        None if req.method != Method::GET => b.body(Vec::<u8>::new()),
        None => b,
    };
    let res = b.send().await?;
    let status = res.status().as_u16();
    let retry_after = res.headers().get("retry-after").and_then(|h| h.to_str().ok()).and_then(|s| s.trim().parse::<u64>().ok());
    let body = read_body(res).await?;
    if (200..300).contains(&status) { Ok(Reply { status, body }) } else { Err(api_error(status, retry_after, &body)) }
}

async fn read_body(res: reqwest::Response) -> Result<String, SpotifyError> {
    if res.content_length().map_or(false, |n| n as usize > MAX_BODY_BYTES) {
        return Err(SpotifyError::Api("Respons Spotify terlalu besar".into()));
    }
    let bytes = res.bytes().await?;
    if bytes.len() > MAX_BODY_BYTES {
        return Err(SpotifyError::Api("Respons Spotify terlalu besar".into()));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn api_error(status: u16, retry_after: Option<u64>, body: &str) -> SpotifyError {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let reason = v.pointer("/error/reason").and_then(Value::as_str).unwrap_or("");
    let msg: String = v.pointer("/error/message").and_then(Value::as_str).unwrap_or("").chars().take(200).collect();
    match (status, reason) {
        (401, _) => SpotifyError::Unauthorized,
        (_, "PREMIUM_REQUIRED") => SpotifyError::PremiumRequired,
        (_, "NO_ACTIVE_DEVICE") => SpotifyError::NoActiveDevice,
        (404, _) if msg.to_ascii_lowercase().contains("no active device") => SpotifyError::NoActiveDevice,
        (403, _) => SpotifyError::Forbidden,
        (429, _) => SpotifyError::RateLimited(retry_after.unwrap_or(5).clamp(1, 300)),
        _ if msg.is_empty() => SpotifyError::Api(format!("Spotify menolak permintaan (HTTP {status})")),
        _ => SpotifyError::Api(format!("Spotify: {msg} (HTTP {status})")),
    }
}

// ---------- Model respons (hanya yang dibutuhkan UI) ----------

#[derive(Serialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: Option<String>,
    pub name: String,
    pub kind: String,
    pub is_active: bool,
    pub volume_percent: Option<u8>,
    pub supports_volume: bool,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    /// "track" | "episode"
    pub kind: String,
    pub name: String,
    pub artists: String,
    pub album: String,
    pub duration_ms: u64,
    pub image_url: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub is_playing: bool,
    pub progress_ms: u64,
    pub shuffle: bool,
    /// "off" | "context" | "track"
    pub repeat: String,
    pub device: Option<Device>,
    pub item: Option<Track>,
}

fn parse_device(v: &Value) -> Option<Device> {
    let name = v.get("name").and_then(Value::as_str)?.to_string();
    Some(Device {
        id: v.get("id").and_then(Value::as_str).map(str::to_string),
        name,
        kind: v.get("type").and_then(Value::as_str).unwrap_or("Unknown").to_string(),
        is_active: v.get("is_active").and_then(Value::as_bool).unwrap_or(false),
        volume_percent: v.get("volume_percent").and_then(Value::as_u64).map(|n| n.min(100) as u8),
        supports_volume: v.get("supports_volume").and_then(Value::as_bool).unwrap_or(true),
    })
}

/// Pilih sampul terkecil yang lebarnya >= 200 px (hemat byte lewat IPC), cadangannya yang terbesar.
fn pick_image(images: Option<&Value>) -> Option<String> {
    let list: Vec<(u64, &str)> = images?
        .as_array()?
        .iter()
        .filter_map(|i| Some((i.get("width").and_then(Value::as_u64).unwrap_or(0), i.get("url")?.as_str()?)))
        .collect();
    list.iter()
        .filter(|(w, _)| *w >= 200)
        .min_by_key(|(w, _)| *w)
        .or_else(|| list.iter().max_by_key(|(w, _)| *w))
        .map(|(_, u)| u.to_string())
}

fn parse_item(v: &Value) -> Option<Track> {
    if v.is_null() {
        return None;
    }
    let kind = v.get("type").and_then(Value::as_str).unwrap_or("track").to_string();
    let name = v.get("name").and_then(Value::as_str)?.to_string();
    let duration_ms = v.get("duration_ms").and_then(Value::as_u64).unwrap_or(0);
    if kind == "episode" {
        return Some(Track {
            kind,
            name,
            artists: v.pointer("/show/publisher").and_then(Value::as_str).unwrap_or("").to_string(),
            album: v.pointer("/show/name").and_then(Value::as_str).unwrap_or("").to_string(),
            duration_ms,
            image_url: pick_image(v.get("images")).or_else(|| pick_image(v.pointer("/show/images"))),
        });
    }
    let artists = v
        .get("artists")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.get("name").and_then(Value::as_str)).collect::<Vec<_>>().join(", "))
        .unwrap_or_default();
    Some(Track {
        kind,
        name,
        artists,
        album: v.pointer("/album/name").and_then(Value::as_str).unwrap_or("").to_string(),
        duration_ms,
        image_url: pick_image(v.pointer("/album/images")),
    })
}

/// Respons kosong (HTTP 204 = tidak ada sesi pemutaran) -> `None`.
pub fn parse_player(body: &str) -> Option<Player> {
    if body.trim().is_empty() {
        return None;
    }
    let v: Value = serde_json::from_str(body).ok()?;
    let repeat = match v.get("repeat_state").and_then(Value::as_str) {
        Some(s @ ("track" | "context")) => s.to_string(),
        _ => "off".to_string(),
    };
    Some(Player {
        is_playing: v.get("is_playing").and_then(Value::as_bool).unwrap_or(false),
        progress_ms: v.get("progress_ms").and_then(Value::as_u64).unwrap_or(0),
        shuffle: v.get("shuffle_state").and_then(Value::as_bool).unwrap_or(false),
        repeat,
        device: v.get("device").and_then(parse_device),
        item: v.get("item").and_then(parse_item),
    })
}

pub fn parse_devices(body: &str) -> Vec<Device> {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("devices").and_then(Value::as_array).map(|a| a.iter().filter_map(parse_device).collect()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAYER: &str = r#"{
        "device":{"id":"abc123","name":"Laptop","type":"Computer","is_active":true,"volume_percent":55,"supports_volume":true},
        "repeat_state":"context","shuffle_state":true,"progress_ms":42000,"is_playing":true,
        "item":{"type":"track","name":"Lagu","duration_ms":200000,
          "artists":[{"name":"A"},{"name":"B"}],
          "album":{"name":"Album","images":[{"url":"https://i.scdn.co/640","width":640},{"url":"https://i.scdn.co/300","width":300},{"url":"https://i.scdn.co/64","width":64}]}}
    }"#;

    #[test]
    fn token_sukses_dan_rotasi_refresh() {
        let t = parse_tokens(r#"{"access_token":"AT","token_type":"Bearer","expires_in":3600,"refresh_token":"RT","scope":"x"}"#).unwrap();
        assert_eq!(t, Tokens { access: "AT".into(), expires_in: 3600, refresh: Some("RT".into()) });
        let t = parse_tokens(r#"{"access_token":"AT"}"#).unwrap();
        assert_eq!((t.expires_in, t.refresh), (3600, None));
        assert!(parse_tokens(r#"{"refresh_token":"RT"}"#).is_err());
        assert!(parse_tokens("<html>").is_err());
    }

    #[test]
    fn error_token_invalid_grant_berarti_login_ulang() {
        assert!(matches!(token_error(400, r#"{"error":"invalid_grant","error_description":"Refresh token revoked"}"#), SpotifyError::SessionExpired));
        assert!(matches!(token_error(400, r#"{"error":"invalid_client"}"#), SpotifyError::Api(_)));
        assert!(matches!(token_error(503, ""), SpotifyError::Api(_)));
    }

    #[test]
    fn request_kontrol() {
        let r = control_req(&Control::Volume { percent: 40 }).unwrap();
        assert_eq!((r.method.clone(), r.path, r.query.clone()), (Method::PUT, "/me/player/volume", vec![("volume_percent", "40".to_string())]));
        let r = control_req(&Control::Shuffle { on: true }).unwrap();
        assert_eq!(r.query, vec![("state", "true".to_string())]);
        let r = control_req(&Control::Repeat { mode: RepeatMode::Track }).unwrap();
        assert_eq!(r.query, vec![("state", "track".to_string())]);
        assert_eq!(control_req(&Control::Next).unwrap().method, Method::POST);
        let r = control_req(&Control::Transfer { device_id: "abc_1-2".into(), play: true }).unwrap();
        assert_eq!(r.body.unwrap(), json!({"device_ids":["abc_1-2"],"play":true}));
        assert!(control_req(&Control::Volume { percent: 101 }).is_err());
        assert!(control_req(&Control::Seek { position_ms: u64::MAX }).is_err());
        assert!(control_req(&Control::Transfer { device_id: "a b&x=1".into(), play: false }).is_err());
        assert!(control_req(&Control::Transfer { device_id: "".into(), play: false }).is_err());
    }

    #[test]
    fn payload_kontrol_dari_ui_terurai() {
        let c: Control = serde_json::from_str(r#"{"action":"seek","position_ms":1500}"#).unwrap();
        assert!(matches!(c, Control::Seek { position_ms: 1500 }));
        let c: Control = serde_json::from_str(r#"{"action":"repeat","mode":"context"}"#).unwrap();
        assert!(matches!(c, Control::Repeat { mode: RepeatMode::Context }));
        assert!(serde_json::from_str::<Control>(r#"{"action":"hapus_semua"}"#).is_err());
        assert!(serde_json::from_str::<Control>(r#"{"action":"volume","percent":300}"#).is_err());
    }

    #[test]
    fn peta_error_api() {
        assert!(matches!(api_error(401, None, ""), SpotifyError::Unauthorized));
        assert!(matches!(api_error(403, None, r#"{"error":{"status":403,"message":"Player command failed: Premium required","reason":"PREMIUM_REQUIRED"}}"#), SpotifyError::PremiumRequired));
        assert!(matches!(api_error(404, None, r#"{"error":{"status":404,"message":"Player command failed: No active device found","reason":"NO_ACTIVE_DEVICE"}}"#), SpotifyError::NoActiveDevice));
        assert!(matches!(api_error(404, None, r#"{"error":{"message":"No active device found"}}"#), SpotifyError::NoActiveDevice));
        assert!(matches!(api_error(403, None, r#"{"error":{"message":"User not registered in the Developer Dashboard"}}"#), SpotifyError::Forbidden));
        assert!(matches!(api_error(429, Some(7), ""), SpotifyError::RateLimited(7)));
        assert!(matches!(api_error(429, Some(99999), ""), SpotifyError::RateLimited(300)));
        assert!(matches!(api_error(429, None, ""), SpotifyError::RateLimited(5)));
        assert!(matches!(api_error(500, None, "x"), SpotifyError::Api(_)));
    }

    #[test]
    fn parse_player_lengkap() {
        let p = parse_player(PLAYER).unwrap();
        assert!(p.is_playing && p.shuffle);
        assert_eq!((p.progress_ms, p.repeat.as_str()), (42000, "context"));
        let d = p.device.unwrap();
        assert_eq!((d.id.as_deref(), d.volume_percent, d.is_active), (Some("abc123"), Some(55), true));
        let t = p.item.unwrap();
        assert_eq!((t.name.as_str(), t.artists.as_str(), t.album.as_str(), t.duration_ms), ("Lagu", "A, B", "Album", 200000));
        assert_eq!(t.image_url.as_deref(), Some("https://i.scdn.co/300"), "terkecil yang >= 200px");
    }

    #[test]
    fn parse_player_kasus_tepi() {
        assert!(parse_player("").is_none(), "204 = tidak ada sesi");
        assert!(parse_player("bukan json").is_none());
        // iklan / sesi privat: item null, tetap valid
        let p = parse_player(r#"{"is_playing":true,"item":null,"repeat_state":"aneh"}"#).unwrap();
        assert!(p.item.is_none() && p.device.is_none());
        assert_eq!(p.repeat, "off");
        // podcast
        let p = parse_player(r#"{"item":{"type":"episode","name":"Eps","duration_ms":1,"images":[{"url":"https://i.scdn.co/e","width":64}],"show":{"name":"Acara","publisher":"Pub"}}}"#).unwrap();
        let t = p.item.unwrap();
        assert_eq!((t.kind.as_str(), t.artists.as_str(), t.album.as_str()), ("episode", "Pub", "Acara"));
        assert_eq!(t.image_url.as_deref(), Some("https://i.scdn.co/e"), "cadangan: terbesar bila tak ada >= 200");
    }

    #[test]
    fn parse_perangkat() {
        let d = parse_devices(r#"{"devices":[{"id":null,"name":"HP","type":"Smartphone","is_active":false,"volume_percent":300,"supports_volume":false},{"name":"PC"},{"type":"x"}]}"#);
        assert_eq!(d.len(), 2, "entri tanpa nama dibuang");
        assert_eq!((d[0].id.clone(), d[0].volume_percent, d[0].supports_volume), (None, Some(100), false));
        assert!(d[1].supports_volume, "default true bila field tidak ada");
        assert!(parse_devices("x").is_empty());
    }
}
