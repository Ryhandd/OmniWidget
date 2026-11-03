use std::path::Path;

pub async fn extract_archive(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let path_str = archive_path.to_string_lossy().to_lowercase();
    tokio::fs::create_dir_all(target_dir).await.map_err(|e| e.to_string())?;

    if path_str.ends_with(".zip") {
        extract_zip(archive_path, target_dir)?;
    } else if path_str.ends_with(".tar.gz") || path_str.ends_with(".tgz") {
        extract_tar_gz(archive_path, target_dir)?;
    } else if path_str.ends_with(".tar.xz") {
        extract_tar_xz(archive_path, target_dir)?;
    } else {
        return Err("Unsupported archive format".into());
    }
    Ok(())
}

fn extract_zip(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = match entry.enclosed_name() {
            Some(n) => n.to_owned(),
            None => continue,
        };
        let file_name = match name.file_name() {
            Some(f) => f.to_owned(),
            None => continue,
        };
        let out_path = target_dir.join(file_name);
        let mut outfile = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut outfile).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn extract_tar_gz(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive_path).map_err(|e| e.to_string())?;
    let decompressed = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decompressed);

    for entry_result in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry_result.map_err(|e| e.to_string())?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().map_err(|e| e.to_string())?;
        let file_name = match path.file_name() {
            Some(f) => f.to_owned(),
            None => continue,
        };
        let out_path = target_dir.join(file_name);
        entry.unpack(&out_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn extract_tar_xz(archive_path: &Path, target_dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive_path).map_err(|e| e.to_string())?;
    let decompressed = xz2::read::XzDecoder::new(file);
    let mut archive = tar::Archive::new(decompressed);

    for entry_result in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry_result.map_err(|e| e.to_string())?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().map_err(|e| e.to_string())?;
        let file_name = match path.file_name() {
            Some(f) => f.to_owned(),
            None => continue,
        };
        let out_path = target_dir.join(file_name);
        entry.unpack(&out_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
