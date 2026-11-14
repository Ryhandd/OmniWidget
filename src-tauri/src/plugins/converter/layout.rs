//! Rekonstruksi tata letak halaman PDF: karakter berposisi -> baris -> paragraf.
//!
//! Murni geometri, tanpa dependensi, sehingga bisa diuji tanpa Pdfium. Koordinat mengikuti PDF
//! (satuan point, sumbu Y naik). `pdf_docx` memakai hasilnya untuk menulis DOCX yang bisa diedit.

/// Satu karakter dengan kotak dan gayanya.
#[derive(Debug, Clone, PartialEq)]
pub struct Ch {
    pub c: char,
    pub x0: f32,
    pub x1: f32,
    pub y0: f32,
    pub y1: f32,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub color: [u8; 3],
    pub font: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub font: String,
    /// Ukuran dalam setengah point (satuan DOCX).
    pub half_pt: u32,
    pub bold: bool,
    pub italic: bool,
    pub color: [u8; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

/// Potongan baris yang dipisah celah lebar (kolom tabel, isian formulir).
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub x0: f32,
    pub x1: f32,
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub x0: f32,
    pub x1: f32,
    pub top: f32,
    /// Pendekatan garis dasar: sisi bawah kotak karakter.
    pub base: f32,
    pub size: f32,
    pub segments: Vec<Segment>,
    /// Baris diawali penanda daftar (bullet/nomor) yang dipisah celah dari teksnya.
    pub lead_tab: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Para {
    pub lines: Vec<Line>,
    pub align: Align,
    /// Tepi kiri teks (koordinat halaman).
    pub left: f32,
    /// Posisi baris pertama relatif terhadap `left`: positif = indentasi, negatif = menggantung.
    pub first: f32,
    /// 0 = bukan judul, 1..=3 = tingkat judul.
    pub heading: u8,
    /// Jarak antar baris (point) bila paragraf >1 baris, selain itu 1,2 x ukuran.
    pub pitch: f32,
}

const TAB_GAP: f32 = 1.6;
const SPACE_GAP: f32 = 0.18;

impl Line {
    pub fn text_len(&self) -> usize {
        self.segments.iter().flat_map(|s| &s.spans).map(|s| s.text.chars().count()).sum()
    }
    pub fn first_text(&self) -> &str {
        self.segments.first().and_then(|s| s.spans.first()).map(|s| s.text.as_str()).unwrap_or("")
    }
    pub fn last_text(&self) -> &str {
        self.segments.last().and_then(|s| s.spans.last()).map(|s| s.text.as_str()).unwrap_or("")
    }
    /// Baris satu blok teks (boleh berpenanda daftar): kandidat penyambungan paragraf.
    fn flowing(&self) -> bool {
        self.segments.len() == 1 || (self.lead_tab && self.segments.len() == 2)
    }
    fn is_bold(&self) -> bool {
        let (mut bold, mut all) = (0usize, 0usize);
        for s in self.segments.iter().flat_map(|s| &s.spans) {
            let n = s.text.chars().count();
            all += n;
            if s.style.bold {
                bold += n;
            }
        }
        all > 0 && bold * 2 >= all
    }
}

/// Nama font PDF -> nama font DOCX: buang prefiks subset dan sufiks gaya, beri spasi pada CamelCase.
pub fn clean_font(raw: &str) -> String {
    let b = raw.as_bytes();
    let name = if b.len() > 7 && b[6] == b'+' && b[..6].iter().all(|c| c.is_ascii_uppercase()) {
        &raw[7..]
    } else {
        raw
    };
    let base = name.split(|c| c == '-' || c == ',').next().unwrap_or("").trim();
    let mut base = base.to_string();
    for suffix in ["PSMT", "MT", "PS"] {
        if base.len() > suffix.len() + 2 && base.ends_with(suffix) {
            base.truncate(base.len() - suffix.len());
            break;
        }
    }
    let mut out = String::with_capacity(base.len() + 4);
    let mut prev: Option<char> = None;
    for c in base.chars() {
        if let Some(p) = prev {
            if p.is_ascii_lowercase() && c.is_ascii_uppercase() {
                out.push(' ');
            }
        }
        out.push(c);
        prev = Some(c);
    }
    if out.trim().is_empty() {
        "Calibri".to_string()
    } else {
        out
    }
}

fn style_of(c: &Ch) -> Style {
    Style {
        font: c.font.clone(),
        half_pt: ((c.size * 2.0).round() as u32).max(2),
        bold: c.bold,
        italic: c.italic,
        color: c.color,
    }
}

fn is_marker_text(t: &str) -> bool {
    let t = t.trim();
    let mut it = t.chars();
    let Some(first) = it.next() else { return false };
    if "•●▪■◦○–—-*·►➢✓".contains(first) {
        return it.as_str().trim().is_empty();
    }
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    let rest = &t[digits.len()..];
    if !digits.is_empty() && digits.len() <= 3 {
        return rest == "." || rest == ")";
    }
    let mut ch = t.chars();
    match (ch.next(), ch.next(), ch.next()) {
        (Some(a), Some('.' | ')'), None) => a.is_ascii_alphabetic(),
        _ => false,
    }
}

/// Teks diawali penanda daftar yang menyatu dengan isinya ("• isi", "1. isi").
fn starts_with_marker(t: &str) -> bool {
    let mut parts = t.splitn(2, char::is_whitespace);
    let head = parts.next().unwrap_or("");
    parts.next().is_some() && is_marker_text(head)
}

fn make_line(mut chars: Vec<Ch>) -> Line {
    chars.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));
    let n = chars.len() as f32;
    let avg_size = chars.iter().map(|c| c.size).sum::<f32>() / n;

