//! Aturan privasi Clipboard Manager (TDD 4.5 "Privasi (wajib)"). Murni: watcher yang membaca OS,
//! di sini hanya keputusan "boleh direkam atau tidak" dan normalisasi teks.

/// Format penanda konten sensitif (huruf kecil). Aplikasi sumber (pengelola kata sandi, dsb.) menyetel
/// salah satunya saat menyalin rahasia; kehadirannya saja sudah berarti "jangan rekam".
const SENSITIVE_FORMATS: &[&str] = &[
    // Windows
    "excludeclipboardcontentfrommonitorprocessing",
    // macOS (konvensi nspasteboard.org)
    "org.nspasteboard.concealedtype",
    "org.nspasteboard.transienttype",
    // 1Password (macOS)
    "com.agilebits.onepassword",
    // Linux/KDE: dipakai KeePassXC, Bitwarden, dsb.
    "x-kde-passwordmanagerhint",
];

/// Windows: DWORD 0 berarti "jangan masukkan ke riwayat clipboard".
pub const WIN_CAN_INCLUDE_IN_HISTORY: &str = "CanIncludeInClipboardHistory";

pub fn has_sensitive_format(formats: &[String]) -> bool {
    formats.iter().any(|f| SENSITIVE_FORMATS.contains(&f.to_ascii_lowercase().as_str()))
}

/// DWORD little-endian bernilai 0 = diblokir.
pub fn history_blocked_by_dword(buf: &[u8]) -> bool {
    buf.len() >= 4 && u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) == 0
}

/// Keputusan akhir dari daftar format clipboard. `read` membaca buffer mentah satu format.
/// Gagal-tertutup: bila `CanIncludeInClipboardHistory` ada tetapi tidak terbaca, JANGAN rekam.
pub fn blocks_recording(formats: &[String], read: impl Fn(&str) -> Option<Vec<u8>>) -> bool {
    if has_sensitive_format(formats) {
        return true;
    }
    if formats.iter().any(|f| f.eq_ignore_ascii_case(WIN_CAN_INCLUDE_IN_HISTORY)) {
        return read(WIN_CAN_INCLUDE_IN_HISTORY).map_or(true, |b| history_blocked_by_dword(&b));
    }
    false
}

/// Sepotong nama proses pengelola kata sandi (huruf kecil). Heuristik "contains", bukan daftar lengkap.
const DENY_APPS: &[&str] = &[
    "keepass", "1password", "bitwarden", "lastpass", "dashlane", "enpass", "nordpass", "roboform", "protonpass",
    "proton pass",
];

pub fn is_denied_app(process_name: &str) -> bool {
    let n = process_name.to_ascii_lowercase();
    DENY_APPS.iter().any(|d| n.contains(d))
}

/// Potong di batas karakter (tidak pernah di tengah UTF-8). Mengembalikan (teks, dipotong?).
pub fn truncate_text(s: &str, max_bytes: usize) -> (String, bool) {
    if s.len() <= max_bytes {
        return (s.to_string(), false);
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    (s[..end].to_string(), true)
}

pub const PREVIEW_CHARS: usize = 200;

/// Pratinjau satu baris: spasi/baris baru beruntun dipadatkan jadi satu spasi, maksimal 200 karakter.
pub fn make_preview(s: &str) -> String {
    let mut out = String::new();
    let mut n = 0usize;
    let mut pending_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            n += 1;
            pending_space = false;
            if n >= PREVIEW_CHARS {
                break;
            }
        }
        out.push(c);
        n += 1;
        if n >= PREVIEW_CHARS {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmts(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn format_sensitif_tak_peka_huruf_besar_kecil() {
        assert!(has_sensitive_format(&fmts(&["text/plain", "ExcludeClipboardContentFromMonitorProcessing"])));
        assert!(has_sensitive_format(&fmts(&["org.nspasteboard.ConcealedType"])));
        assert!(has_sensitive_format(&fmts(&["org.nspasteboard.TransientType", "public.utf8-plain-text"])));
        assert!(has_sensitive_format(&fmts(&["x-kde-passwordManagerHint"])));
        assert!(!has_sensitive_format(&fmts(&["text/plain", "public.png", "CF_UNICODETEXT"])));
        assert!(!has_sensitive_format(&[]));
    }

    #[test]
    fn can_include_in_history_dword_dan_gagal_tertutup() {
        let f = fmts(&["CanIncludeInClipboardHistory", "CF_UNICODETEXT"]);
        assert!(blocks_recording(&f, |_| Some(vec![0, 0, 0, 0])), "DWORD 0 = blokir");
        assert!(!blocks_recording(&f, |_| Some(vec![1, 0, 0, 0])), "DWORD 1 = boleh");
        assert!(blocks_recording(&f, |_| None), "ada tapi tak terbaca = blokir");
        assert!(!blocks_recording(&f, |_| Some(vec![0])), "buffer pendek bukan DWORD valid: tidak dianggap 0");
        // Tanpa format itu, buffer tidak pernah dibaca.
        assert!(!blocks_recording(&fmts(&["CF_UNICODETEXT"]), |_| panic!("tidak boleh dibaca")));
    }

    #[test]
    fn denylist_aplikasi() {
        assert!(is_denied_app("KeePassXC.exe"));
        assert!(is_denied_app("1Password"));
        assert!(is_denied_app("Bitwarden"));
        assert!(!is_denied_app("Code.exe"));
        assert!(!is_denied_app("gatekeeper")); // tidak ada kata kunci "keeper" yang terlalu umum
    }

    #[test]
    fn potong_di_batas_karakter() {
        assert_eq!(truncate_text("halo", 10), ("halo".to_string(), false));
        let (t, cut) = truncate_text("aé", 2); // 'é' = 2 byte, mulai di indeks 1: batas 2 jatuh di tengah
        assert!(cut);
        assert_eq!(t, "a");
        let big = "x".repeat(70_000);
        let (t, cut) = truncate_text(&big, 64 * 1024);
        assert!(cut);
        assert_eq!(t.len(), 64 * 1024);
    }

    #[test]
    fn pratinjau_memadatkan_spasi_dan_membatasi_panjang() {
        assert_eq!(make_preview("  baris satu\n\n\tbaris   dua  "), "baris satu baris dua");
        assert_eq!(make_preview("   \n "), "");
        assert_eq!(make_preview(&"a".repeat(500)).chars().count(), PREVIEW_CHARS);
        assert!(make_preview(&"ab ".repeat(500)).chars().count() <= PREVIEW_CHARS);
        assert_eq!(make_preview("日本語 テキスト"), "日本語 テキスト");
    }
}
