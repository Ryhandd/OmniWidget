//! Ring buffer riwayat clipboard (TDD 4.5). Murni: tanpa Tauri, tanpa OS, tanpa I/O, jadi teruji penuh.
//!
//! Urutan `items`: depan = terbaru. Kapasitas hanya menghitung item TIDAK disematkan (20);
//! item tersemat tidak pernah tergusur, tetapi dibatasi `MAX_PINNED` agar RAM tetap terbatas.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

pub const CAPACITY: usize = 20;
/// Tambahan di luar TDD: tanpa batas, 20 teks x 64 KB + sematan tak terbatas melanggar anggaran RAM 5.5.
pub const MAX_PINNED: usize = 20;
/// Teks dipotong di batas ini (TDD 4.5).
pub const MAX_TEXT_BYTES: usize = 64 * 1024;
/// Total thumbnail yang boleh tinggal di RAM (TDD 5.5). Yang lebih lama dilepas; dimuat ulang dari disk.
pub const THUMB_RAM_BUDGET: usize = 400 * 1024;
/// Total PNG penuh di disk (TDD 4.5 / 5.5).
pub const IMAGE_DISK_BUDGET: u64 = 50 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Text,
    Image,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    /// Hash konten yang disimpan (bukan konten asli sebelum dipotong). Menjadi `id` di UI (string desimal).
    pub hash: u64,
    pub kind: Kind,
    pub preview: String,
    pub text: Option<String>,
    /// PNG thumbnail <= 20 KB. Hanya cache RAM: tidak ikut dipersistenkan, bisa dibangun ulang dari berkas.
    #[serde(skip)]
    pub thumb: Option<Vec<u8>>,
    pub truncated: bool,
    /// Epoch milidetik UTC.
    pub ts: i64,
    pub pinned: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Ukuran PNG di disk (gambar saja).
    pub bytes: Option<u64>,
}

// ---------------------------------------------------------------- Hash

/// FNV-1a 64-bit. Dipilih karena STABIL lintas versi Rust & restart (id harus sama setelah dipulihkan
/// dari penyimpanan); `DefaultHasher` tidak menjamin itu. Bukan kriptografis: hanya untuk dedup.
pub struct Fnv(u64);

impl Fnv {
    pub fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    pub fn write(&mut self, data: &[u8]) {
        for &b in data {
            self.byte(b);
        }
    }
    fn byte(&mut self, b: u8) {
        self.0 ^= u64::from(b);
        self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
    }
    pub fn finish(&self) -> u64 {
        self.0
    }
}

/// CR diabaikan: "a\r\nb" (Windows) dan "a\nb" dianggap sama, juga bila teks ditulis ulang oleh OS.
pub fn hash_text(s: &str) -> u64 {
    let mut h = Fnv::new();
    h.byte(b't');
    for b in s.bytes().filter(|&b| b != b'\r') {
        h.byte(b);
    }
    h.finish()
}

pub fn hash_image(png: &[u8]) -> u64 {
    let mut h = Fnv::new();
    h.byte(b'i');
    h.write(png);
    h.finish()
}

// ---------------------------------------------------------------- Ring

#[derive(Debug, PartialEq, Eq)]
pub enum PinError {
    NotFound,
    TooMany,
}

pub struct Ring {
    items: VecDeque<Item>,
    cap: usize,
}