    let mut segments: Vec<Segment> = Vec::new();
    let mut prev_x1: Option<f32> = None;
    let mut prev_size = avg_size;
    for ch in &chars {
        let gap = prev_x1.map(|p| ch.x0 - p);
        // Penanda daftar ("•", "1.") dipisah dari isinya pada celah yang lebih sempit, agar indentasi
        // menggantung bisa dihitung dari posisi teks isi.
        let after_marker = segments.len() == 1
            && segments[0].spans.len() == 1
            && is_marker_text(&segments[0].spans[0].text);
        let new_segment = match gap {
            None => true,
            Some(g) => g > TAB_GAP * prev_size || (after_marker && g > 0.5 * prev_size),
        };
        if new_segment {
            segments.push(Segment { x0: ch.x0, x1: ch.x1, spans: Vec::new() });
        } else if gap.is_some_and(|g| g > SPACE_GAP * prev_size) {
            if let Some(last) = segments.last_mut().and_then(|s| s.spans.last_mut()) {
                last.text.push(' ');
            }
        }
        let seg = segments.last_mut().expect("segmen ada");
        seg.x1 = seg.x1.max(ch.x1);
        let style = style_of(ch);
        match seg.spans.last_mut() {
            Some(last) if last.style == style => last.text.push(ch.c),
            _ => seg.spans.push(Span { text: ch.c.to_string(), style }),
        }
        prev_x1 = Some(ch.x1);
        prev_size = ch.size;
    }

    let mut counts: Vec<(u32, usize)> = Vec::new();
    for c in &chars {
        let k = ((c.size * 2.0).round() as u32).max(2);
        match counts.iter_mut().find(|(s, _)| *s == k) {
            Some(e) => e.1 += 1,
            None => counts.push((k, 1)),
        }
    }
    let size = counts.iter().max_by_key(|(_, n)| *n).map(|(s, _)| *s as f32 / 2.0).unwrap_or(avg_size);

    let lead_tab = segments.len() >= 2
        && segments[0].spans.len() == 1
        && is_marker_text(&segments[0].spans[0].text);
    Line {
        x0: chars.iter().map(|c| c.x0).fold(f32::MAX, f32::min),
        x1: chars.iter().map(|c| c.x1).fold(f32::MIN, f32::max),
        top: chars.iter().map(|c| c.y1).fold(f32::MIN, f32::max),
        base: chars.iter().map(|c| c.y0).sum::<f32>() / n,
        size,
        segments,
        lead_tab,
    }
}

