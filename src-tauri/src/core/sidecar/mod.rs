pub mod manifest;
pub mod download;
pub mod extract;
pub mod commands;

use tauri::AppHandle;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SidecarState {
    pub app_data_dir: PathBuf,
    pub active_downloads: Arc<Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
}

impl SidecarState {
    pub fn new(app: AppHandle) -> Result<Self, String> {
        use tauri::Manager;
        let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&app_data_dir).map_err(|e| e.to_string())?;
        Ok(Self {
            app_data_dir,
            active_downloads: Arc::new(Mutex::new(std::collections::HashMap::new())),
        })
    }
}
