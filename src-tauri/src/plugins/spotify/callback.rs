//! Server callback loopback sekali pakai untuk menerima redirect OAuth dari browser.
//! Hanya mendengarkan 127.0.0.1, hanya menerima `GET /callback` dengan `state` yang cocok, dan tidak
//! pernah memantulkan isi query ke halaman (halaman statis): tidak ada celah XSS/refleksi.

use std::time::Duration;

use reqwest::Url;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, Notify},
    time::timeout,
};

use super::{
    error::SpotifyError,
    pkce::{state_eq, CALLBACK_PATH},
};

const MAX_HEAD_BYTES: usize = 8 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Hasil penguraian baris permintaan HTTP pertama.
#[derive(Debug, PartialEq, Eq)]
pub struct Hit {
    pub path: String,
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// `GET /callback?code=..&state=.. HTTP/1.1` -> `Hit`. Metode selain GET atau target tak wajar -> None.
pub fn parse_request_line(head: &str) -> Option<Hit> {
    let line = head.lines().next()?;
    let mut parts = line.split(' ');
    let (method, target) = (parts.next()?, parts.next()?);
    if method != "GET" || !target.starts_with('/') || target.starts_with("//") {
        return None;
    }
    let url = Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
    let mut hit = Hit { path: url.path().to_string(), code: None, state: None, error: None };
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "code" => hit.code = Some(v.into_owned()),
            "state" => hit.state = Some(v.into_owned()),
            "error" => hit.error = Some(v.into_owned()),
            _ => {}
        }
    }
    Some(hit)
}

/// Keputusan untuk satu permintaan: balasan HTTP + (opsional) hasil akhir login.
pub struct Verdict {
    pub status: &'static str,
    pub body: &'static str,
    pub done: Option<Result<String, SpotifyError>>,
}

pub fn decide(hit: Option<Hit>, expected_state: &str) -> Verdict {
    let Some(hit) = hit else {
        return Verdict { status: "400 Bad Request", body: PAGE_BAD, done: None };
    };
    if hit.path != CALLBACK_PATH {
        return Verdict { status: "404 Not Found", body: PAGE_NOT_FOUND, done: None };
    }
    // State diperiksa SEBELUM menghormati `error`/`code`: permintaan tanpa state valid diabaikan total
    // (tidak mengakhiri login yang sedang berjalan).
    match hit.state.as_deref() {
        Some(s) if state_eq(s, expected_state) => {}
        _ => return Verdict { status: "400 Bad Request", body: PAGE_BAD, done: None },
    }
    if let Some(err) = hit.error {
        let short: String = err.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').take(40).collect();
        return Verdict { status: "200 OK", body: PAGE_DENIED, done: Some(Err(SpotifyError::Denied(short))) };
    }
    match hit.code {
        Some(code) if !code.is_empty() && code.len() <= 2048 => {
            Verdict { status: "200 OK", body: PAGE_OK, done: Some(Ok(code)) }
        }
        _ => Verdict { status: "400 Bad Request", body: PAGE_BAD, done: None },
    }
}

async fn read_head(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let n = timeout(READ_TIMEOUT, stream.read(&mut chunk)).await.ok()?.ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() >= MAX_HEAD_BYTES {
            break;
        }
    }
    (!buf.is_empty()).then(|| String::from_utf8_lossy(&buf).into_owned())
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let msg = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nReferrer-Policy: no-referrer\r\n\
         Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(msg.as_bytes()).await;
    let _ = stream.shutdown().await;
}

async fn serve_one(mut stream: TcpStream, expected_state: String, tx: mpsc::UnboundedSender<Result<String, SpotifyError>>) {
    // Koneksi spekulatif browser (membuka soket tanpa mengirim apa pun) cukup habis waktu di task-nya sendiri.
    let Some(head) = read_head(&mut stream).await else { return };
    let verdict = decide(parse_request_line(&head), &expected_state);
    respond(&mut stream, verdict.status, verdict.body).await;
    if let Some(done) = verdict.done {
        let _ = tx.send(done);
    }
}

/// Tunggu callback sampai `deadline`. Tiap koneksi ditangani di task sendiri agar soket menggantung
/// tidak menunda callback asli. Mengembalikan authorization code.
pub async fn wait_for_code(
    listener: TcpListener,
    expected_state: &str,
    cancel: &Notify,
    deadline: Duration,
) -> Result<String, SpotifyError> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let sleep = tokio::time::sleep(deadline);
    tokio::pin!(sleep);
    loop {
        tokio::select! {
            _ = cancel.notified() => return Err(SpotifyError::Cancelled),
            _ = &mut sleep => return Err(SpotifyError::LoginTimeout(deadline.as_secs())),
            done = rx.recv() => {
                // `tx` masih hidup di fungsi ini, jadi `None` tidak mungkin; tetap ditangani.
                return done.unwrap_or(Err(SpotifyError::Cancelled));
            }
            accepted = listener.accept() => {
                if let Ok((stream, peer)) = accepted {
                    if peer.ip().is_loopback() {
                        tokio::spawn(serve_one(stream, expected_state.to_string(), tx.clone()));
                    }
                }
            }
        }
    }
}


