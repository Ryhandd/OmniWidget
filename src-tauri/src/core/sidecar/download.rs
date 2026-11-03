use futures_util::StreamExt;
use sha2::{Sha256, Digest};
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tokio::sync::oneshot;

pub async fn download_file(
    url: &str,
    dest: &Path,
    expected_sha256: &str,
    mut on_progress: impl FnMut(u64, u64, f64),
    mut cancel_rx: oneshot::Receiver<()>,
) -> Result<(), String> {
    if expected_sha256.len() != 64 || expected_sha256.chars().all(|c| c == '0') {
        return Err("Invalid or empty SHA256 checksum in manifest".into());
    }

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| e.to_string())?;

    let res = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("HTTP error status: {}", res.status()));
    }

    let total_size = res.content_length().unwrap_or(0);
    if total_size > 512 * 1024 * 1024 {
        return Err("File size exceeds 512 MiB limit".into());
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    }

    let mut file = tokio::fs::File::create(dest).await.map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut hasher = Sha256::new();
    let mut stream = res.bytes_stream();

    let start_time = std::time::Instant::now();
    let mut last_progress_time = std::time::Instant::now();

    loop {
        tokio::select! {
            _ = &mut cancel_rx => {
                let _ = tokio::fs::remove_file(dest).await;
                return Err("Cancelled".into());
            }
            chunk_result = stream.next() => {
                match chunk_result {
                    Some(Ok(chunk)) => {
                        if downloaded + chunk.len() as u64 > 512 * 1024 * 1024 {
                            let _ = tokio::fs::remove_file(dest).await;
                            return Err("File size exceeded limit during download".into());
                        }
                        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
                        hasher.update(&chunk);
                        downloaded += chunk.len() as u64;

                        let elapsed = start_time.elapsed().as_secs_f64();
                        let speed = if elapsed > 0.0 { downloaded as f64 / elapsed } else { 0.0 };

                        if last_progress_time.elapsed().as_millis() > 100 {
                            on_progress(downloaded, total_size, speed);
                            last_progress_time = std::time::Instant::now();
                        }
                    }
                    Some(Err(e)) => {
                        let _ = tokio::fs::remove_file(dest).await;
                        return Err(e.to_string());
                    }
                    None => break,
                }
            }
        }
    }

    file.flush().await.map_err(|e| e.to_string())?;
    drop(file);

    let result_hash = hex::encode(hasher.finalize());
    if result_hash.to_lowercase() != expected_sha256.to_lowercase() {
        let _ = tokio::fs::remove_file(dest).await;
        return Err(format!("Checksum mismatch! Expected {}, got {}", expected_sha256, result_hash));
    }

    Ok(())
}
