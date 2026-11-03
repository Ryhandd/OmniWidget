//! PKCE (RFC 7636) untuk Spotify Authorization Code Flow: tanpa client secret.
//! Semua fungsi di sini murni (kecuali pembangkit acak) sehingga teruji tanpa jaringan.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::Url;
use sha2::{Digest, Sha256};

use super::error::SpotifyError;

pub const AUTHORIZE_URL: &str = "https://accounts.spotify.com/authorize";
/// Hanya izin yang dibutuhkan: baca status pemutaran + ubah pemutaran.
pub const SCOPES: &str = "user-read-playback-state user-modify-playback-state";

/// Spotify (sejak 2025) menolak `localhost` dan http non-loopback. Literal IP loopback + port tetap
/// harus didaftarkan PERSIS di dashboard: `http://127.0.0.1:43821/callback`.
pub const REDIRECT_PORT: u16 = 43821;
pub const REDIRECT_URI: &str = "http://127.0.0.1:43821/callback";
pub const CALLBACK_PATH: &str = "/callback";

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

fn random_b64(bytes: usize) -> Result<String, SpotifyError> {
    let mut buf = vec![0u8; bytes];
    getrandom::getrandom(&mut buf).map_err(|e| SpotifyError::Api(format!("Pembangkit acak sistem gagal: {e}")))?;
    Ok(URL_SAFE_NO_PAD.encode(buf))
}

pub fn challenge_of(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

impl Pkce {
    /// Verifier 64 byte acak -> 86 karakter (batas RFC: 43..=128). State 16 byte untuk anti-CSRF.
    pub fn generate() -> Result<Self, SpotifyError> {
        let verifier = random_b64(64)?;
        let challenge = challenge_of(&verifier);
        Ok(Self { verifier, challenge, state: random_b64(16)? })
    }
}

pub fn authorize_url(client_id: &str, challenge: &str, state: &str) -> String {
    let mut u = Url::parse(AUTHORIZE_URL).expect("konstanta URL valid");
    u.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", challenge)
        .append_pair("state", state)
        .append_pair("scope", SCOPES);
    u.into()
}

/// Perbandingan waktu-tetap untuk `state` (tidak berhenti di byte pertama yang berbeda).
pub fn state_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Client ID Spotify: 32 karakter heksadesimal huruf kecil.
pub fn is_client_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_sesuai_vektor_rfc_7636_lampiran_b() {
        assert_eq!(
            challenge_of("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn verifier_dan_state_acak_dengan_panjang_valid() {
        let a = Pkce::generate().unwrap();
        let b = Pkce::generate().unwrap();
        assert!((43..=128).contains(&a.verifier.len()));
        assert!(a.verifier.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'));
        assert_ne!(a.verifier, b.verifier);
        assert_ne!(a.state, b.state);
        assert_eq!(a.challenge, challenge_of(&a.verifier));
    }

    #[test]
    fn url_otorisasi_memuat_semua_parameter_dan_di_encode() {
        let u = Url::parse(&authorize_url("abc123", "CHAL", "ST/ATE")).unwrap();
        assert_eq!(u.host_str(), Some("accounts.spotify.com"));
        let q: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
        assert_eq!(q["client_id"], "abc123");
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["redirect_uri"], REDIRECT_URI);
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["code_challenge"], "CHAL");
        assert_eq!(q["state"], "ST/ATE");
        assert_eq!(q["scope"], SCOPES);
        assert!(!q.contains_key("client_secret"));
    }

    #[test]
    fn redirect_uri_konsisten_dengan_port() {
        assert_eq!(REDIRECT_URI, format!("http://127.0.0.1:{REDIRECT_PORT}{CALLBACK_PATH}"));
    }

    #[test]
    fn state_eq_dan_client_id() {
        assert!(state_eq("abc", "abc"));
        assert!(!state_eq("abc", "abd"));
        assert!(!state_eq("abc", "abcd"));
        assert!(is_client_id("0123456789abcdef0123456789abcdef"));
        assert!(!is_client_id("0123456789ABCDEF0123456789abcdef"));
        assert!(!is_client_id("short"));
        assert!(!is_client_id("0123456789abcdef0123456789abcdeg"));
    }
}
