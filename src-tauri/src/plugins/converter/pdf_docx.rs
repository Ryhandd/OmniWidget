//! PDF -> DOCX yang mengikuti tata letak asli dan tetap bisa diedit.
//!
//! Pdfium memberi karakter berposisi; `layout` merangkainya menjadi baris dan paragraf. Hasilnya
//! ditulis sebagai paragraf DOCX sungguhan (run berfont/ukuran/warna, rata, indentasi, jarak,
//! judul) dengan ukuran halaman dan margin dari PDF, bukan kotak teks atau gambar halaman.
//! Gambar diletakkan melayang pada koordinat aslinya. Tabel tidak dibangun ulang: kolomnya
//! dijaga lewat tab stop. PDF hasil pindai (tanpa teks) hanya menghasilkan gambar tiap halaman.

use super::{
    error::ConvError,
    layout::{self, Align, Ch, Line, Para},
    CancelToken,
};
use docx_rs::{
    AlignmentType, Docx, LineSpacing, LineSpacingType, PageMargin, Paragraph, Pic,
    RelativeFromHType, RelativeFromVType, Run, RunFonts, SpecialIndentType, Style,
    StyleType, Tab, TabValueType,
};
use pdfium_render::prelude::*;
use std::{path::Path, sync::atomic::Ordering};

const MAX_IMAGES: usize = 300;
const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
/// Margin minimum (point): ujung teks PDF sering nyaris di tepi kertas.
const MIN_MARGIN: f32 = 18.0;

struct PageImage {
    png: Vec<u8>,
    x: f32,
    /// Sisi atas gambar (koordinat PDF, Y naik).
    top: f32,
    w: f32,
    h: f32,
}

struct PageData {
    w: f32,
    h: f32,
    lines: Vec<Line>,
    images: Vec<PageImage>,
}

pub fn write(
    document: &PdfDocument,
    output: &Path,
    cancel: &CancelToken,
    progress: &dyn Fn(f32),
) -> Result<(), ConvError> {
    let total = document.pages().len() as usize;
    let mut pages: Vec<PageData> = Vec::with_capacity(total);
    let (mut image_count, mut image_bytes) = (0usize, 0usize);

    for (i, page) in document.pages().iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(ConvError::Cancelled);
        }
        let (w, h) = (page.width().value, page.height().value);
        let lines = layout::build_lines(extract_chars(&page));
        let images = extract_images(&page, document, w, h, !lines.is_empty(), &mut image_count, &mut image_bytes);
        pages.push(PageData { w, h, lines, images });
        progress(0.05 + 0.7 * ((i + 1) as f32 / total as f32));
    }

    if pages.iter().all(|p| p.lines.is_empty() && p.images.is_empty()) {
        return Err(ConvError::Pdf(
            "Tidak ada teks atau gambar yang bisa dibaca (kemungkinan PDF hasil pindai tanpa lapisan teks)".into(),
        ));
    }

    let body = layout::dominant_size(pages.iter().flat_map(|p| p.lines.iter()));
    let (w0, h0) = (pages[0].w, pages[0].h);

    // Margin dari batas isi seluruh halaman.
    let mut ext: Option<(f32, f32, f32, f32)> = None;
    for p in &pages {
        if let Some((l, r, t, b)) = layout::extent(&p.lines) {
            ext = Some(match ext {
                None => (l, r, t, b),
                Some((l0, r0, t0, b0)) => (l0.min(l), r0.max(r), t0.max(t), b0.min(b)),
            });
        }
    }
    let (left, right, top, bottom) = ext.unwrap_or((72.0, w0 - 72.0, h0 - 72.0, 72.0));
    let margin_l = left.max(MIN_MARGIN);
    let margin_r = (w0 - right).max(MIN_MARGIN);
    let margin_t = (h0 - top).max(MIN_MARGIN);
    let margin_b = bottom.max(MIN_MARGIN);
    let top_edge = h0 - margin_t; // Y tepi atas area isi

    let mut docx = Docx::new()
        .page_size(tw(w0) as u32, tw(h0) as u32)
        .page_margin(
            PageMargin::new()
                .top(tw(margin_t))
                .bottom(tw(margin_b))
                .left(tw(margin_l))
                .right(tw(margin_r))
                .header(0)
                .footer(0),
        );
    for level in 1..=3 {
        docx = docx.add_style(Style::new(format!("Heading{level}"), StyleType::Paragraph));
    }

    let ctx = Ctx { margin_l, top_edge };
    for (index, page) in pages.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(ConvError::Cancelled);
        }
        let h = page.h;
        let paras = layout::build_paragraphs(page.lines, page.w, body);
        let mut images = page.images;
        let mut prev_base: Option<f32> = None;

        if paras.is_empty() {
            // Halaman tanpa teks (mis. hasil pindai): paragraf kosong sebagai jangkar gambar.
            let mut p = Paragraph::new().page_break_before(index > 0).line_spacing(
                LineSpacing::new().before(0).after(0).line(20).line_rule(LineSpacingType::Exact),
            );
            for img in images.drain(..) {
                p = p.add_run(Run::new().add_image(floating(&img, h)));
            }
            docx = docx.add_paragraph(p);
            continue;
        }

        for (k, para) in paras.iter().enumerate() {
            let before_pt = match prev_base {
                Some(pb) => pb - para.lines[0].base - para.pitch.min(para.lines[0].size * 1.2).max(1.0),
                None => top_edge - para.lines[0].top,
            };
            let mut p = build_paragraph(para, before_pt.max(0.0), &ctx);
            if k == 0 {
                p = p.page_break_before(index > 0);
                // Gambar melayang dijangkarkan di paragraf pertama halamannya.
                for img in images.drain(..) {
                    p = p.add_run(Run::new().add_image(floating(&img, h)));
                }
            }
            docx = docx.add_paragraph(p);
            prev_base = Some(para.lines[para.lines.len() - 1].base);
        }
        progress(0.75 + 0.2 * ((index + 1) as f32 / total as f32));
    }

    let file = std::fs::File::create(output)?;
    docx.build()
        .pack(file)
        .map_err(|e| ConvError::Doc(format!("Gagal menulis DOCX: {e}")))?;
    Ok(())
}

