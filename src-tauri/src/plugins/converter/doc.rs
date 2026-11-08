//! Konversi dokumen tanpa Pandoc.
//!
//! Pipeline: DOCX -> blok -> (Markdown | HTML | teks), dan Markdown/HTML -> blok -> DOCX.
//! Markdown ke HTML memakai `pulldown-cmark`; HTML dibaca dengan `scraper` (MIT);
//! DOCX ditulis dengan `docx-rs` dan dibaca manual dari `word/document.xml`.

use super::{error::ConvError, formats::Format, CancelToken};
use ego_tree::NodeRef;
use scraper::{Html, Node};
use std::{io::Read, path::Path, sync::atomic::Ordering};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Block {
    Heading(u8, String),
    Paragraph(String),
    Quote(String),
    Code(String),
    List { ordered: bool, items: Vec<String> },
}

pub fn convert(
    input: &Path,
    output: &Path,
    from: Format,
    to: Format,
    cancel: &CancelToken,
    progress: &dyn Fn(f32),
) -> Result<(), ConvError> {
    check(cancel)?;
    let blocks = match from {
        Format::Docx => read_docx(input)?,
        Format::Md => {
            let text = read_text(input)?;
            if to == Format::Html {
                progress(0.4);
                let html = markdown_to_html(&text);
                std::fs::write(output, html)?;
                progress(1.0);
                return Ok(());
            }
            parse_markdown(&text)
        }
        Format::Html => {
            let html = read_text(input)?;
            let md = html_to_markdown(&html);
            if to == Format::Md {
                std::fs::write(output, md)?;
                progress(1.0);
                return Ok(());
            }
            parse_markdown(&md)
        }
        _ => return Err(ConvError::NoRoute),
    };
    progress(0.4);
    check(cancel)?;

    match to {
        Format::Md => std::fs::write(output, blocks_to_markdown(&blocks))?,
        Format::Html => std::fs::write(output, markdown_to_html(&blocks_to_markdown(&blocks)))?,
        Format::Txt => std::fs::write(output, blocks_to_text(&blocks))?,
        Format::Docx => write_docx(&blocks, output)?,
        _ => return Err(ConvError::NoRoute),
    }
    progress(1.0);
    Ok(())
}

fn check(cancel: &CancelToken) -> Result<(), ConvError> {
    if cancel.load(Ordering::Relaxed) {
        Err(ConvError::Cancelled)
    } else {
        Ok(())
    }
}

fn read_text(path: &Path) -> Result<String, ConvError> {
    let mut text = String::new();
    std::fs::File::open(path)?.read_to_string(&mut text)?;
    Ok(text)
}

// ---------- Markdown -> HTML ----------

