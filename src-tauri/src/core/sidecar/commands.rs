use tauri::{AppHandle, Manager, ipc::Channel};
use std::path::PathBuf;
use super::{SidecarState, manifest::parse_manifest, download::download_file, extract::extract_archive};

fn get_current_platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows-x86_64"
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") { "macos-aarch64" } else { "macos-x86_64" }
    } else {
        "linux-x86_64"
    }
}

pub fn resolve_dir(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let state = app.state::<SidecarState>();
    Ok(state.app_data_dir.join("sidecars").join(name))
}

#[tauri::command]
pub async fn sidecar_status(app: AppHandle, name: String) -> Result<bool, String> {
    let dir = resolve_dir(&app, &name)?;
    let ok_marker = dir.join(".ok");
    Ok(ok_marker.is_file())
}

#[tauri::command]
pub async fn sidecar_cancel(app: AppHandle, name: String) -> Result<(), String> {
    let state = app.state::<SidecarState>();
    let mut map = state.active_downloads.lock().await;
    if let Some(tx) = map.remove(&name) {
        let _ = tx.send(());
    }
    Ok(())
}

#[derive(serde::Serialize, Clone)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum InstallEvent {
    Started,
    Downloading { downloaded: u64, total: u64, speed: f64 },
    Verifying,
    Extracting,
    Installed,
    Failed { error: String },
    Cancelled,
}

#[tauri::command]
pub async fn sidecar_install(
    app: AppHandle,
    name: String,
    on_event: Channel<InstallEvent>,
) -> Result<(), String> {
    let _ = on_event.send(InstallEvent::Started);

    let manifest_str = include_str!("../../../sidecars.manifest.json");
    let manifest = parse_manifest(manifest_str)?;
    let entry = manifest.sidecars.get(&name).ok_or("Sidecar not found in manifest")?;
    if !entry.supported {
        return Err("Sidecar not supported on this platform".into());
    }

    let plat = get_current_platform();
    let asset = entry.platforms.get(plat).ok_or("Platform asset not found in manifest")?;

    let state = app.state::<SidecarState>();
    let downloads_dir = state.app_data_dir.join("sidecars").join(".downloads");
    tokio::fs::create_dir_all(&downloads_dir).await.map_err(|e| e.to_string())?;

    let part_file = downloads_dir.join(format!("{}-{}.part", name, uuid_v4_simple()));
    let target_dir = resolve_dir(&app, &name)?;

    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut map = state.active_downloads.lock().await;
        map.insert(name.clone(), tx);
    }

    // Closure progres memiliki salinannya sendiri; `on_event` tetap dipakai untuk status akhir.
    let progress_ch = on_event.clone();
    let dl_res = download_file(
        &asset.url,
        &part_file,
        &asset.sha256,
        move |d, t, s| {
            let _ = progress_ch.send(InstallEvent::Downloading { downloaded: d, total: t, speed: s });
        },
        rx,
    ).await;

    {
        let mut map = state.active_downloads.lock().await;
        map.remove(&name);
    }

    if let Err(e) = dl_res {
        let _ = tokio::fs::remove_file(&part_file).await;
        if e.contains("Cancelled") {
            let _ = on_event.send(InstallEvent::Cancelled);
        } else {
            let _ = on_event.send(InstallEvent::Failed { error: e.clone() });
        }
        return Err(e);
    }

    let _ = on_event.send(InstallEvent::Verifying);

    let _ = on_event.send(InstallEvent::Extracting);
    let staging_dir = downloads_dir.join(format!("staging-{}", uuid_v4_simple()));
    if let Err(e) = extract_archive(&part_file, &staging_dir).await {
        let _ = tokio::fs::remove_file(&part_file).await;
        let _ = tokio::fs::remove_dir_all(&staging_dir).await;
        let _ = on_event.send(InstallEvent::Failed { error: e.clone() });
        return Err(e);
    }

    let _ = tokio::fs::remove_file(&part_file).await;
    if target_dir.exists() {
        let _ = tokio::fs::remove_dir_all(&target_dir).await;
    }
    tokio::fs::create_dir_all(&target_dir).await.map_err(|e| e.to_string())?;

    let mut entries = tokio::fs::read_dir(&staging_dir).await.map_err(|e| e.to_string())?;
    while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
        let path = entry.path();
        let file_name = path.file_name().ok_or("Invalid filename")?;
        let dest_path = target_dir.join(file_name);
        tokio::fs::rename(&path, &dest_path).await.map_err(|e| e.to_string())?;
    }
    let _ = tokio::fs::remove_dir_all(&staging_dir).await;

    let ok_path = target_dir.join(".ok");
    tokio::fs::write(&ok_path, &asset.sha256).await.map_err(|e| e.to_string())?;

    let _ = on_event.send(InstallEvent::Installed);
    Ok(())
}

fn uuid_v4_simple() -> String {
    format!("{:032x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())
}
