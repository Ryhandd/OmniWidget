//! Stub platform selain Windows: OmniWidget tetap bisa dibangun, fitur now-playing nonaktif.

use serde::Serialize;
use tauri::AppHandle;

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub available: bool,
    pub app: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub shuffle: bool,
    pub repeat: String,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_next: bool,
    pub can_prev: bool,
    pub can_seek: bool,
    pub can_shuffle: bool,
    pub can_repeat: bool,
    pub art_key: String,
    pub art: Option<String>,
}

pub fn setup(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

fn unsupported() -> String {
    "Now-playing Spotify hanya tersedia di Windows".into()
}

#[tauri::command]
pub async fn nowplaying_state(_known_art: Option<String>) -> Result<Snapshot, String> {
    Ok(Snapshot::default())
}

#[tauri::command]
pub async fn nowplaying_control(
    _app: AppHandle,
    _action: String,
    _value: Option<serde_json::Value>,
) -> Result<(), String> {
    Err(unsupported())
}

#[tauri::command]
pub async fn nowplaying_volume(_app: AppHandle) -> Result<Option<f32>, String> {
    Ok(None)
}

#[tauri::command]
pub async fn nowplaying_set_volume(_app: AppHandle, _level: f32) -> Result<f32, String> {
    Err(unsupported())
}

#[tauri::command]
pub fn nowplaying_open_uri(_uri: String) -> Result<(), String> {
    Err(unsupported())
}