fn markdown_to_html(md: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(md, options);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

// ---------- HTML -> Markdown ----------

fn html_to_markdown(html: &str) -> String {
    let document = Html::parse_document(html);
    let mut out = String::new();
    for child in document.tree.root().children() {
        walk_html(child, &mut out);
    }
    cleanup_markdown(out)
}

fn walk_html(node: NodeRef<Node>, out: &mut String) {
    match node.value() {
        Node::Text(text) => out.push_str(&collapse_ws(text)),
        Node::Element(element) => {
            let name = element.name();
            match name {
                "script" | "style" | "head" | "meta" | "link" | "noscript" | "svg" | "template" => {}
                "br" => out.push('\n'),
                "hr" => out.push_str("\n\n---\n\n"),
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let level = name[1..].parse::<usize>().unwrap_or(1);
                    out.push_str("\n\n");
                    out.push_str(&"#".repeat(level));
                    out.push(' ');
                    walk_children(node, out);
                    out.push_str("\n\n");
                }
                "p" | "div" | "section" | "article" | "header" | "footer" | "main" | "body" | "table" | "tr" => {
                    out.push_str("\n\n");
                    walk_children(node, out);
                    out.push_str("\n\n");
                }
                "strong" | "b" => {
                    out.push_str("**");
                    walk_children(node, out);
                    out.push_str("**");
                }
                "em" | "i" => {
                    out.push('*');
                    walk_children(node, out);
                    out.push('*');
                }
                "code" => {
                    out.push('`');
                    walk_children(node, out);
                    out.push('`');
                }
                "pre" => {
                    out.push_str("\n\n```\n");
                    out.push_str(&collect_text(node));
                    out.push_str("\n```\n\n");
                }
                "blockquote" => {
                    let mut inner = String::new();
                    walk_children(node, &mut inner);
                    for line in inner.trim().lines() {
                        out.push_str("> ");
                        out.push_str(line);
                        out.push('\n');
                    }
                    out.push('\n');
                }
                "ul" | "ol" => {
                    out.push('\n');
                    walk_children(node, out);
                    out.push('\n');
                }
                "li" => {
                    out.push_str("\n- ");
                    walk_children(node, out);
                }
                "a" => {
                    let href = element.attr("href").unwrap_or("");
                    out.push('[');
                    walk_children(node, out);
                    out.push_str("](");
                    out.push_str(href);
                    out.push(')');
                }
                "img" => {
                    let alt = element.attr("alt").unwrap_or("");
                    let src = element.attr("src").unwrap_or("");
                    out.push_str("![");
                    out.push_str(alt);
                    out.push_str("](");
                    out.push_str(src);
                    out.push(')');
                }
                "td" | "th" => {
                    out.push_str(" | ");
                    walk_children(node, out);
                }
                _ => walk_children(node, out),
            }
        }
        _ => walk_children(node, out),
    }
}

fn walk_children(node: NodeRef<Node>, out: &mut String) {
    for child in node.children() {
        walk_html(child, out);
    }
}

fn collect_text(node: NodeRef<Node>) -> String {
    let mut out = String::new();
    match node.value() {
        Node::Text(text) => out.push_str(text),
        _ => {
            for child in node.children() {
                out.push_str(&collect_text(child));
            }
        }
    }
    out
}

fn collapse_ws(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            space = true;
        } else {
            if space && !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            space = false;
            out.push(ch);
        }
    }
    out
}

fn cleanup_markdown(text: String) -> String {
    let mut lines: Vec<String> = text.lines().map(|l| l.trim_end().to_string()).collect();
    // Rapatkan baris kosong beruntun.
    let mut out = String::new();
    let mut blank = 0;
    for line in lines.drain(..) {
        if line.is_empty() {
            blank += 1;
            if blank > 2 {
                continue;
            }
        } else {
            blank = 0;
        }
        out.push_str(&line);
        out.push('\n');
    }
    out.trim().to_string() + "\n"
}

// ---------- Markdown -> blok ----------

