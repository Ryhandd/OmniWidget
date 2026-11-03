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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine { Native, Ffmpeg, Pandoc, Pdfium }

impl Engine {
    /// Nama komponen tambahan yang dibutuhkan engine ini (None = bawaan, tanpa unduhan).
    pub fn sidecar(self) -> Option<&'static str> {
        match self {
            Engine::Native => None,
            Engine::Ffmpeg => Some("ffmpeg"),
            Engine::Pandoc => Some("pandoc"),
            Engine::Pdfium => Some("pdfium"),
        }
    }
}

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
const AUDIO: [Format; 6] = [Format::Mp3, Format::Wav, Format::Flac, Format::Ogg, Format::M4a, Format::Opus];

pub fn routes() -> Vec<Route> {
    use Format::*;
    let mut v = Vec::new();
    for f in IMAGES { for t in IMAGES { if f != t { v.push(Route { from: f, to: t, engine: Engine::Native }); } } }
    for f in AUDIO  { for t in AUDIO  { if f != t { v.push(Route { from: f, to: t, engine: Engine::Ffmpeg }); } } }
    for (f, t) in [(Docx, Md), (Docx, Html), (Docx, Txt), (Md, Docx), (Md, Html), (Html, Md), (Html, Docx), (Html, Txt)] {
        v.push(Route { from: f, to: t, engine: Engine::Pandoc });
    }
    v.push(Route { from: Pdf, to: Png, engine: Engine::Pdfium });
    v.push(Route { from: Pdf, to: Jpg, engine: Engine::Pdfium });
    v.push(Route { from: Pdf, to: Txt, engine: Engine::Pdfium });
    v
}

pub fn targets_for(from: Format) -> Vec<Route> { routes().into_iter().filter(|r| r.from == from).collect() }
pub fn find(from: Format, to: Format) -> Option<Route> { routes().into_iter().find(|r| r.from == from && r.to == to) }

pub fn detect(path: &Path) -> Option<Format> {
    use Format::*;
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    if let Ok(Some(kind)) = infer::get_from_path(path) {
        let f = match kind.mime_type() {
            "image/png" => Png,
            "image/jpeg" => Jpg,
            "image/webp" => Webp,
            "image/bmp" => Bmp,
            "image/gif" => Gif,
            "image/vnd.microsoft.icon" => Ico,
            "application/pdf" => Pdf,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Docx,
            "audio/mpeg" => Mp3,
            "audio/x-wav" => Wav,
            "audio/x-flac" => Flac,
            "audio/ogg" => if ext.as_deref() == Some("opus") { Opus } else { Ogg },
            "audio/m4a" | "audio/x-m4a" => M4a,
            _ => return None,
        };
        return Some(f);
    }
    match ext.as_deref() {
        Some("md") | Some("markdown") => Some(Md),
        Some("html") | Some("htm") => Some(Html),
        Some("txt") => Some(Txt),
        Some("pdf") => Some(Pdf),
        _ => None,
    }
}