/// Karakter satu halaman -> baris, dari atas ke bawah. Spasi diturunkan dari celah antarkarakter
/// (spasi bawaan PDF sering tak punya kotak yang valid), jadi karakter spasi dibuang dulu.
pub fn build_lines(mut chars: Vec<Ch>) -> Vec<Line> {
    chars.retain(|c| {
        !c.c.is_whitespace()
            && !c.c.is_control()
            && c.size > 0.0
            && c.x1 >= c.x0
            && c.y1 > c.y0
            && c.x0.is_finite()
            && c.y1.is_finite()
    });
    let cy = |c: &Ch| (c.y0 + c.y1) / 2.0;
    chars.sort_by(|a, b| cy(b).partial_cmp(&cy(a)).unwrap_or(std::cmp::Ordering::Equal));

    let mut groups: Vec<(f32, f32, Vec<Ch>)> = Vec::new();
    for ch in chars {
        let (c, h) = (cy(&ch), ch.y1 - ch.y0);
        match groups.last_mut() {
            Some(g) => {
                let n = g.2.len() as f32;
                if (c - g.0 / n).abs() <= 0.4 * (g.1 / n) {
                    g.0 += c;
                    g.1 += h;
                    g.2.push(ch);
                } else {
                    groups.push((c, h, vec![ch]));
                }
            }
            None => groups.push((c, h, vec![ch])),
        }
    }
    groups.into_iter().map(|g| make_line(g.2)).collect()
}

/// Ukuran huruf paling dominan (berdasarkan jumlah karakter) = ukuran teks isi dokumen.
pub fn dominant_size<'a>(lines: impl Iterator<Item = &'a Line>) -> f32 {
    let mut counts: Vec<(u32, usize)> = Vec::new();
    for l in lines {
        let k = (l.size * 2.0).round() as u32;
        match counts.iter_mut().find(|(s, _)| *s == k) {
            Some(e) => e.1 += l.text_len(),
            None => counts.push((k, l.text_len())),
        }
    }
    counts.iter().max_by_key(|(_, n)| *n).map(|(s, _)| *s as f32 / 2.0).unwrap_or(11.0)
}

/// Batas isi: (kiri, kanan, atas, bawah) dalam koordinat PDF.
pub fn extent(lines: &[Line]) -> Option<(f32, f32, f32, f32)> {
    if lines.is_empty() {
        return None;
    }
    let l = lines.iter().map(|x| x.x0).fold(f32::MAX, f32::min);
    let r = lines.iter().map(|x| x.x1).fold(f32::MIN, f32::max);
    let t = lines.iter().map(|x| x.top).fold(f32::MIN, f32::max);
    let b = lines.iter().map(|x| x.base).fold(f32::MAX, f32::min);
    Some((l, r, t, b))
}

fn continues(para: &[Line], cur: &Line, text_right: f32) -> bool {
    let prev = &para[para.len() - 1];
    if !prev.flowing() || !cur.flowing() {
        return false;
    }
    let size = prev.size.max(cur.size);
    let pitch = prev.base - cur.base;
    if pitch <= 0.0 || pitch > 1.5 * size || (prev.size - cur.size).abs() > 0.8 {
        return false;
    }
    if prev.x1 < text_right - 2.0 * size {
        return false; // baris sebelumnya berhenti sebelum tepi kanan: paragraf sudah selesai
    }
    if cur.lead_tab || starts_with_marker(cur.first_text()) {
        return false;
    }
    let first_is_item = para[0].lead_tab || starts_with_marker(para[0].first_text());
    if !first_is_item && cur.x0 > prev.x0 + 1.2 * size {
        return false; // indentasi baris pertama paragraf baru
    }
    true
}

fn make_para(lines: Vec<Line>, page_w: f32, text_left: f32, text_right: f32, body: f32) -> Para {
    let anchor = if lines.len() > 1 {
        lines[1].x0
    } else if lines[0].lead_tab {
        lines[0].segments[1].x0
    } else {
        lines[0].x0
    };
    let left = anchor;
    let first = lines[0].x0 - anchor;

    let l = lines.iter().map(|x| x.x0).fold(f32::MAX, f32::min);
    let r = lines.iter().map(|x| x.x1).fold(f32::MIN, f32::max);
    let single_block = lines.iter().all(|x| x.segments.len() == 1);
    let size = lines.iter().map(|x| x.size).fold(0.0, f32::max);

    let mut align = Align::Left;
    if single_block {
        let mid = (l + r) / 2.0;
        if (mid - page_w / 2.0).abs() <= 0.025 * page_w
            && l - text_left > 0.04 * page_w
            && text_right - r > 0.04 * page_w
        {
            align = Align::Center;
        } else if (r - text_right).abs() < 0.02 * page_w && l - text_left > 0.15 * page_w {
            align = Align::Right;
        } else if lines.len() >= 3
            && (text_right - r).abs() <= 0.03 * page_w
            && lines[..lines.len() - 1].iter().all(|x| (x.x1 - r).abs() <= 0.015 * page_w)
        {
            align = Align::Justify;
        }
    }

    let heading = if lines.len() == 1
        && single_block
        && !lines[0].lead_tab
        && lines[0].text_len() < 200
        && size >= 1.12 * body
        && (lines[0].is_bold() || size >= 1.3 * body)
    {
        if size >= 1.6 * body {
            1
        } else if size >= 1.3 * body {
            2
        } else {
            3
        }
    } else {
        0
    };

    let pitch = if lines.len() > 1 {
        let mut d: Vec<f32> = lines.windows(2).map(|w| w[0].base - w[1].base).collect();
        d.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        d[d.len() / 2]
    } else {
        size * 1.2
    };

    Para { lines, align, left, first, heading, pitch }
}