fn parse_markdown(md: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    let mut code: Option<String> = None;
    let mut quote: Vec<String> = Vec::new();
    let mut list: Option<(bool, Vec<String>)> = None;

    let flush_paragraph = |paragraph: &mut Vec<String>, blocks: &mut Vec<Block>| {
        if !paragraph.is_empty() {
            blocks.push(Block::Paragraph(paragraph.join(" ")));
            paragraph.clear();
        }
    };
    let flush_quote = |quote: &mut Vec<String>, blocks: &mut Vec<Block>| {
        if !quote.is_empty() {
            blocks.push(Block::Quote(quote.join(" ")));
            quote.clear();
        }
    };
    let flush_list = |list: &mut Option<(bool, Vec<String>)>, blocks: &mut Vec<Block>| {
        if let Some((ordered, items)) = list.take() {
            if !items.is_empty() {
                blocks.push(Block::List { ordered, items });
            }
        }
    };

    for line in md.lines() {
        if let Some(fence) = code.as_mut() {
            if line.trim_start().starts_with("```") {
                blocks.push(Block::Code(code.take().unwrap_or_default()));
            } else {
                fence.push_str(line);
                fence.push('\n');
            }
            continue;
        }

        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            flush_paragraph(&mut paragraph, &mut blocks);
            flush_quote(&mut quote, &mut blocks);
            flush_list(&mut list, &mut blocks);
            code = Some(String::new());
            continue;
        }
        if trimmed.is_empty() {
            flush_paragraph(&mut paragraph, &mut blocks);
            flush_quote(&mut quote, &mut blocks);
            flush_list(&mut list, &mut blocks);
            continue;
        }

        if let Some((level, text)) = heading(trimmed) {
            flush_paragraph(&mut paragraph, &mut blocks);
            flush_quote(&mut quote, &mut blocks);
            flush_list(&mut list, &mut blocks);
            blocks.push(Block::Heading(level, strip_inline(text)));
            continue;
        }

        if trimmed.starts_with('>') {
            flush_paragraph(&mut paragraph, &mut blocks);
            flush_list(&mut list, &mut blocks);
            quote.push(strip_inline(trimmed.trim_start_matches('>').trim()));
            continue;
        }

        if let Some((ordered, item)) = list_item(trimmed) {
            flush_paragraph(&mut paragraph, &mut blocks);
            flush_quote(&mut quote, &mut blocks);
            match &mut list {
                Some((kind, items)) if *kind == ordered => items.push(strip_inline(item)),
                _ => {
                    flush_list(&mut list, &mut blocks);
                    list = Some((ordered, vec![strip_inline(item)]));
                }
            }
            continue;
        }

        flush_quote(&mut quote, &mut blocks);
        flush_list(&mut list, &mut blocks);
        paragraph.push(strip_inline(trimmed));
    }

    if let Some(text) = code {
        blocks.push(Block::Code(text));
    }
    flush_paragraph(&mut paragraph, &mut blocks);
    flush_quote(&mut quote, &mut blocks);
    flush_list(&mut list, &mut blocks);
    blocks
}

fn heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        Some((hashes as u8, line[hashes..].trim()))
    } else {
        None
    }
}

fn list_item(line: &str) -> Option<(bool, &str)> {
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(marker) {
            return Some((false, rest));
        }
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        let rest = &line[digits..];
        if let Some(rest) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            return Some((true, rest));
        }
    }
    None
}

/// Buang penanda Markdown inline sederhana untuk output teks/DOCX.
fn strip_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '`' | '*' | '_' | '~' => i += 1,
            '!' if i + 1 < chars.len() && chars[i + 1] == '[' => i += 1,
            '[' => {
                // [teks](url) -> teks (url)
                if let Some(close) = find_char(&chars, i + 1, ']') {
                    let label: String = chars[i + 1..close].iter().collect();
                    out.push_str(&label);
                    if close + 1 < chars.len() && chars[close + 1] == '(' {
                        if let Some(end) = find_char(&chars, close + 2, ')') {
                            let url: String = chars[close + 2..end].iter().collect();
                            out.push_str(" (");
                            out.push_str(&url);
                            out.push(')');
                            i = end + 1;
                            continue;
                        }
                    }
                    i = close + 1;
                } else {
                    out.push('[');
                    i += 1;
                }
            }
            '\\' if i + 1 < chars.len() => {
                out.push(chars[i + 1]);
                i += 2;
            }
            ch => {
                out.push(ch);
                i += 1;
            }
        }
    }
    out
}

fn find_char(chars: &[char], from: usize, needle: char) -> Option<usize> {
    (from..chars.len()).find(|i| chars[*i] == needle)
}

// ---------- Blok -> format ----------

fn blocks_to_markdown(blocks: &[Block]) -> String {
    let mut out = String::new();
    for block in blocks {
        match block {
            Block::Heading(level, text) => {
                out.push_str(&"#".repeat(*level as usize));
                out.push(' ');
                out.push_str(text);
                out.push_str("\n\n");
            }
            Block::Paragraph(text) => {
                out.push_str(text);
                out.push_str("\n\n");
            }
            Block::Quote(text) => {
                for line in text.lines() {
                    out.push_str("> ");
                    out.push_str(line);
                    out.push('\n');
                }
                out.push('\n');
            }
            Block::Code(text) => {
                out.push_str("```\n");
                out.push_str(text.trim_end());
                out.push_str("\n```\n\n");
            }
            Block::List { ordered, items } => {
                for (i, item) in items.iter().enumerate() {
                    if *ordered {
                        out.push_str(&format!("{}. ", i + 1));
                    } else {
                        out.push_str("- ");
                    }
                    out.push_str(item);
                    out.push('\n');
                }
                out.push('\n');
            }
        }
    }
    out.trim().to_string() + "\n"
}