struct Ctx {
    margin_l: f32,
    top_edge: f32,
}

/// point -> twip (1 pt = 20 twip).
fn tw(pt: f32) -> i32 {
    (pt * 20.0).round() as i32
}

fn emu(pt: f32) -> u32 {
    (pt.max(0.0) * 12700.0).round() as u32
}

fn floating(img: &PageImage, page_h: f32) -> Pic {
    Pic::new(&img.png)
        .size(emu(img.w), emu(img.h))
        .floating()
        .relative_from_h(RelativeFromHType::Page)
        .relative_from_v(RelativeFromVType::Page)
        .offset_x((img.x * 12700.0).round() as i32)
        .offset_y(((page_h - img.top) * 12700.0).round() as i32)
}

enum Item<'a> {
    Text(&'a layout::Span, bool),
    Tab,
}

fn run_of(span: &layout::Span, trailing_space: bool) -> Run {
    let font = span.style.font.as_str();
    let mut text = span.text.clone();
    if trailing_space {
        text.push(' ');
    }
    let mut run = Run::new()
        .add_text(text)
        .size(span.style.half_pt as usize)
        .fonts(RunFonts::new().ascii(font).hi_ansi(font).east_asia(font).cs(font));
    if span.style.bold {
        run = run.bold();
    }
    if span.style.italic {
        run = run.italic();
    }
    let [r, g, b] = span.style.color;
    if r > 24 || g > 24 || b > 24 {
        run = run.color(format!("{r:02X}{g:02X}{b:02X}"));
    }
    run
}

/// Urutan run satu paragraf: antar baris disambung spasi (tanpa spasi setelah tanda hubung),
/// antar segmen disambung tab.
fn items(para: &Para) -> Vec<Item<'_>> {
    let mut out = Vec::new();
    for (li, line) in para.lines.iter().enumerate() {
        for (si, seg) in line.segments.iter().enumerate() {
            if si > 0 {
                out.push(Item::Tab);
            }
            let last_seg = si + 1 == line.segments.len();
            for (pi, span) in seg.spans.iter().enumerate() {
                let last_span = pi + 1 == seg.spans.len();
                let join = li + 1 < para.lines.len()
                    && last_seg
                    && last_span
                    && !span.text.ends_with('-');
                out.push(Item::Text(span, join));
            }
        }
    }
    out
}

