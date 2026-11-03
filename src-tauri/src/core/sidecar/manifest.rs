use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlatformAsset {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SidecarEntry {
    pub supported: bool,
    pub platforms: HashMap<String, PlatformAsset>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Manifest {
    pub version: u32,
    pub sidecars: HashMap<String, SidecarEntry>,
}

pub fn parse_manifest(json_str: &str) -> Result<Manifest, String> {
    let manifest: Manifest = serde_json::from_str(json_str).map_err(|e| e.to_string())?;
    if manifest.version != 1 {
        return Err("Unsupported manifest version".into());
    }
    for (name, entry) in &manifest.sidecars {
        if entry.supported && entry.platforms.is_empty() {
            return Err(format!("Sidecar '{}' marked supported but has no platforms", name));
        }
    }
    Ok(manifest)
}
