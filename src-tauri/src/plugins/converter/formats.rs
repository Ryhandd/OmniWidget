use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Png, Jpg, Webp, Bmp, Gif, Ico,
    Docx, Md, Html, Txt,
    Mp3, Wav, Flac, Ogg, M4a, Opus,
    Pdf,
}

/// Engine konversi. Semua engine adalah kode bawaan aplikasi: tidak ada biner eksternal
/// yang perlu dipasang pengguna.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine { Native, Audio, Doc, Pdfium, LibreOffice }

#[derive(Debug, Clone, Copy)]
pub struct Route { pub from: Format, pub to: Format, pub engine: Engine }

impl Format {
    pub fn ext(self) -> &'static str {
        use Format::*;
        match self {
            Png => "png", Jpg => "jpg", Webp => "webp", Bmp => "bmp", Gif => "gif", Ico => "ico",
            Docx => "docx", Md => "md", Html => "html", Txt => "txt",
            Mp3 => "mp3", Wav => "wav", Flac => "flac", Ogg => "ogg", M4a => "m4a", Opus => "opus",
            Pdf => "pdf",
        }
    }
}

const IMAGES: [Format; 6] = [Format::Png, Format::Jpg, Format::Webp, Format::Bmp, Format::Gif, Format::Ico];
/// Sumber audio yang bisa diddekode `symphonia` (Opus belum didukung).
const AUDIO_SOURCES: [Format; 5] = [Format::Mp3, Format::Wav, Format::Flac, Format::Ogg, Format::M4a];
/// Target audio yang bisa ditulis encoder murni Rust.
const AUDIO_TARGETS: [Format; 4] = [Format::Mp3, Format::Wav, Format::Flac, Format::Ogg];

pub fn routes() -> Vec<Route> {
    use Format::*;
    let mut v = Vec::new();
    for f in IMAGES { for t in IMAGES { if f != t { v.push(Route { from: f, to: t, engine: Engine::Native }); } } }
    for f in AUDIO_SOURCES {
        for t in AUDIO_TARGETS {
            if f != t { v.push(Route { from: f, to: t, engine: Engine::Audio }); }
        }
    }
    for (f, t) in [
        (Docx, Md), (Docx, Html), (Docx, Txt),
        (Md, Docx), (Md, Html), (Md, Pdf),
        (Html, Md), (Html, Docx), (Html, Txt), (Html, Pdf),
    ] {
        v.push(Route { from: f, to: t, engine: Engine::Doc });
    }
    // DOCX -> PDF: LibreOffice (bila terpasang) memberi tata letak paling akurat; engine Doc bawaan
    // selalu tersedia sebagai cadangan sehingga rute ini tidak pernah mati.
    v.push(Route { from: Docx, to: Pdf, engine: Engine::LibreOffice });
    v.push(Route { from: Docx, to: Pdf, engine: Engine::Doc });
    v.push(Route { from: Pdf, to: Png, engine: Engine::Pdfium });
    v.push(Route { from: Pdf, to: Jpg, engine: Engine::Pdfium });
    v.push(Route { from: Pdf, to: Txt, engine: Engine::Pdfium });
    v.push(Route { from: Pdf, to: Docx, engine: Engine::Pdfium });
    v
}

pub fn targets_for(from: Format) -> Vec<Route> { routes().into_iter().filter(|r| r.from == from).collect() }
pub fn routes_for(from: Format, to: Format) -> Vec<Route> { routes().into_iter().filter(|r| r.from == from && r.to == to).collect() }
pub fn find(from: Format, to: Format) -> Option<Route> { routes_for(from, to).into_iter().next() }

pub fn detect(path: &Path) -> Option<Format> {
    use Format::*;
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    if let Ok(Some(kind)) = infer::get_from_path(path) {
        let detected = match kind.mime_type() {
            "image/png" => Some(Png),
            "image/jpeg" => Some(Jpg),
            "image/webp" => Some(Webp),
            "image/bmp" => Some(Bmp),
            "image/gif" => Some(Gif),
            "image/vnd.microsoft.icon" => Some(Ico),
            "application/pdf" => Some(Pdf),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Some(Docx),
            "audio/mpeg" => Some(Mp3),
            "audio/x-wav" => Some(Wav),
            "audio/x-flac" => Some(Flac),
            "audio/ogg" => Some(if ext.as_deref() == Some("opus") { Opus } else { Ogg }),
            "audio/m4a" | "audio/x-m4a" | "video/mp4" => Some(M4a),
            // DOCX (ZIP) dan beberapa kontainer lain tidak dikenali `infer`; jatuh ke ekstensi.
            _ => None,
        };
        if detected.is_some() {
            return detected;
        }
    }
    match ext.as_deref() {
        Some("md") | Some("markdown") => Some(Md),
        Some("html") | Some("htm") => Some(Html),
        Some("txt") => Some(Txt),
        Some("pdf") => Some(Pdf),
        Some("docx") => Some(Docx),
        Some("mp3") => Some(Mp3),
        Some("wav") => Some(Wav),
        Some("flac") => Some(Flac),
        Some("ogg") | Some("oga") => Some(Ogg),
        Some("m4a") => Some(M4a),
        Some("opus") => Some(Opus),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semua_rute_memakai_engine_bawaan() {
        for route in routes() {
            assert_ne!(route.from, route.to);
        }
    }

    #[test]
    fn m4a_hanya_sumber_bukan_target() {
        assert!(targets_for(Format::M4a).iter().all(|r| r.engine == Engine::Audio));
        assert!(find(Format::Mp3, Format::M4a).is_none());
    }

    #[test]
    fn dokumen_memiliki_rute_dua_arah() {
        assert!(find(Format::Html, Format::Docx).is_some());
        assert!(find(Format::Docx, Format::Html).is_some());
    }

    #[test]
    fn dokumen_bisa_menjadi_pdf() {
        let e = find(Format::Docx, Format::Pdf).map(|r| r.engine);
        assert!(e == Some(Engine::LibreOffice) || e == Some(Engine::Doc));
        assert!(find(Format::Md, Format::Pdf).is_some());
        assert!(find(Format::Html, Format::Pdf).is_some());
    }

    #[test]
    fn pdf_bisa_menjadi_docx() {
        assert_eq!(find(Format::Pdf, Format::Docx).map(|r| r.engine), Some(Engine::Pdfium));
    }
}