macro_rules! page {
    ($title:literal, $text:literal) => {
        concat!(
            "<!doctype html><html lang=\"id\"><meta charset=\"utf-8\"><title>OmniWidget</title><style>",
            "body{font:16px system-ui,sans-serif;display:grid;place-items:center;height:100vh;margin:0;background:#12131a;color:#e8e9f0}",
            "main{text-align:center;padding:24px}h1{font-size:20px;margin:0 0 8px}p{margin:0;opacity:.7}",
            "</style><main><h1>", $title, "</h1><p>", $text, "</p></main></html>"
        )
    };
}

const PAGE_OK: &str = page!("Spotify terhubung", "Anda bisa menutup tab ini dan kembali ke OmniWidget.");
const PAGE_DENIED: &str = page!("Izin tidak diberikan", "Kembali ke OmniWidget untuk mencoba lagi.");
const PAGE_BAD: &str = page!("Permintaan tidak valid", "Tab ini bisa ditutup.");
const PAGE_NOT_FOUND: &str = page!("Tidak ditemukan", "Tab ini bisa ditutup.");

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(line: &str) -> Option<Hit> {
        parse_request_line(&format!("{line}\r\nHost: 127.0.0.1\r\n\r\n"))
    }

    #[test]
    fn parse_baris_permintaan() {
        let h = hit("GET /callback?code=AQD%2Fx&state=s1 HTTP/1.1").unwrap();
        assert_eq!(h.path, "/callback");
        assert_eq!(h.code.as_deref(), Some("AQD/x"));
        assert_eq!(h.state.as_deref(), Some("s1"));
        let e = hit("GET /callback?error=access_denied&state=s1 HTTP/1.1").unwrap();
        assert_eq!(e.error.as_deref(), Some("access_denied"));
        assert!(hit("POST /callback HTTP/1.1").is_none());
        assert!(hit("GET //evil.com/callback HTTP/1.1").is_none());
        assert!(hit("GET callback HTTP/1.1").is_none());
        assert!(parse_request_line("").is_none());
    }

    #[test]
    fn keputusan_menerima_hanya_state_yang_cocok() {
        let ok = decide(hit("GET /callback?code=C&state=S HTTP/1.1"), "S");
        assert_eq!(ok.done.unwrap().unwrap(), "C");
        assert_eq!(ok.status, "200 OK");

        // state salah / hilang -> diabaikan, login tetap menunggu
        assert!(decide(hit("GET /callback?code=C&state=X HTTP/1.1"), "S").done.is_none());
        assert!(decide(hit("GET /callback?code=C HTTP/1.1"), "S").done.is_none());
        // error palsu tanpa state valid tidak boleh menggagalkan login
        assert!(decide(hit("GET /callback?error=access_denied HTTP/1.1"), "S").done.is_none());
        // jalur lain -> 404
        let nf = decide(hit("GET /favicon.ico HTTP/1.1"), "S");
        assert_eq!(nf.status, "404 Not Found");
        assert!(nf.done.is_none());
        // kode kosong
        assert!(decide(hit("GET /callback?code=&state=S HTTP/1.1"), "S").done.is_none());
    }

    #[test]
    fn penolakan_izin_dengan_state_valid_menjadi_error_denied() {
        let v = decide(hit("GET /callback?error=access_denied&state=S HTTP/1.1"), "S");
        match v.done.unwrap() {
            Err(SpotifyError::Denied(s)) => assert_eq!(s, "access_denied"),
            other => panic!("tak terduga: {other:?}"),
        }
    }

    #[test]
    fn halaman_statis_tidak_memuat_input_pengguna() {
        for p in [PAGE_OK, PAGE_DENIED, PAGE_BAD, PAGE_NOT_FOUND] {
            assert!(p.starts_with("<!doctype html>") && !p.contains("<script"));
        }
    }

    #[tokio::test]
    async fn server_menerima_callback_nyata_dan_membatalkan() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let cancel = std::sync::Arc::new(Notify::new());

        let task = {
            let cancel = cancel.clone();
            tokio::spawn(async move { wait_for_code(listener, "S", &cancel, Duration::from_secs(10)).await })
        };
        let mut s = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(b"GET /callback?code=KODE&state=S HTTP/1.1\r\nHost: x\r\n\r\n").await.unwrap();
        let mut resp = String::new();
        s.read_to_string(&mut resp).await.unwrap();
        assert!(resp.starts_with("HTTP/1.1 200 OK"));
        assert_eq!(task.await.unwrap().unwrap(), "KODE");

        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let c2 = std::sync::Arc::new(Notify::new());
        c2.notify_one();
        let r = wait_for_code(listener, "S", &c2, Duration::from_secs(10)).await;
        assert!(matches!(r, Err(SpotifyError::Cancelled)));
    }
}