impl Ring {
    pub fn new(cap: usize) -> Self {
        Self { items: VecDeque::new(), cap }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Item> {
        self.items.iter()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    fn pos(&self, hash: u64) -> Option<usize> {
        self.items.iter().position(|x| x.hash == hash)
    }

    pub fn contains(&self, hash: u64) -> bool {
        self.pos(hash).is_some()
    }

    pub fn get(&self, hash: u64) -> Option<&Item> {
        self.items.iter().find(|x| x.hash == hash)
    }

    pub fn unpinned_len(&self) -> usize {
        self.items.iter().filter(|x| !x.pinned).count()
    }

    pub fn pinned_len(&self) -> usize {
        self.items.iter().filter(|x| x.pinned).count()
    }

    pub fn snapshot(&self) -> Vec<Item> {
        self.items.iter().cloned().collect()
    }

    pub fn image_hashes(&self) -> Vec<u64> {
        self.items.iter().filter(|x| x.kind == Kind::Image).map(|x| x.hash).collect()
    }

    fn image_bytes(&self) -> u64 {
        self.items.iter().filter(|x| x.kind == Kind::Image).filter_map(|x| x.bytes).sum()
    }

    fn thumb_bytes(&self) -> usize {
        self.items.iter().filter_map(|x| x.thumb.as_ref()).map(Vec::len).sum()
    }

    /// Dorong item baru ke depan. Duplikat (hash sama) dipindah ke depan dan tetap tersemat bila sebelumnya
    /// tersemat. Mengembalikan item yang TERGUSUR (pemanggil menghapus berkas gambarnya); item lama yang
    /// digantikan duplikat tidak dikembalikan karena berkasnya sama dan masih dipakai.
    pub fn push(&mut self, mut it: Item) -> Vec<Item> {
        if let Some(i) = self.pos(it.hash) {
            if let Some(old) = self.items.remove(i) {
                it.pinned |= old.pinned;
                if it.thumb.is_none() {
                    it.thumb = old.thumb;
                }
            }
        }
        self.items.push_front(it);
        self.enforce()
    }

    /// Pindahkan ke depan (tempel kembali / salin ulang). false bila tidak ada.
    pub fn touch(&mut self, hash: u64, ts: i64) -> bool {
        let Some(i) = self.pos(hash) else { return false };
        if let Some(mut it) = self.items.remove(i) {
            it.ts = ts;
            self.items.push_front(it);
        }
        true
    }

    pub fn set_pinned(&mut self, hash: u64, pinned: bool) -> Result<Vec<Item>, PinError> {
        let pinned_now = self.pinned_len();
        let it = self.items.iter_mut().find(|x| x.hash == hash).ok_or(PinError::NotFound)?;
        if it.pinned == pinned {
            return Ok(Vec::new());
        }
        if pinned && pinned_now >= MAX_PINNED {
            return Err(PinError::TooMany);
        }
        it.pinned = pinned;
        // Melepas sematan bisa membuat jumlah tidak tersemat melewati kapasitas.
        Ok(self.enforce())
    }

    pub fn remove(&mut self, hash: u64) -> Option<Item> {
        let i = self.pos(hash)?;
        self.items.remove(i)
    }

    /// Kosongkan riwayat; item tersemat dipertahankan bila `keep_pinned`. Mengembalikan yang dibuang.
    pub fn clear(&mut self, keep_pinned: bool) -> Vec<Item> {
        let mut gone = Vec::new();
        let mut kept = VecDeque::new();
        for it in std::mem::take(&mut self.items) {
            if keep_pinned && it.pinned {
                kept.push_back(it);
            } else {
                gone.push(it);
            }
        }
        self.items = kept;
        gone
    }

    /// Pulihkan dari penyimpanan (urutan terbaru dulu).
    pub fn restore(&mut self, items: Vec<Item>) {
        self.items = items.into();
        let _ = self.enforce();
    }

    pub fn set_thumb(&mut self, hash: u64, thumb: Vec<u8>) {
        if let Some(it) = self.items.iter_mut().find(|x| x.hash == hash) {
            it.thumb = Some(thumb);
        }
        self.enforce_thumb_budget();
    }

    /// Lepas semua thumbnail dari RAM (trim L2+). Mengembalikan byte yang dibebaskan.
    pub fn release_thumbs(&mut self) -> usize {
        self.items.iter_mut().filter_map(|x| x.thumb.take()).map(|t| t.len()).sum()
    }

    fn evict_oldest_unpinned(&mut self) -> Option<Item> {
        let i = self.items.iter().rposition(|x| !x.pinned)?;
        self.items.remove(i)
    }

    fn enforce(&mut self) -> Vec<Item> {
        let mut out = Vec::new();
        while self.unpinned_len() > self.cap {
            match self.evict_oldest_unpinned() {
                Some(it) => out.push(it),
                None => break,
            }
        }
        // Anggaran disk: buang gambar tidak tersemat tertua, tetapi jangan pernah item terbaru (indeks 0).
        while self.image_bytes() > IMAGE_DISK_BUDGET {
            let victim = self
                .items
                .iter()
                .enumerate()
                .rev()
                .find(|(i, x)| *i != 0 && !x.pinned && x.kind == Kind::Image)
                .map(|(i, _)| i);
            match victim.and_then(|i| self.items.remove(i)) {
                Some(it) => out.push(it),
                None => break,
            }
        }
        self.enforce_thumb_budget();
        out
    }

    fn enforce_thumb_budget(&mut self) {
        let mut total = self.thumb_bytes();
        if total <= THUMB_RAM_BUDGET {
            return;
        }
        for it in self.items.iter_mut().rev() {
            if total <= THUMB_RAM_BUDGET {
                break;
            }
            if let Some(t) = it.thumb.take() {
                total -= t.len();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(hash: u64) -> Item {
        Item {
            hash,
            kind: Kind::Text,
            preview: format!("t{hash}"),
            text: Some(format!("t{hash}")),
            thumb: None,
            truncated: false,
            ts: hash as i64,
            pinned: false,
            width: None,
            height: None,
            bytes: None,
        }
    }

    fn image(hash: u64, bytes: u64, thumb: usize) -> Item {
        Item {
            kind: Kind::Image,
            text: None,
            thumb: Some(vec![0; thumb]),
            width: Some(10),
            height: Some(10),
            bytes: Some(bytes),
            ..text(hash)
        }
    }

    fn order(r: &Ring) -> Vec<u64> {
        r.iter().map(|x| x.hash).collect()
    }

    #[test]
    fn fnv_vektor_standar_dan_domain_terpisah() {
        let mut h = Fnv::new();
        assert_eq!(h.finish(), 0xcbf2_9ce4_8422_2325); // basis offset
        h.write(b"a");
        assert_eq!(h.finish(), 0xaf63_dc4c_8601_ec8c); // vektor uji resmi FNV-1a 64
        assert_ne!(hash_text("x"), hash_image(b"x"));
        assert_eq!(hash_text("a\r\nb"), hash_text("a\nb"));
        assert_ne!(hash_text("ab"), hash_text("ba"));
    }

    #[test]
    fn duplikat_pindah_ke_depan_dan_tetap_tersemat() {
        let mut r = Ring::new(20);
        r.push(text(1));
        r.push(text(2));
        r.set_pinned(1, true).unwrap();
        let ev = r.push(text(1)); // salin ulang item 1 (item baru datang tidak tersemat)
        assert!(ev.is_empty());
        assert_eq!(order(&r), vec![1, 2]);
        assert!(r.get(1).unwrap().pinned, "sematan lama harus bertahan");
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn kapasitas_menggusur_tertua_tidak_tersemat() {
        let mut r = Ring::new(3);
        for h in 1..=3 {
            r.push(text(h));
        }
        r.set_pinned(1, true).unwrap(); // yang tertua disematkan
        let ev = r.push(text(4)); // tidak tersemat: 2,3,4 -> 3 pas
        assert!(ev.is_empty());
        let ev = r.push(text(5)); // 2,3,4,5 -> 2 tergusur, bukan 1
        assert_eq!(ev.iter().map(|x| x.hash).collect::<Vec<_>>(), vec![2]);
        assert_eq!(order(&r), vec![5, 4, 3, 1]);
        assert_eq!(r.unpinned_len(), 3);
    }

    #[test]
    fn batas_sematan_dan_pelepasan_menggusur() {
        let mut r = Ring::new(2);
        for h in 1..=2 {
            r.push(text(h));
        }
        r.set_pinned(1, true).unwrap();
        r.push(text(3));
        r.push(text(4)); // tersisa: 4,3 tidak tersemat + 1 tersemat
        assert_eq!(order(&r), vec![4, 3, 1]);
        let ev = r.set_pinned(1, false).unwrap(); // kini 3 tidak tersemat > 2: yang tertua (1) tergusur
        assert_eq!(ev.iter().map(|x| x.hash).collect::<Vec<_>>(), vec![1]);
        assert_eq!(r.set_pinned(99, true).unwrap_err(), PinError::NotFound);

        let mut r = Ring::new(100);
        for h in 0..=(MAX_PINNED as u64) {
            r.push(text(h));
        }
        for h in 0..MAX_PINNED as u64 {
            r.set_pinned(h, true).unwrap();
        }
        assert_eq!(r.set_pinned(MAX_PINNED as u64, true).unwrap_err(), PinError::TooMany);
        assert!(r.set_pinned(0, true).unwrap().is_empty(), "idempoten: sudah tersemat");
    }

    #[test]
    fn touch_clear_dan_remove() {
        let mut r = Ring::new(20);
        for h in 1..=4 {
            r.push(text(h));
        }
        r.set_pinned(2, true).unwrap();
        assert!(r.touch(1, 999));
        assert_eq!(order(&r), vec![1, 4, 3, 2]);
        assert_eq!(r.get(1).unwrap().ts, 999);
        assert!(!r.touch(42, 1));
        assert_eq!(r.remove(4).unwrap().hash, 4);
        assert!(r.remove(4).is_none());
        let gone = r.clear(true);
        assert_eq!(gone.len(), 2);
        assert_eq!(order(&r), vec![2]);
        assert_eq!(r.clear(false).len(), 1);
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn anggaran_thumbnail_ram_melepas_yang_tertua_dulu() {
        let mut r = Ring::new(20);
        // 3 x 150 KB = 450 KB > 400 KB: thumbnail item tertua dilepas, item lain utuh.
        r.push(image(1, 10, 150 * 1024));
        r.push(image(2, 10, 150 * 1024));
        r.push(image(3, 10, 150 * 1024));
        assert!(r.get(1).unwrap().thumb.is_none());
        assert!(r.get(2).unwrap().thumb.is_some());
        assert!(r.get(3).unwrap().thumb.is_some());
        let freed = r.release_thumbs();
        assert_eq!(freed, 300 * 1024);
        assert!(r.iter().all(|x| x.thumb.is_none()));
        r.set_thumb(2, vec![0; 100]);
        assert_eq!(r.get(2).unwrap().thumb.as_ref().unwrap().len(), 100);
    }

    #[test]
    fn anggaran_disk_menggusur_gambar_tertua_tapi_tidak_yang_terbaru_atau_tersemat() {
        let mb = 1024 * 1024;
        let mut r = Ring::new(20);
        r.push(image(1, 30 * mb, 0));
        r.set_pinned(1, true).unwrap();
        r.push(image(2, 15 * mb, 0));
        r.push(image(3, 15 * mb, 0)); // total 60 MB > 50 MB -> 2 (tertua tidak tersemat) tergusur
        assert_eq!(order(&r), vec![3, 1]);
        let ev = r.push(image(4, 45 * mb, 0)); // 30 + 15 + 45: 3 tergusur; 4 (terbaru) & 1 (tersemat) aman
        assert_eq!(ev.iter().map(|x| x.hash).collect::<Vec<_>>(), vec![3]);
        assert_eq!(order(&r), vec![4, 1]);
        let ev = r.push(image(5, 60 * mb, 0)); // satu gambar melebihi anggaran: tetap disimpan (terbaru)
        assert_eq!(order(&r), vec![5, 1]);
        assert_eq!(ev.len(), 1);
    }

    #[test]
    fn restore_memotong_ke_kapasitas() {
        let mut r = Ring::new(2);
        r.restore((1..=5).map(text).collect());
        assert_eq!(r.len(), 2);
    }
}