fn blocks_to_text(blocks: &[Block]) -> String {
    let mut out = String::new();
    for block in blocks {
        match block {
            Block::Heading(_, text) | Block::Paragraph(text) | Block::Quote(text) => {
                out.push_str(text);
                out.push_str("\n\n");
            }
            Block::Code(text) => {
                out.push_str(text.trim_end());
                out.push_str("\n\n");
            }
            Block::List { ordered, items } => {
                for (i, item) in items.iter().enumerate() {
                    if *ordered {
                        out.push_str(&format!("{i}. "));
                    } else {
                        out.push_str("• ");
                    }
                    out.push_str(item);
                    out.push('\n');
                }
                out.push('\n');
            }
        }
    }
    out.trim().to_string() + "\n"
}

// ---------- Tulis DOCX ----------

fn write_docx(blocks: &[Block], output: &Path) -> Result<(), ConvError> {
    use docx_rs::{Docx, Paragraph, Run, Style, StyleType};

    let mut docx = Docx::new();
    for level in 1..=6 {
        docx = docx.add_style(Style::new(format!("Heading{level}"), StyleType::Paragraph));
    }

    for block in blocks {
        docx = match block {
            Block::Heading(level, text) => {
                let style = format!("Heading{level}");
                docx.add_paragraph(
                    Paragraph::new()
                        .style(style.as_str())
                        .add_run(Run::new().add_text(text.as_str())),
                )
            }
            Block::Paragraph(text) => {
                docx.add_paragraph(Paragraph::new().add_run(Run::new().add_text(text.as_str())))
            }
            Block::Quote(text) => docx.add_paragraph(
                Paragraph::new().add_run(Run::new().add_text(text.as_str()).italic()),
            ),
            Block::Code(text) => {
                let mut paragraph = Paragraph::new();
                for (i, line) in text.trim_end().lines().enumerate() {
                    if i > 0 {
                        paragraph = paragraph.add_run(Run::new().add_text("\n"));
                    }
                    paragraph = paragraph.add_run(Run::new().add_text(line));
                }
                docx.add_paragraph(paragraph)
            }
            Block::List { ordered, items } => {
                let mut docx = docx;
                for (i, item) in items.iter().enumerate() {
                    let prefix = if *ordered {
                        format!("{}. ", i + 1)
                    } else {
                        "• ".to_string()
                    };
                    docx = docx.add_paragraph(
                        Paragraph::new().add_run(Run::new().add_text(format!("{prefix}{item}"))),
                    );
                }
                docx
            }
        };
    }

    let file = std::fs::File::create(output)?;
    docx.build()
        .pack(file)
        .map_err(|e| ConvError::Doc(format!("Gagal menulis DOCX: {e}")))?;
    Ok(())
}

// ---------- Baca DOCX ----------

fn read_docx(path: &Path) -> Result<Vec<Block>, ConvError> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| ConvError::Doc(format!("DOCX tidak valid: {e}")))?;
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .map_err(|e| ConvError::Doc(format!("Bagian document.xml tidak ditemukan: {e}")))?
        .read_to_string(&mut xml)?;
    parse_docx_xml(&xml)
}

