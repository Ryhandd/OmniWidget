//! Penamaan & penulisan output yang aman. Hanya memakai `std` agar mudah diuji terpisah.
//!
//! Aturan (TDD 4.1): tulis ke berkas `.part` lalu rename atomik; nama yang sudah ada tidak pernah
//! ditimpa diam-diam, tetapi diberi sufiks `-1`, `-2`, ...

use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
};

const MAX_STEM_CHARS: usize = 120;
const MAX_SUFFIX_TRIES: u32 = 10_000;

/// Jenis hasil konversi: satu berkas, atau satu folder (PDF multi-halaman -> banyak gambar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutKind {
    File,
    Dir,
}

/// Bersihkan nama dasar dari karakter yang tidak valid di Windows/Unix dan dari pemisah path,
/// sehingga hasilnya tidak mungkin keluar dari folder output.
pub fn sanitize_stem(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    let limited: String = trimmed.chars().take(MAX_STEM_CHARS).collect();
    let limited = limited.trim_end().to_string();
    if limited.is_empty() {
        "output".to_string()
    } else {
        limited
    }
}

/// Nama dasar dari path input (tanpa ekstensi), sudah disanitasi.
pub fn stem_of(input: &Path) -> String {
    sanitize_stem(input.file_stem().and_then(|s| s.to_str()).unwrap_or("output"))
}

/// Path sementara tersembunyi di folder output. `job_id` mencegah tabrakan antar job bernama sama.
pub fn part_path(dir: &Path, stem: &str, job_id: &str) -> PathBuf {
    dir.join(format!(".{stem}.{job_id}.part"))
}

/// Pesan nama berkas final dan *pesan tempatnya* secara atomik (`create_new`): dua job yang
/// selesai bersamaan tidak akan memilih nama yang sama. Berkas kosong hasil reservasi kemudian
/// ditimpa oleh `rename` (rename menimpa tujuan di Unix maupun Windows).
pub fn reserve_unique_file(dir: &Path, stem: &str, ext: &str) -> io::Result<PathBuf> {
    for i in 0..MAX_SUFFIX_TRIES {
        let name = if i == 0 { format!("{stem}.{ext}") } else { format!("{stem}-{i}.{ext}") };
        let candidate = dir.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&candidate) {
            Ok(_) => return Ok(candidate),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "terlalu banyak berkas bernama sama"))
}

/// Path folder final yang belum dipakai (`nama`, `nama-1`, ...).
pub fn unique_dir_path(dir: &Path, stem: &str) -> io::Result<PathBuf> {
    for i in 0..MAX_SUFFIX_TRIES {
        let name = if i == 0 { stem.to_string() } else { format!("{stem}-{i}") };
        let candidate = dir.join(name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "terlalu banyak folder bernama sama"))
}

/// Pindahkan hasil sementara ke nama final. Mengembalikan path final.
pub fn finalize(part: &Path, dir: &Path, stem: &str, ext: &str, kind: OutKind) -> io::Result<PathBuf> {
    match kind {
        OutKind::File => {
            let dest = reserve_unique_file(dir, stem, ext)?;
            fs::rename(part, &dest).map_err(|e| {
                let _ = fs::remove_file(&dest); // buang berkas kosong hasil reservasi
                e
            })?;
            Ok(dest)
        }
        OutKind::Dir => {
            let dest = unique_dir_path(dir, stem)?;
            fs::rename(part, &dest)?;
            Ok(dest)
        }
    }
}

/// Penjaga RAII: menghapus berkas/folder sementara kecuali sudah `disarm()`. Menjamin tidak ada
/// sisa `.part` pada error, pembatalan, maupun panic.
pub struct PartGuard {
    path: Option<PathBuf>,
}

impl PartGuard {
    pub fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }
    pub fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for PartGuard {
    fn drop(&mut self) {
        if let Some(p) = self.path.take() {
            if p.is_dir() {
                let _ = fs::remove_dir_all(&p);
            } else {
                let _ = fs::remove_file(&p);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ow-out-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn sanitize_menghapus_pemisah_dan_karakter_terlarang() {
        assert_eq!(sanitize_stem("a/b\\c"), "a_b_c");
        assert_eq!(sanitize_stem("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(sanitize_stem("lap:oran*?"), "lap_oran__");
        assert_eq!(sanitize_stem("  ..  "), "output");
        assert_eq!(sanitize_stem(""), "output");
        assert_eq!(sanitize_stem("ok.v2"), "ok.v2");
        assert_eq!(sanitize_stem(&"x".repeat(500)).chars().count(), MAX_STEM_CHARS);
        assert!(!sanitize_stem("a\nb\tc").contains(['\n', '\t']));
    }

    #[test]
    fn stem_of_memakai_nama_tanpa_ekstensi() {
        assert_eq!(stem_of(Path::new("/x/y/foto liburan.final.png")), "foto liburan.final");
        assert_eq!(stem_of(Path::new("")), "output");
    }

    #[test]
    fn reservasi_tidak_pernah_menimpa() {
        let d = tmp("reserve");
        let a = reserve_unique_file(&d, "laporan", "pdf").unwrap();
        let b = reserve_unique_file(&d, "laporan", "pdf").unwrap();
        let c = reserve_unique_file(&d, "laporan", "pdf").unwrap();
        assert_eq!(a.file_name().unwrap(), "laporan.pdf");
        assert_eq!(b.file_name().unwrap(), "laporan-1.pdf");
        assert_eq!(c.file_name().unwrap(), "laporan-2.pdf");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn finalize_file_menimpa_placeholder_bukan_berkas_pengguna() {
        let d = tmp("final-file");
        fs::write(d.join("a.txt"), "milik pengguna").unwrap();
        let part = part_path(&d, "a", "job-1");
        fs::write(&part, "hasil").unwrap();
        let out = finalize(&part, &d, "a", "txt", OutKind::File).unwrap();
        assert_eq!(out.file_name().unwrap(), "a-1.txt");
        assert_eq!(fs::read_to_string(&out).unwrap(), "hasil");
        assert_eq!(fs::read_to_string(d.join("a.txt")).unwrap(), "milik pengguna");
        assert!(!part.exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn finalize_dir_memberi_sufiks() {
        let d = tmp("final-dir");
        fs::create_dir(d.join("buku")).unwrap();
        let part = part_path(&d, "buku", "job-2");
        fs::create_dir(&part).unwrap();
        fs::write(part.join("page-001.png"), "x").unwrap();
        let out = finalize(&part, &d, "buku", "", OutKind::Dir).unwrap();
        assert_eq!(out.file_name().unwrap(), "buku-1");
        assert!(out.join("page-001.png").is_file());
        assert!(!part.exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn guard_membersihkan_kecuali_disarm() {
        let d = tmp("guard");
        let f = d.join("x.part");
        fs::write(&f, "1").unwrap();
        drop(PartGuard::new(f.clone()));
        assert!(!f.exists());

        let dir = d.join("y.part");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("a"), "1").unwrap();
        drop(PartGuard::new(dir.clone()));
        assert!(!dir.exists());

        let keep = d.join("k.part");
        fs::write(&keep, "1").unwrap();
        let mut g = PartGuard::new(keep.clone());
        g.disarm();
        drop(g);
        assert!(keep.exists());
        let _ = fs::remove_dir_all(&d);
    }
}
