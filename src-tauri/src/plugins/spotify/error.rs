use serde::{ser::SerializeStruct, Serialize, Serializer};

/// Error modul Spotify. Dikirim ke UI sebagai objek `{ code, message }` (bukan string polos) supaya UI
/// bisa membedakan "perlu login ulang" dari "gagal sementara" tanpa mencocokkan teks.
#[derive(Debug, thiserror::Error)]
pub enum SpotifyError {
    #[error("Client ID Spotify belum diatur")]
    NoClientId,
    #[error("Client ID tidak valid (harus 32 karakter heksadesimal)")]
    BadClientId,
    #[error("Spotify belum terhubung")]
    NotConnected,
    #[error("Sesi Spotify berakhir. Hubungkan ulang akun Anda")]
    SessionExpired,
    /// Internal: 401 dari Web API. Dipakai untuk memicu satu kali refresh + ulang; tidak sampai ke UI.
    #[error("Token ditolak Spotify")]
    Unauthorized,
    #[error("Login dibatalkan")]
    Cancelled,
    #[error("Login melewati batas waktu ({0} dtk). Coba lagi")]
    LoginTimeout(u64),
    #[error("Port {0} sedang dipakai aplikasi lain. Tutup aplikasi itu lalu coba lagi")]
    PortBusy(u16),
    #[error("Spotify menolak izin ({0})")]
    Denied(String),
    #[error("Gagal membuka browser: {0}")]
    Browser(String),
    #[error("Kontrol pemutaran membutuhkan Spotify Premium")]
    PremiumRequired,
    #[error("Tidak ada perangkat Spotify yang aktif. Buka Spotify di perangkat Anda, lalu pilih perangkat")]
    NoActiveDevice,
    #[error("Akses ditolak Spotify. Pastikan akun Anda terdaftar di User Management dashboard dan pemilik app memakai Premium")]
    Forbidden,
    #[error("Terlalu banyak permintaan. Coba lagi dalam {0} dtk")]
    RateLimited(u64),
    #[error("Permintaan tidak valid: {0}")]
    Invalid(String),
    #[error("{0}")]
    Api(String),
    #[error("{0}")]
    Network(String),
    #[error("{0}")]
    Storage(String),
}

impl SpotifyError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoClientId => "no_client_id",
            Self::BadClientId => "bad_client_id",
            Self::NotConnected => "not_connected",
            Self::SessionExpired | Self::Unauthorized => "session_expired",
            Self::Cancelled => "cancelled",
            Self::LoginTimeout(_) => "login_timeout",
            Self::PortBusy(_) => "port_busy",
            Self::Denied(_) => "denied",
            Self::Browser(_) => "browser",
            Self::PremiumRequired => "premium_required",
            Self::NoActiveDevice => "no_active_device",
            Self::Forbidden => "forbidden",
            Self::RateLimited(_) => "rate_limited",
            Self::Invalid(_) => "invalid",
            Self::Api(_) => "api",
            Self::Network(_) => "network",
            Self::Storage(_) => "storage",
        }
    }
}

impl Serialize for SpotifyError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("SpotifyError", 2)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

impl From<reqwest::Error> for SpotifyError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            Self::Network("Permintaan ke Spotify melewati batas waktu".into())
        } else if e.is_connect() {
            Self::Network("Tidak dapat terhubung ke Spotify. Periksa koneksi internet".into())
        } else {
            Self::Network("Gagal menghubungi Spotify".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialisasi_membawa_kode_dan_pesan() {
        let v = serde_json::to_value(SpotifyError::PremiumRequired).unwrap();
        assert_eq!(v["code"], "premium_required");
        assert!(v["message"].as_str().unwrap().contains("Premium"));
    }

    #[test]
    fn unauthorized_internal_terlihat_sebagai_sesi_berakhir() {
        assert_eq!(SpotifyError::Unauthorized.code(), "session_expired");
    }
}