fn build_paragraph(para: &Para, before_pt: f32, ctx: &Ctx) -> Paragraph {
    let mut p = Paragraph::new();
    for item in items(para) {
        p = match item {
            Item::Text(span, join) => p.add_run(run_of(span, join)),
            Item::Tab => p.add_run(Run::new().add_tab()),
        };
    }

    p = p.align(match para.align {
        Align::Left => AlignmentType::Left,
        Align::Center => AlignmentType::Center,
        Align::Right => AlignmentType::Right,
        Align::Justify => AlignmentType::Both,
    });

    if matches!(para.align, Align::Left | Align::Justify) {
        let left = tw((para.left - ctx.margin_l).max(0.0));
        let special = if para.first > 0.5 {
            Some(SpecialIndentType::FirstLine(tw(para.first)))
        } else if para.first < -0.5 {
            Some(SpecialIndentType::Hanging(tw(-para.first)))
        } else {
            None
        };
        p = p.indent(Some(left), special, None, None);
    }

    // Baris satu paragraf dengan beberapa segmen (kolom): tab stop di posisi asli tiap segmen.
    // Penanda daftar memakai indentasi menggantung, jadi segmen pertama-kedua tak butuh tab stop.
    if para.lines.len() == 1 {
        let line = &para.lines[0];
        let skip = if line.lead_tab { 2 } else { 1 };
        for seg in line.segments.iter().skip(skip) {
            p = p.add_tab(
                Tab::new()
                    .val(TabValueType::Left)
                    .pos(tw((seg.x0 - ctx.margin_l).max(0.0)) as usize),
            );
        }
    }

    if para.heading > 0 {
        p = p.style(&format!("Heading{}", para.heading));
    }

    p.line_spacing(
        LineSpacing::new()
            .before(tw(before_pt) as u32)
            .after(0)
            .line(tw(para.pitch).max(20))
            .line_rule(LineSpacingType::Exact),
    )
}

// ---------- Ekstraksi Pdfium ----------

fn extract_chars(page: &PdfPage) -> Vec<Ch> {
    let Ok(text) = page.text() else { return Vec::new() };
    let mut out = Vec::new();
    for ch in text.chars().iter() {
        let Some(c) = ch.unicode_char() else { continue };
        if c.is_whitespace() || c.is_control() {
            continue;
        }
        let Ok(b) = ch.loose_bounds() else { continue };
        let name = ch.font_name();
        let lower = name.to_ascii_lowercase();
        let color = ch
            .fill_color()
            .map(|c| [c.red(), c.green(), c.blue()])
            .unwrap_or([0, 0, 0]);
        out.push(Ch {
            c,
            x0: b.left().value,
            x1: b.right().value,
            y0: b.bottom().value,
            y1: b.top().value,
            size: ch.scaled_font_size().value,
            bold: ["bold", "black", "heavy"].iter().any(|k| lower.contains(k)),
            italic: lower.contains("italic") || lower.contains("oblique"),
            color,
            font: layout::clean_font(&name),
        });
    }
    out
}

/// Gambar halaman sebagai PNG berikut kotaknya. Gambar yang nyaris menutupi seluruh halaman
/// dilewati bila halaman punya teks (latar/watermark): sebagai gambar melayang ia akan menimpa teks.
fn extract_images(
    page: &PdfPage,
    document: &PdfDocument,
    page_w: f32,
    page_h: f32,
    has_text: bool,
    count: &mut usize,
    bytes: &mut usize,
) -> Vec<PageImage> {
    let mut out = Vec::new();
    for object in page.objects().iter() {
        if *count >= MAX_IMAGES || *bytes >= MAX_IMAGE_BYTES {
            break;
        }
        let Some(image) = object.as_image_object() else { continue };
        let Ok(q) = object.bounds() else { continue };
        let xs = [q.x1.value, q.x2.value, q.x3.value, q.x4.value];
        let ys = [q.y1.value, q.y2.value, q.y3.value, q.y4.value];
        let x0 = xs.iter().cloned().fold(f32::MAX, f32::min);
        let x1 = xs.iter().cloned().fold(f32::MIN, f32::max);
        let y0 = ys.iter().cloned().fold(f32::MAX, f32::min);
        let y1 = ys.iter().cloned().fold(f32::MIN, f32::max);
        let (w, h) = (x1 - x0, y1 - y0);
        if w < 2.0 || h < 2.0 {
            continue;
        }
        if has_text && w * h > 0.9 * page_w * page_h {
            continue;
        }
        let Ok(img) = image.get_processed_image(document) else { continue };
        let mut png = Vec::new();
        if img
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .is_err()
        {
            continue;
        }
        *count += 1;
        *bytes += png.len();
        out.push(PageImage { png, x: x0, top: y1, w, h });
    }
    out
}