/// Baris satu halaman (atas ke bawah) -> paragraf.
pub fn build_paragraphs(lines: Vec<Line>, page_w: f32, body: f32) -> Vec<Para> {
    if lines.is_empty() {
        return Vec::new();
    }
    let text_left = lines.iter().map(|l| l.x0).fold(f32::MAX, f32::min);
    let long: Vec<&Line> = lines.iter().filter(|l| l.flowing() && l.text_len() >= 30).collect();
    let pool: Vec<&Line> = if long.is_empty() { lines.iter().collect() } else { long };
    let text_right = pool.iter().map(|l| l.x1).fold(f32::MIN, f32::max);

    let mut out = Vec::new();
    let mut cur: Vec<Line> = Vec::new();
    for line in lines {
        if !cur.is_empty() && !continues(&cur, &line, text_right) {
            out.push(make_para(std::mem::take(&mut cur), page_w, text_left, text_right, body));
        }
        cur.push(line);
    }
    if !cur.is_empty() {
        out.push(make_para(cur, page_w, text_left, text_right, body));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, x: f32, base: f32, size: f32, bold: bool) -> Vec<Ch> {
        let w = size * 0.5;
        text.chars()
            .enumerate()
            .map(|(i, c)| Ch {
                c,
                x0: x + w * i as f32,
                x1: x + w * (i + 1) as f32,
                y0: base,
                y1: base + size,
                size,
                bold,
                italic: false,
                color: [0, 0, 0],
                font: "Arial".into(),
            })
            .collect()
    }

    /// Satu baris: kata-kata dipisah celah `space` (dalam em).
    fn row(words: &[&str], x: f32, base: f32, size: f32, space: f32, bold: bool) -> Vec<Ch> {
        let mut x = x;
        let mut out = Vec::new();
        for w in words {
            let c = word(w, x, base, size, bold);
            x = c.last().map(|c| c.x1).unwrap_or(x) + space * size;
            out.extend(c);
        }
        out
    }

    fn text_of(l: &Line) -> String {
        l.segments
            .iter()
            .map(|s| s.spans.iter().map(|p| p.text.as_str()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\t")
    }

    #[test]
    fn nama_font_dibersihkan() {
        assert_eq!(clean_font("ABCDEF+TimesNewRomanPS-BoldMT"), "Times New Roman");
        assert_eq!(clean_font("ArialMT"), "Arial");
        assert_eq!(clean_font("CourierNewPSMT"), "Courier New");
        assert_eq!(clean_font("Calibri,Bold"), "Calibri");
        assert_eq!(clean_font(""), "Calibri");
    }

    #[test]
    fn spasi_diturunkan_dari_celah_dan_urutan_atas_bawah() {
        let mut chars = row(&["Halo", "dunia"], 72.0, 600.0, 12.0, 0.3, false);
        chars.extend(row(&["Baris", "kedua"], 72.0, 585.0, 12.0, 0.3, false));
        chars.reverse(); // urutan acak tidak boleh berpengaruh
        let lines = build_lines(chars);
        assert_eq!(lines.len(), 2);
        assert_eq!(text_of(&lines[0]), "Halo dunia");
        assert_eq!(text_of(&lines[1]), "Baris kedua");
    }

    #[test]
    fn celah_lebar_menjadi_segmen_tab() {
        let lines = build_lines(row(&["Nama", "Nilai"], 72.0, 500.0, 12.0, 4.0, false));
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].segments.len(), 2);
        assert_eq!(text_of(&lines[0]), "Nama\tNilai");
    }

    #[test]
    fn karakter_tanpa_kotak_valid_dibuang() {
        let mut chars = word("Ok", 72.0, 500.0, 12.0, false);
        chars.push(Ch { c: 'X', x0: 0.0, x1: 0.0, y0: 0.0, y1: 0.0, size: 12.0, bold: false, italic: false, color: [0; 3], font: "Arial".into() });
        chars.push(Ch { c: ' ', x0: 90.0, x1: 94.0, y0: 500.0, y1: 512.0, size: 12.0, bold: false, italic: false, color: [0; 3], font: "Arial".into() });
        let lines = build_lines(chars);
        assert_eq!(lines.len(), 1);
        assert_eq!(text_of(&lines[0]), "Ok");
    }

    #[test]
    fn baris_bersambung_menjadi_satu_paragraf_dan_berhenti_di_baris_pendek() {
        let w = 595.0;
        let long = ["aaaa", "bbbb", "cccc", "dddd", "eeee", "ffff", "gggg", "hhhh", "iiii", "jjjj", "kkkk", "llll"];
        let mut chars = Vec::new();
        chars.extend(row(&long, 72.0, 700.0, 12.0, 0.3, false));
        chars.extend(row(&long, 72.0, 686.0, 12.0, 0.3, false));
        chars.extend(row(&["akhir"], 72.0, 672.0, 12.0, 0.3, false)); // pendek: paragraf selesai
        chars.extend(row(&long, 72.0, 640.0, 12.0, 0.3, false)); // paragraf baru (jarak besar)
        let paras = build_paragraphs(build_lines(chars), w, 12.0);
        assert_eq!(paras.len(), 2);
        assert_eq!(paras[0].lines.len(), 3);
        assert_eq!(paras[1].lines.len(), 1);
        assert!((paras[0].pitch - 14.0).abs() < 0.01);
    }

    #[test]
    fn judul_terdeteksi_dari_ukuran() {
        let mut chars = row(&["BAB", "I"], 200.0, 750.0, 20.0, 0.3, true);
        chars.extend(row(&["isi", "teks", "biasa"], 72.0, 700.0, 12.0, 0.3, false));
        let lines = build_lines(chars);
        let paras = build_paragraphs(lines, 595.0, 12.0);
        assert_eq!(paras[0].heading, 1);
        assert_eq!(paras[1].heading, 0);
    }

    #[test]
    fn rata_tengah_terdeteksi() {
        let mut chars = row(&["Judul", "Tengah"], 0.0, 750.0, 14.0, 0.3, false);
        let w = chars.last().unwrap().x1;
        let shift = 595.0 / 2.0 - w / 2.0;
        for c in &mut chars {
            c.x0 += shift;
            c.x1 += shift;
        }
        chars.extend(row(&["isi", "teks", "yang", "panjang", "sekali", "sampai", "ujung", "kanan"], 72.0, 700.0, 14.0, 0.3, false));
        let paras = build_paragraphs(build_lines(chars), 595.0, 14.0);
        assert_eq!(paras[0].align, Align::Center);
    }

    #[test]
    fn daftar_berpenanda_menggantung_dan_tidak_menyatu() {
        let mut chars = word("•", 72.0, 700.0, 12.0, false);
        chars.extend(row(&["butir", "pertama", "yang", "cukup", "panjang", "untuk", "sekali", "membungkus"], 90.0, 700.0, 12.0, 0.3, false));
        chars.extend(row(&["lanjutan", "butir", "pertama", "yang", "cukup", "panjang", "untuk", "membungkus"], 90.0, 686.0, 12.0, 0.3, false));
        chars.extend(word("•", 72.0, 672.0, 12.0, false));
        chars.extend(row(&["butir", "kedua"], 90.0, 672.0, 12.0, 0.3, false));
        let paras = build_paragraphs(build_lines(chars), 595.0, 12.0);
        assert_eq!(paras.len(), 2);
        assert_eq!(paras[0].lines.len(), 2);
        assert!((paras[0].left - 90.0).abs() < 0.01);
        assert!((paras[0].first + 18.0).abs() < 0.01);
        assert!((paras[1].first + 18.0).abs() < 0.01);
    }

    #[test]
    fn penanda_dikenali() {
        assert!(is_marker_text("•"));
        assert!(is_marker_text("12."));
        assert!(is_marker_text("a)"));
        assert!(!is_marker_text("Halo"));
        assert!(starts_with_marker("1. Isi butir"));
        assert!(!starts_with_marker("1.5 liter"));
    }
}