fn parse_docx_xml(xml: &str) -> Result<Vec<Block>, ConvError> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut blocks = Vec::new();
    let mut text = String::new();
    let mut heading: Option<u8> = None;
    let mut in_paragraph = false;
    let mut in_text = false;
    let mut ordered: Option<bool> = None;
    let mut in_list_item = false;

    loop {
        match reader.read_event() {
            Err(e) => return Err(ConvError::Doc(format!("XML DOCX tidak valid: {e}"))),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"w:p" => {
                    in_paragraph = true;
                    text.clear();
                    heading = None;
                }
                b"w:pStyle" => {
                    if let Some(value) = attr_value(&e, b"w:val") {
                        heading = style_heading(&value);
                        if value == "ListParagraph" || value.starts_with("List") {
                            in_list_item = true;
                        }
                    }
                }
                b"w:numPr" => in_list_item = true,
                b"w:t" => in_text = true,
                b"w:tab" => text.push('\t'),
                b"w:br" | b"w:cr" => text.push('\n'),
                _ => {}
            },
            Ok(Event::Empty(e)) => match e.name().as_ref() {
                b"w:pStyle" => {
                    if let Some(value) = attr_value(&e, b"w:val") {
                        heading = style_heading(&value);
                        if value == "ListParagraph" || value.starts_with("List") {
                            in_list_item = true;
                        }
                    }
                }
                b"w:numPr" => in_list_item = true,
                b"w:tab" => text.push('\t'),
                b"w:br" | b"w:cr" => text.push('\n'),
                _ => {}
            },
            Ok(Event::Text(e)) => {
                if in_text {
                    let decoded = e
                        .unescape()
                        .map_err(|err| ConvError::Doc(format!("Teks DOCX tidak valid: {err}")))?;
                    text.push_str(&decoded);
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"w:t" => in_text = false,
                b"w:p" => {
                    if in_paragraph {
                        push_docx_paragraph(&mut blocks, text.trim(), heading, in_list_item, &mut ordered);
                    }
                    in_paragraph = false;
                    in_list_item = false;
                }
                b"w:tbl" => ordered = None,
                _ => {}
            },
            _ => {}
        }
    }
    Ok(blocks)
}

fn push_docx_paragraph(
    blocks: &mut Vec<Block>,
    text: &str,
    heading: Option<u8>,
    in_list_item: bool,
    ordered: &mut Option<bool>,
) {
    if text.is_empty() {
        return;
    }
    if let Some(level) = heading {
        *ordered = None;
        blocks.push(Block::Heading(level, text.to_string()));
        return;
    }
    if in_list_item {
        let kind = *ordered.get_or_insert(false);
        let same = matches!(
            blocks.last(),
            Some(Block::List { ordered, .. }) if *ordered == kind
        );
        if same {
            if let Some(Block::List { items, .. }) = blocks.last_mut() {
                items.push(text.to_string());
            }
        } else {
            blocks.push(Block::List {
                ordered: kind,
                items: vec![text.to_string()],
            });
        }
        return;
    }
    *ordered = None;
    blocks.push(Block::Paragraph(text.to_string()));
}

fn style_heading(style: &str) -> Option<u8> {
    let lower = style.to_ascii_lowercase();
    if lower == "title" {
        return Some(1);
    }
    let rest = lower.strip_prefix("heading")?.trim();
    rest.parse::<u8>().ok().filter(|n| (1..=6).contains(n))
}

