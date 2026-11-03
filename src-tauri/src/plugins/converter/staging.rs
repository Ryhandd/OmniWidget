//! Staging berkas dari drop/paste DOM. `File` di WebView tidak punya path, sedangkan konverter bekerja
//! dengan path di disk, jadi byte-nya dikirim sebagai badan biner mentah dan disimpan sementara di
//! `<cache>/omniwidget-staging/<id>/<nama>`. Isi berkas divalidasi ulang di sini (magic bytes), bukan
//! mempercayai ekstensi/MIME dari UI. Folder staging dibersihkan otomatis (lebih tua dari 24 jam).

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tauri::{ipc::InvokeBody, AppHandle, Manager};

use super::{error::ConvError, formats, output};

/// Nama folder staging. HARUS sama dengan STAGING_DIR di `src/modules/converter/intake.ts`.
pub const STAGING_DIR: &str = "omniwidget-staging";
/// HARUS sama dengan MAX_UPLOAD_BYTES di intake.ts.
pub const MAX_UPLOAD_BYTES: usize = 100 * 1024 * 1024;
const MAX_EXT_CHARS: usize = 10;
const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

static COUNTER: AtomicU64 = AtomicU64::new(0);

// ---------- Logika murni (teruji dengan folder sementara) ----------

/// Nama aman dari nama mentah UI: hanya segmen terakhir, tanpa karakter terlarang, ekstensi alfanumerik pendek.
pub fn sanitize_upload_name(raw: &str) -> String {
    let last = raw.rsplit(|c| c == '/' || c == '\\').next().unwrap_or("");
    let (stem, ext) = match last.rfind('.') {
        Some(i) if i > 0 => (&last[..i], &last[i + 1..]),
        _ => (last, ""),
    };
    let ext: String = ext
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(MAX_EXT_CHARS)
        .collect::<String>()
        .to_ascii_lowercase();
    let stem = output::sanitize_stem(stem);
    if ext.is_empty() { stem } else { format!("{stem}.{ext}") }
}

/// Decode `%XX` (hasil `encodeURIComponent` di JS). Urutan tidak valid dibiarkan apa adanya.
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let (h, l) = (b[i + 1], b[i + 2]);
            if h.is_ascii_hexdigit() && l.is_ascii_hexdigit() {
                let hex = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
                out.push(hex(h) * 16 + hex(l));
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn check_size(len: usize) -> Result<(), ConvError> {
    if len == 0 {
        return Err(ConvError::EmptyUpload);
    }
    if len > MAX_UPLOAD_BYTES {
        let mb = |n: usize| (n as u64 + 1024 * 1024 - 1) / (1024 * 1024);
        return Err(ConvError::UploadTooLarge(mb(len), mb(MAX_UPLOAD_BYTES)));
    }
    Ok(())
}

/// Buat folder unik `root/<id>/` dan kembalikan path berkas tujuan di dalamnya.
fn prepare_target(root: &Path, raw_name: &str) -> Result<PathBuf, ConvError> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let id = format!("{:x}-{:x}", nanos, COUNTER.fetch_add(1, Ordering::Relaxed));
    let dir = root.join(id);
    fs::create_dir_all(&dir)?;
    Ok(dir.join(sanitize_upload_name(raw_name)))
}

/// Buang folder staging milik `file` (dipanggil saat gagal agar tidak menyisakan sampah).
fn discard(file: &Path) {
    if let Some(dir) = file.parent() {
        let _ = fs::remove_dir_all(dir);
    }
}

/// Isi berkas harus format yang dikenal konverter; bila tidak, hapus dan tolak.
fn validate_staged(file: &Path) -> Result<(), ConvError> {
    if formats::detect(file).is_none() {
        discard(file);
        return Err(ConvError::UnknownFormat);
    }
    Ok(())
}

/// Versi sinkron lengkap (dipakai tes; command memakai tulis async untuk berkas besar).
pub fn stage_bytes(root: &Path, raw_name: &str, bytes: &[u8]) -> Result<PathBuf, ConvError> {
    check_size(bytes.len())?;
    let file = prepare_target(root, raw_name)?;
    if let Err(e) = fs::write(&file, bytes) {
        discard(&file);
        return Err(e.into());
    }
    validate_staged(&file)?;
    Ok(file)
}

