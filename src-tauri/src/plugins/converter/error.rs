use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum ConvError {
    #[error("Format file tidak dikenali atau belum didukung")]
    UnknownFormat,
    #[error("Tidak ada rute konversi untuk pasangan format ini")]
    NoRoute,
    #[error("Berkas input tidak ditemukan: {0}")]
    InputMissing(String),
    #[error("Folder output tidak valid: {0}")]
    BadOutDir(String),
    #[error("Gambar terlalu besar ({0} MP, maks {1} MP)")]
    PixelBomb(u64, u64),
    #[error("Sidecar '{0}' belum terpasang. Pasang lewat Pengaturan")]
    SidecarMissing(&'static str),
    #[error("Proses {0} gagal: {1}")]
    SidecarFailed(&'static str, String),
    #[error("Proses melewati batas waktu ({0} dtk)")]
    Timeout(u64),
    #[error("Dibatalkan")]
    Cancelled,
    #[error("Image: {0}")]
    Image(#[from] image::ImageError),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("PDF: {0}")]
    Pdf(String),
    #[error("PDF memiliki {0} halaman (maks {1} untuk ekspor semua halaman). Pilih \"Halaman pertama\"")]
    TooManyPages(usize, usize),
    #[error("Halaman {0} berukuran {1:.0} x {2:.0} inci, terlalu besar untuk dirender dengan aman. Pilih \"Halaman pertama\" atau perkecil PDF-nya")]
    PageTooLarge(usize, f32, f32),
    #[error("Halaman {0} memiliki ukuran yang tidak valid")]
    BadPageSize(usize),
    #[error("Proses konversi berhenti tiba-tiba")]
    WorkerCrashed,
    #[error("Berkas kosong atau tidak terbaca")]
    EmptyUpload,
    #[error("Berkas terlalu besar ({0} MB, maks {1} MB untuk tempel/seret). Gunakan \"Pilih file…\"")]
    UploadTooLarge(u64, u64),
}

impl ConvError {
    pub fn is_cancelled(&self) -> bool {
        matches!(self, ConvError::Cancelled)
    }
}

impl Serialize for ConvError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
