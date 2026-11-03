//! Enkripsi riwayat clipboard (TDD 4.5): ChaCha20-Poly1305, kunci 256-bit di keychain OS.
//!
//! Format blob: `[versi:1][nonce:12][ciphertext+tag]`. Nonce acak per blob (96-bit acak aman untuk
//! puluhan blob per sesi). `aad` mengikat blob ke konteksnya (riwayat vs berkas gambar + hash-nya),
//! sehingga blob tidak bisa ditukar antar konteks tanpa terdeteksi.
//!
//! Kunci TIDAK pernah jatuh ke SQLite/berkas: bila keychain tak tersedia, persistensi ditolak
//! (bukan diturunkan ke penyimpanan tak aman).

use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    ChaCha20Poly1305, Key, Nonce,
};

const VERSION: u8 = 1;
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;

pub fn random_key() -> Result<[u8; 32], String> {
    let mut k = [0u8; 32];
    getrandom::getrandom(&mut k).map_err(|e| format!("Pembangkit acak OS gagal: {e}"))?;
    Ok(k)
}

pub struct Sealer {
    cipher: ChaCha20Poly1305,
}

impl Sealer {
    pub fn new(key: &[u8; 32]) -> Self {
        Self { cipher: ChaCha20Poly1305::new(Key::from_slice(key)) }
    }

    /// Kunci acak yang hidup hanya di memori proses: berkas yang tersisa setelah crash tak bisa dibaca siapa pun.
    pub fn random() -> Result<Self, String> {
        Ok(Self::new(&random_key()?))
    }

    pub fn seal(&self, aad: &[u8], plain: &[u8]) -> Result<Vec<u8>, String> {
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::getrandom(&mut nonce).map_err(|e| format!("Pembangkit acak OS gagal: {e}"))?;
        let ct = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), Payload { msg: plain, aad })
            .map_err(|_| "Gagal mengenkripsi".to_string())?;
        let mut out = Vec::with_capacity(1 + NONCE_LEN + ct.len());
        out.push(VERSION);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        Ok(out)
    }

    pub fn open(&self, aad: &[u8], blob: &[u8]) -> Result<Vec<u8>, String> {
        if blob.len() < 1 + NONCE_LEN + TAG_LEN || blob[0] != VERSION {
            return Err("Data terenkripsi rusak atau versinya tidak dikenal".into());
        }
        let (nonce, ct) = blob[1..].split_at(NONCE_LEN);
        self.cipher
            .decrypt(Nonce::from_slice(nonce), Payload { msg: ct, aad })
            .map_err(|_| "Gagal mendekripsi (kunci berbeda atau data diubah)".to_string())
    }
}

/// Penyimpanan kunci. Trait agar logika persistensi teruji tanpa keychain sungguhan.
pub trait KeyStore: Send + Sync {
    /// Ambil kunci; buat & simpan bila belum ada (atau isinya rusak).
    fn load_or_create(&self) -> Result<[u8; 32], String>;
    fn delete(&self) -> Result<(), String>;
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
mod os {
    use super::{random_key, KeyStore};

    const SERVICE: &str = "com.omniwidget.app";
    const USER: &str = "clipboard-history-key";

    pub struct OsKeyStore;

    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, USER).map_err(|e| format!("Keychain OS tidak tersedia: {e}"))
    }

    fn decode(hexed: &str) -> Option<[u8; 32]> {
        hex::decode(hexed.trim()).ok()?.try_into().ok()
    }

    fn create(entry: &keyring::Entry) -> Result<[u8; 32], String> {
        let key = random_key()?;
        entry
            .set_password(&hex::encode(key))
            .map_err(|e| format!("Keychain OS menolak menyimpan kunci: {e}"))?;
        Ok(key)
    }

    impl KeyStore for OsKeyStore {
        fn load_or_create(&self) -> Result<[u8; 32], String> {
            let entry = entry()?;
            match entry.get_password() {
                // Isi rusak: data lama memang sudah tak terbaca, jadi buat kunci baru.
                Ok(hexed) => match decode(&hexed) {
                    Some(k) => Ok(k),
                    None => create(&entry),
                },
                Err(keyring::Error::NoEntry) => create(&entry),
                Err(e) => Err(format!("Keychain OS tidak dapat dibaca: {e}")),
            }
        }

        fn delete(&self) -> Result<(), String> {
            match entry()?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(format!("Gagal menghapus kunci dari keychain: {e}")),
            }
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub use os::OsKeyStore;

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub struct OsKeyStore;

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
impl KeyStore for OsKeyStore {
    fn load_or_create(&self) -> Result<[u8; 32], String> {
        Err("Keychain OS tidak didukung di platform ini".into())
    }
    fn delete(&self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_bolak_balik_dan_nonce_unik() {
        let s = Sealer::random().unwrap();
        let a = s.seal(b"ctx", b"rahasia").unwrap();
        let b = s.seal(b"ctx", b"rahasia").unwrap();
        assert_ne!(a, b, "nonce acak: ciphertext tidak boleh sama");
        assert_eq!(s.open(b"ctx", &a).unwrap(), b"rahasia");
        assert!(!a.windows(7).any(|w| w == b"rahasia"), "plaintext tidak boleh terlihat");
        assert_eq!(s.open(b"ctx", &s.seal(b"ctx", b"").unwrap()).unwrap(), b"");
    }

    #[test]
    fn menolak_kunci_salah_aad_salah_dan_data_diubah() {
        let s = Sealer::random().unwrap();
        let other = Sealer::random().unwrap();
        let blob = s.seal(b"ctx", b"isi").unwrap();
        assert!(other.open(b"ctx", &blob).is_err());
        assert!(s.open(b"lain", &blob).is_err());
        let mut bad = blob.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert!(s.open(b"ctx", &bad).is_err());
        let mut bad = blob.clone();
        bad[0] = 9;
        assert!(s.open(b"ctx", &bad).is_err());
        assert!(s.open(b"ctx", &blob[..10]).is_err());
        assert!(s.open(b"ctx", &[]).is_err());
    }

    #[test]
    fn kunci_tetap_sama_menghasilkan_sealer_yang_saling_kompatibel() {
        let key = random_key().unwrap();
        let blob = Sealer::new(&key).seal(b"a", b"x").unwrap();
        assert_eq!(Sealer::new(&key).open(b"a", &blob).unwrap(), b"x");
    }
}