/// Hapus folder staging yang lebih tua dari `max_age`. Gagal menghapus satu folder tidak menghentikan yang lain.
pub fn purge_older_than(root: &Path, max_age: Duration) -> usize {
    let Ok(entries) = fs::read_dir(root) else { return 0 };
    let now = SystemTime::now();
    let mut removed = 0;
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .map(|t| now.duration_since(t).unwrap_or(Duration::ZERO)) // jam mundur: anggap baru
            .map_or(false, |age| age >= max_age);
        if old && fs::remove_dir_all(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

// ---------- Integrasi Tauri ----------

fn io_other(e: impl ToString) -> ConvError {
    ConvError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

pub fn staging_root(app: &AppHandle) -> Result<PathBuf, ConvError> {
    Ok(app.path().app_cache_dir().map_err(io_other)?.join(STAGING_DIR))
}

/// Bersihkan staging lama di latar belakang (dipanggil dari `setup`).
pub fn spawn_purge(app: &AppHandle) {
    if let Ok(root) = staging_root(app) {
        tauri::async_runtime::spawn_blocking(move || {
            purge_older_than(&root, MAX_AGE);
        });
    }
}

/// Untuk input hasil staging, "folder sumber" tidak bermakna (folder sementara tersembunyi), jadi hasil
/// diarahkan ke folder Unduhan. `Ok(None)` = bukan input staging: pakai aturan biasa.
pub fn default_out_dir(app: &AppHandle, input: &Path) -> Result<Option<PathBuf>, ConvError> {
    let Ok(root) = staging_root(app) else { return Ok(None) };
    if !input.starts_with(&root) {
        return Ok(None);
    }
    let resolver = app.path();
    resolver
        .download_dir()
        .or_else(|_| resolver.document_dir())
        .map(Some)
        .map_err(|_| ConvError::BadOutDir("folder Unduhan tidak ditemukan; pilih folder output".into()))
}

/// Terima byte file sebagai badan biner mentah; nama asli di header `x-file-name` (di-encode URI).
/// Mengembalikan path hasil staging untuk diteruskan ke `converter_probe`.
#[tauri::command]
pub async fn converter_stage_upload(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<String, ConvError> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(ConvError::EmptyUpload);
    };
    check_size(bytes.len())?;
    let raw_name = request
        .headers()
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .map(percent_decode)
        .unwrap_or_default();

    let root = staging_root(&app)?;
    let file = prepare_target(&root, &raw_name)?;
    if let Err(e) = tokio::fs::write(&file, bytes).await {
        discard(&file);
        return Err(e.into());
    }
    validate_staged(&file)?;
    Ok(file.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: [u8; 12] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];

    #[test]
    fn nama_upload_disanitasi() {
        assert_eq!(sanitize_upload_name("foto.PNG"), "foto.png");
        assert_eq!(sanitize_upload_name("../../evil.png"), "evil.png");
        assert_eq!(sanitize_upload_name("C:\\Users\\x\\laporan akhir.pdf"), "laporan akhir.pdf");
        assert_eq!(sanitize_upload_name("a:b*c.txt"), "a_b_c.txt");
        assert_eq!(sanitize_upload_name("tanpa-ekstensi"), "tanpa-ekstensi");
        assert_eq!(sanitize_upload_name(""), "output");
        assert_eq!(sanitize_upload_name("x.p!n@g"), "x.png");
        assert_eq!(sanitize_upload_name(".hidden"), "hidden");
        assert!(!sanitize_upload_name("a/b\\c.png").contains(|c: char| c == '/' || c == '\\'));
    }

    #[test]
    fn percent_decode_utf8_dan_sisa_tidak_valid() {
        assert_eq!(percent_decode("laporan%20akhir.pdf"), "laporan akhir.pdf");
        assert_eq!(percent_decode("caf%C3%A9.png"), "café.png");
        assert_eq!(percent_decode("100%.png"), "100%.png");
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("%"), "%");
        assert_eq!(percent_decode("%4"), "%4");
    }

    #[test]
    fn stage_menyimpan_berkas_valid() {
        let root = tempfile::tempdir().unwrap();
        let p = stage_bytes(root.path(), "foto.png", &PNG).unwrap();
        assert!(p.is_file() && p.starts_with(root.path()));
        assert_eq!(p.file_name().unwrap(), "foto.png");
        assert_eq!(fs::read(&p).unwrap(), PNG);
        // Dua upload bernama sama tidak saling menimpa.
        let q = stage_bytes(root.path(), "foto.png", &PNG).unwrap();
        assert_ne!(p, q);
    }

    #[test]
    fn stage_menolak_kosong_terlalu_besar_dan_format_tak_dikenal_tanpa_sisa() {
        let root = tempfile::tempdir().unwrap();
        assert!(matches!(stage_bytes(root.path(), "a.png", &[]), Err(ConvError::EmptyUpload)));
        assert!(matches!(check_size(MAX_UPLOAD_BYTES + 1), Err(ConvError::UploadTooLarge(_, _))));
        assert!(check_size(MAX_UPLOAD_BYTES).is_ok());
        assert!(matches!(stage_bytes(root.path(), "rahasia.exe", b"MZ nope"), Err(ConvError::UnknownFormat)));
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0, "berkas ditolak tidak boleh tersisa");
    }

    #[test]
    fn ekstensi_palsu_tidak_lolos_tapi_isi_valid_tanpa_ekstensi_lolos() {
        let root = tempfile::tempdir().unwrap();
        assert!(stage_bytes(root.path(), "gambar", &PNG).is_ok(), "dideteksi dari isi");
        assert!(matches!(stage_bytes(root.path(), "palsu.png", b"bukan gambar"), Err(ConvError::UnknownFormat)));
    }

    #[test]
    fn purge_hanya_menghapus_yang_cukup_tua() {
        let root = tempfile::tempdir().unwrap();
        stage_bytes(root.path(), "a.png", &PNG).unwrap();
        stage_bytes(root.path(), "b.png", &PNG).unwrap();
        assert_eq!(purge_older_than(root.path(), Duration::from_secs(3600)), 0, "baru dibuat: tetap");
        assert_eq!(purge_older_than(root.path(), Duration::ZERO), 2);
        assert_eq!(purge_older_than(&root.path().join("tidak-ada"), Duration::ZERO), 0);
    }
}
