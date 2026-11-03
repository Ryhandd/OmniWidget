//! Persistensi riwayat clipboard di SQLite (tabel `clip_items`, skema di `core/storage.rs`).
//! Hanya dipakai saat pengguna menyalakan "Simpan riwayat terenkripsi". Setiap item di-JSON-kan lalu
//! dienkripsi utuh (teks, pratinjau, hash, sematan, waktu): di disk hanya ada urutan (`seq`) dan blob.

use rusqlite::params;

use super::{crypto::Sealer, ring::Item};
use crate::core::storage::Storage;

const AAD: &[u8] = b"omniwidget.clip.item.v1";

fn e2s(e: rusqlite::Error) -> String {
    e.to_string()
}

/// Ganti seluruh isi tabel dengan `items` (terbaru dulu) dalam satu transaksi.
/// Enkripsi dilakukan SEBELUM kunci database diambil agar tidak menahan koneksi.
pub fn save_all(st: &Storage, sealer: &Sealer, items_newest_first: &[Item]) -> Result<(), String> {
    let mut blobs = Vec::with_capacity(items_newest_first.len());
    // Tulis lama -> baru supaya `seq` menaik mengikuti waktu.
    for it in items_newest_first.iter().rev() {
        let json = serde_json::to_vec(it).map_err(|e| e.to_string())?;
        blobs.push(sealer.seal(AAD, &json)?);
    }
    st.with_conn(|c| {
        let tx = c.unchecked_transaction().map_err(e2s)?;
        tx.execute("DELETE FROM clip_items", []).map_err(e2s)?;
        {
            let mut stmt = tx.prepare("INSERT INTO clip_items(blob) VALUES(?1)").map_err(e2s)?;
            for b in &blobs {
                stmt.execute(params![b]).map_err(e2s)?;
            }
        }
        tx.commit().map_err(e2s)
    })
}

/// Baca semua item (terbaru dulu). Blob yang tak bisa dibuka (kunci berbeda / rusak) dilewati.
pub fn load_all(st: &Storage, sealer: &Sealer) -> Result<Vec<Item>, String> {
    let blobs: Vec<Vec<u8>> = st.with_conn(|c| {
        let mut stmt = c.prepare("SELECT blob FROM clip_items ORDER BY seq DESC").map_err(e2s)?;
        let rows = stmt.query_map([], |r| r.get::<_, Vec<u8>>(0)).map_err(e2s)?;
        let v = rows.collect::<Result<Vec<_>, _>>().map_err(e2s)?;
        Ok(v)
    })?;
    Ok(blobs
        .iter()
        .filter_map(|b| sealer.open(AAD, b).ok())
        .filter_map(|plain| serde_json::from_slice::<Item>(&plain).ok())
        .collect())
}

pub fn wipe(st: &Storage) -> Result<(), String> {
    st.with_conn(|c| c.execute("DELETE FROM clip_items", []).map(|_| ()).map_err(e2s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::clipboard::ring::{hash_text, Kind};

    fn item(s: &str, pinned: bool) -> Item {
        Item {
            hash: hash_text(s),
            kind: Kind::Text,
            preview: s.into(),
            text: Some(s.into()),
            thumb: Some(vec![1, 2, 3]), // tidak boleh ikut tersimpan
            truncated: false,
            ts: 1_700_000_000_000,
            pinned,
            width: None,
            height: None,
            bytes: None,
        }
    }

    #[test]
    fn bolak_balik_menjaga_urutan_sematan_dan_id_u64_besar() {
        let st = Storage::open_in_memory().unwrap();
        let s = Sealer::random().unwrap();
        assert!(load_all(&st, &s).unwrap().is_empty());
        let items = vec![item("baru", false), item("lama", true)];
        save_all(&st, &s, &items).unwrap();
        let back = load_all(&st, &s).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].text.as_deref(), Some("baru"));
        assert!(back[1].pinned);
        assert_eq!(back[0].hash, items[0].hash, "hash u64 utuh");
        assert!(back.iter().all(|i| i.thumb.is_none()), "thumbnail hanya cache RAM");
    }

    #[test]
    fn simpan_ulang_mengganti_isi_dan_tidak_menumpuk() {
        let st = Storage::open_in_memory().unwrap();
        let s = Sealer::random().unwrap();
        save_all(&st, &s, &[item("a", false), item("b", false)]).unwrap();
        save_all(&st, &s, &[item("c", false)]).unwrap();
        let back = load_all(&st, &s).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].text.as_deref(), Some("c"));
        wipe(&st).unwrap();
        assert!(load_all(&st, &s).unwrap().is_empty());
    }

    #[test]
    fn isi_di_disk_terenkripsi_dan_kunci_salah_tidak_membuka() {
        let st = Storage::open_in_memory().unwrap();
        let s = Sealer::random().unwrap();
        save_all(&st, &s, &[item("kata-sandi-rahasia-123", false)]).unwrap();
        let raw: Vec<u8> = st
            .with_conn(|c| c.query_row("SELECT blob FROM clip_items", [], |r| r.get(0)).map_err(e2s))
            .unwrap();
        assert!(!raw.windows(10).any(|w| w == b"kata-sandi"), "plaintext bocor ke DB");
        assert!(load_all(&st, &Sealer::random().unwrap()).unwrap().is_empty());
    }
}