fn attr_value(event: &quick_xml::events::BytesStart<'_>, name: &[u8]) -> Option<String> {
    for attr in event.attributes().flatten() {
        if attr.key.as_ref() == name {
            return attr.unescape_value().ok().map(|v| v.into_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_ke_markdown_mempertahankan_struktur_dasar() {
        let md = html_to_markdown(
            "<h1>Judul</h1><p>Halo <b>dunia</b> dan <a href=\"https://contoh.id\">tautan</a>.</p>\
             <ul><li>satu</li><li>dua</li></ul><pre><code>let x = 1;</code></pre>",
        );
        assert!(md.contains("# Judul"), "md:\n{md}");
        assert!(md.contains("**dunia**"), "md:\n{md}");
        assert!(md.contains("[tautan](https://contoh.id)"), "md:\n{md}");
        assert!(md.contains("- satu"), "md:\n{md}");
        assert!(md.contains("let x = 1;"), "md:\n{md}");
    }

    #[test]
    fn markdown_diurai_menjadi_blok() {
        let md = "# Judul\n\nParagraf **tebal** dan `kode`.\n\n- satu\n- dua\n\n1. awal\n2. akhir\n\n> kutipan\n\n```\nbaris kode\n```\n";
        let blocks = parse_markdown(md);
        assert_eq!(blocks[0], Block::Heading(1, "Judul".into()));
        assert!(
            matches!(&blocks[1], Block::Paragraph(t) if t.contains("tebal") && !t.contains("**")),
            "blok: {blocks:?}"
        );
        assert!(
            blocks.iter().any(|b| matches!(b, Block::List { ordered: false, items } if items == &["satu", "dua"])),
            "blok: {blocks:?}"
        );
        assert!(
            blocks.iter().any(|b| matches!(b, Block::List { ordered: true, items } if items == &["awal", "akhir"])),
            "blok: {blocks:?}"
        );
        assert!(blocks.iter().any(|b| matches!(b, Block::Quote(t) if t == "kutipan")), "blok: {blocks:?}");
        assert!(blocks.iter().any(|b| matches!(b, Block::Code(t) if t.contains("baris kode"))), "blok: {blocks:?}");
    }

    #[test]
    fn docx_ditulis_lalu_dibaca_kembali() {
        let blocks = vec![
            Block::Heading(1, "Judul Laporan".into()),
            Block::Paragraph("Isi paragraf pertama.".into()),
            Block::Heading(2, "Rincian".into()),
            Block::Paragraph("Isi paragraf kedua.".into()),
        ];
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("laporan.docx");
        write_docx(&blocks, &path).expect("tulis DOCX");

        let back = read_docx(&path).expect("baca DOCX");
        assert!(back.contains(&Block::Heading(1, "Judul Laporan".into())), "hasil: {back:?}");
        assert!(back.contains(&Block::Heading(2, "Rincian".into())), "hasil: {back:?}");
        assert!(back.contains(&Block::Paragraph("Isi paragraf pertama.".into())), "hasil: {back:?}");
        assert!(back.contains(&Block::Paragraph("Isi paragraf kedua.".into())), "hasil: {back:?}");
    }

    #[test]
    fn konversi_markdown_ke_docx_utuh() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("masukan.md");
        let output = dir.path().join("keluaran.docx");
        std::fs::write(&input, "# Halo\n\nDunia **tebal**.\n").unwrap();

        let cancel: CancelToken = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        convert(&input, &output, Format::Md, Format::Docx, &cancel, &|_| {}).expect("konversi");
        let blocks = read_docx(&output).expect("baca hasil");
        assert!(blocks.iter().any(|b| matches!(b, Block::Heading(1, t) if t == "Halo")), "blok: {blocks:?}");
    }

    #[test]
    fn konversi_docx_ke_html_dan_txt() {
        let dir = tempfile::tempdir().unwrap();
        let docx = dir.path().join("sumber.docx");
        write_docx(
            &[
                Block::Heading(1, "Judul".into()),
                Block::Paragraph("Halo dunia.".into()),
            ],
            &docx,
        )
        .unwrap();

        let cancel: CancelToken = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let html_path = dir.path().join("hasil.html");
        convert(&docx, &html_path, Format::Docx, Format::Html, &cancel, &|_| {}).unwrap();
        let html = std::fs::read_to_string(&html_path).unwrap();
        assert!(html.contains("<h1>Judul</h1>"), "html: {html}");
        assert!(html.contains("<p>Halo dunia.</p>"), "html: {html}");

        let txt_path = dir.path().join("hasil.txt");
        convert(&docx, &txt_path, Format::Docx, Format::Txt, &cancel, &|_| {}).unwrap();
        let txt = std::fs::read_to_string(&txt_path).unwrap();
        assert!(txt.contains("Judul"));
        assert!(txt.contains("Halo dunia."));
        assert!(!txt.contains('#'));
    }
}
