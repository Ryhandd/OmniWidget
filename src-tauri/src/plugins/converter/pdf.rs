use super::{
    error::ConvError,
    formats::Format,
    output::OutKind,
    render_budget::{plan_render, BudgetError, RenderPlan},
    CancelToken,
};
use pdfium_render::prelude::*;
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
    sync::atomic::Ordering,
};

/// Batas halaman untuk ekspor "semua halaman" agar satu job tidak menulis ribuan berkas.
pub const MAX_EXPORT_PAGES: usize = 300;

/// Pilihan halaman untuk rute PDF -> gambar (diabaikan untuk PDF -> teks, yang selalu semua halaman).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pages {
    First,
    #[default]
    All,
}

/// Menulis hasil ke `output`:
/// - teks, atau gambar 1 halaman -> satu berkas (`OutKind::File`)
/// - gambar >1 halaman -> folder berisi `page-001.png`, ... (`OutKind::Dir`)
pub fn convert(
    input: &Path,
    output: &Path,
    to: Format,
    pages: Pages,
    lib_dir: &Path,
    cancel: &CancelToken,
    progress: &dyn Fn(f32),
) -> Result<OutKind, ConvError> {
    let bindings = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(lib_dir))
        .map_err(|e| ConvError::Pdf(format!("Gagal memuat pustaka Pdfium: {}", e)))?;
    let pdfium = Pdfium::new(bindings);

    let document = pdfium.load_pdf_from_file(input.to_str().unwrap_or_default(), None)
        .map_err(|e| ConvError::Pdf(format!("Gagal membuka PDF: {}", e)))?;

    progress(0.05);
    if cancel.load(Ordering::Relaxed) { return Err(ConvError::Cancelled); }

    let total = document.pages().len() as usize;
    if total == 0 {
        return Err(ConvError::Pdf("Dokumen PDF kosong".into()));
    }

    match to {
        Format::Txt => {
            // Ditulis per halaman ke disk: RAM tetap sebesar satu halaman, berapa pun jumlah halamannya.
            let mut out = BufWriter::new(File::create(output)?);
            for (i, page) in document.pages().iter().enumerate() {
                if cancel.load(Ordering::Relaxed) { return Err(ConvError::Cancelled); }
                writeln!(out, "--- Page {} ---", i + 1)?;
                if let Ok(t) = page.text() {
                    out.write_all(t.all().as_bytes())?;
                }
                out.write_all(b"\n\n")?;
                progress(0.05 + 0.9 * ((i + 1) as f32 / total as f32));
            }
            out.flush()?;
            progress(1.0);
            Ok(OutKind::File)
        }
        Format::Docx => {
            // Teks tiap halaman diekstrak lewat Pdfium lalu ditulis sebagai DOCX terstruktur.
            // Gambar dan tata letak visual PDF tidak ikut; hanya isi teks yang dipertahankan.
            let mut pages = Vec::with_capacity(total);
            for (i, page) in document.pages().iter().enumerate() {
                if cancel.load(Ordering::Relaxed) { return Err(ConvError::Cancelled); }
                let text = page.text().map(|t| t.all()).unwrap_or_default();
                pages.push(text);
                progress(0.05 + 0.85 * ((i + 1) as f32 / total as f32));
            }
            super::doc::write_text_docx(&pages, output)?;
            progress(1.0);
            Ok(OutKind::File)
        }
        Format::Png | Format::Jpg => {
            let count = match pages {
                Pages::First => 1,
                Pages::All if total > MAX_EXPORT_PAGES => {
                    return Err(ConvError::TooManyPages(total, MAX_EXPORT_PAGES));
                }
                Pages::All => total,
            };
            let single = count == 1;

            // Pra-pindai ukuran SEMUA halaman yang akan diekspor sebelum menulis apa pun: halaman raksasa
            // menggagalkan job di awal (dengan nomor halamannya), bukan setelah puluhan berkas tertulis.
            let mut plans: Vec<RenderPlan> = Vec::with_capacity(count);
            for (i, page) in document.pages().iter().enumerate().take(count) {
                if cancel.load(Ordering::Relaxed) { return Err(ConvError::Cancelled); }
                plans.push(plan_page(i + 1, &page)?);
            }
            progress(0.08);

            if !single {
                fs::create_dir_all(output)?;
            }

            for (i, page) in document.pages().iter().enumerate().take(count) {
                if cancel.load(Ordering::Relaxed) { return Err(ConvError::Cancelled); }
                let target = if single {
                    output.to_path_buf()
                } else {
                    output.join(format!("page-{:03}.{}", i + 1, to.ext()))
                };
                render_page(&page, &plans[i], &target, to)?;
                // Satu halaman dirender, ditulis, lalu dibebaskan sebelum halaman berikutnya.
                progress(0.08 + 0.92 * ((i + 1) as f32 / count as f32));
            }
            progress(1.0);
            Ok(if single { OutKind::File } else { OutKind::Dir })
        }
        _ => Err(ConvError::NoRoute),
    }
}

/// Ukuran halaman (point) -> rencana render yang muat anggaran memori (lihat `render_budget`).
fn plan_page(number: usize, page: &PdfPage) -> Result<RenderPlan, ConvError> {
    plan_render(page.width().value, page.height().value).map_err(|e| match e {
        BudgetError::InvalidSize => ConvError::BadPageSize(number),
        BudgetError::TooLarge { width_in, height_in } => ConvError::PageTooLarge(number, width_in, height_in),
    })
}

fn render_page(page: &PdfPage, plan: &RenderPlan, target: &Path, to: Format) -> Result<(), ConvError> {
    // DPI sudah disesuaikan anggaran piksel; halaman normal tetap 150 DPI.
    // 72 pt = 1 inci, jadi faktor skala = DPI / 72.
    let config = PdfRenderConfig::new()
        .scale_page_by_factor(plan.dpi as f32 / 72.0)
        .render_annotations(true)
        .render_form_data(true);
    let bitmap = page.render_with_config(&config)
        .map_err(|e| ConvError::Pdf(format!("Render PDF gagal: {}", e)))?;
    let img = bitmap.as_image();
    drop(bitmap); // lepas buffer native secepatnya; puncak RAM = satu salinan, bukan dua
    if to == Format::Png {
        img.save_with_format(target, image::ImageFormat::Png)?;
    } else {
        // JPEG tidak punya alpha; pdfium merender di atas latar putih, jadi cukup buang kanal alpha.
        img.into_rgb8().save_with_format(target, image::ImageFormat::Jpeg)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Memastikan pustaka Pdfium yang ikut dipaketkan benar-benar bisa dimuat saat runtime.
    /// Uji dilewati (bukan gagal) jika berkas resource belum tersedia di mesin ini.
    #[test]
    fn pdfium_bawaan_bisa_dimuat() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
        let lib = Pdfium::pdfium_platform_library_name_at_path(&dir);
        if !lib.is_file() {
            eprintln!("dilewati: {} tidak ada", lib.display());
            return;
        }
        let bindings = Pdfium::bind_to_library(&lib).expect("muat pustaka pdfium");
        // `Pdfium::new` memanggil FPDF_InitLibrary: jika DLL tidak cocok, langkah ini gagal/panik.
        let _pdfium = Pdfium::new(bindings);
    }
}
