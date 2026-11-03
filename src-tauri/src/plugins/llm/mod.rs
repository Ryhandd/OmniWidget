//! Klien LLM tipis: satu-satunya pintu jaringan untuk fitur AI (TDD 4.10 aturan 3).
//! CSP WebView hanya mengizinkan `ipc:`, jadi UI tidak pernah menghubungi penyedia langsung.
//! API key disimpan di kv SQLite dengan prefiks `secret.` dan TIDAK pernah dikirim ke WebView:
//! `kv_*` menolak prefiks itu, dan `llm_get_config` hanya melaporkan `hasKey`.
//!
//! Dua protokol: `openai` (Chat Completions; juga dipakai OpenRouter, Groq, Gemini-compat, Ollama, dsb.)
//! dan `anthropic` (Messages API). Permintaan tidak mengirim `temperature`/`max_tokens` pada protokol
//! `openai` karena sebagian model baru menolak parameter tersebut.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, State};

use crate::core::{state::AwakeGuard, storage::Storage};

const CONFIG_KEY: &str = "llm.config";
pub const SECRET_KEY: &str = "secret.llm_api_key";

const TIMEOUT_SECS: u64 = 90;
const MAX_SYSTEM_CHARS: usize = 20_000;
const MAX_USER_CHARS: usize = 60_000;
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const DEFAULT_MAX_TOKENS: u32 = 4096;
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Openai,
    Anthropic,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    pub provider: Provider,
    pub base_url: String,
    pub model: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self { provider: Provider::Openai, base_url: "https://api.openai.com/v1".into(), model: String::new() }
    }
}

/// Status yang aman dikirim ke UI (tanpa key).
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmStatus {
    pub provider: Provider,
    pub base_url: String,
    pub model: String,
    pub has_key: bool,
    pub configured: bool,
}

// ---------- Logika murni (teruji tanpa jaringan) ----------

fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map(|p| p.1).unwrap_or(url);
    let host_port = rest.split(|c| c == '/' || c == '?' || c == '#').next().unwrap_or("");
    let host = if host_port.starts_with('[') {
        host_port.trim_start_matches('[').split(']').next().unwrap_or("")
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    host.to_ascii_lowercase()
}

fn is_local(url: &str) -> bool {
    matches!(host_of(url).as_str(), "localhost" | "127.0.0.1" | "::1")
}

/// HTTPS wajib, kecuali localhost (Ollama/LM Studio). Menolak userinfo (`user:pass@host`).
fn normalize_base_url(raw: &str) -> Result<String, String> {
    let url = raw.trim().trim_end_matches('/').to_string();
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| "URL harus diawali https:// (atau http:// untuk localhost)".to_string())?;
    let host_port = rest.split(|c| c == '/' || c == '?' || c == '#').next().unwrap_or("");
    if host_port.is_empty() || host_port.contains('@') || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("URL tidak valid".into());
    }
    match scheme.to_ascii_lowercase().as_str() {
        "https" => Ok(url),
        "http" if is_local(&url) => Ok(url),
        "http" => Err("HTTP hanya diizinkan untuk localhost; gunakan HTTPS".into()),
        _ => Err("Skema URL harus https (atau http untuk localhost)".into()),
    }
}

fn is_configured(cfg: &LlmConfig, has_key: bool) -> bool {
    !cfg.model.trim().is_empty() && (has_key || (cfg.provider == Provider::Openai && is_local(&cfg.base_url)))
}

struct HttpPlan {
    url: String,
    headers: Vec<(&'static str, String)>,
    body: Value,
}

fn build_request(cfg: &LlmConfig, key: Option<&str>, system: &str, user: &str, max_tokens: u32) -> HttpPlan {
    match cfg.provider {
        Provider::Openai => {
            let mut messages = Vec::new();
            if !system.trim().is_empty() {
                messages.push(json!({ "role": "system", "content": system }));
            }
            messages.push(json!({ "role": "user", "content": user }));
            let mut headers = Vec::new();
            if let Some(k) = key {
                headers.push(("authorization", format!("Bearer {k}")));
            }
            HttpPlan {
                url: format!("{}/chat/completions", cfg.base_url),
                headers,
                body: json!({ "model": cfg.model, "messages": messages }),
            }
        }
        Provider::Anthropic => {
            let mut body = json!({
                "model": cfg.model,
                "max_tokens": max_tokens,
                "messages": [{ "role": "user", "content": user }],
            });
            if !system.trim().is_empty() {
                body["system"] = json!(system);
            }
            let url = if cfg.base_url.ends_with("/v1") {
                format!("{}/messages", cfg.base_url)
            } else {
                format!("{}/v1/messages", cfg.base_url)
            };
            let mut headers = vec![("anthropic-version", ANTHROPIC_VERSION.to_string())];
            if let Some(k) = key {
                headers.push(("x-api-key", k.to_string()));
            }
            HttpPlan { url, headers, body }
        }
    }
}

fn parse_response(provider: Provider, body: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(body).map_err(|_| "Respons penyedia bukan JSON yang valid".to_string())?;
    let text = match provider {
        Provider::Openai => match v.pointer("/choices/0/message/content") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(parts)) => parts
                .iter()
                .filter_map(|p| p.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(""),
            _ => String::new(),
        },
        Provider::Anthropic => v
            .get("content")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(|b| b.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default(),
    };
    let text = text.trim().to_string();
    if text.is_empty() {
        Err("Model tidak mengembalikan teks".into())
    } else {
        Ok(text)
    }
}

fn api_error(status: u16, body: &str) -> String {
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| v.get("error").and_then(Value::as_str).map(str::to_string))
        })
        .unwrap_or_default();
    let detail: String = detail.chars().take(300).collect();
    let head = match status {
        401 | 403 => "API key ditolak atau tidak punya akses",
        404 => "Endpoint atau model tidak ditemukan (periksa URL dan nama model)",
        429 => "Batas permintaan atau kuota tercapai",
        500..=599 => "Server penyedia sedang bermasalah",
        _ => "Permintaan ditolak penyedia",
    };
    if detail.is_empty() {
        format!("{head} (HTTP {status})")
    } else {
        format!("{head} (HTTP {status}): {detail}")
    }
}

fn net_error(e: reqwest::Error) -> String {
    if e.is_timeout() {
        format!("Permintaan melewati batas waktu ({TIMEOUT_SECS} dtk)")
    } else if e.is_connect() {
        "Tidak dapat terhubung ke server. Periksa URL dan koneksi internet".into()
    } else {
        "Gagal menghubungi server".into()
    }
}

// ---------- Penyimpanan ----------

fn load_config(storage: &Storage) -> Result<LlmConfig, String> {
    Ok(storage
        .get(CONFIG_KEY)?
        .and_then(|raw| serde_json::from_str::<LlmConfig>(&raw).ok())
        .unwrap_or_default())
}

fn load_key(storage: &Storage) -> Result<Option<String>, String> {
    Ok(storage.get(SECRET_KEY)?.filter(|k| !k.trim().is_empty()))
}

fn status_of(storage: &Storage) -> Result<LlmStatus, String> {
    let cfg = load_config(storage)?;
    let has_key = load_key(storage)?.is_some();
    let configured = is_configured(&cfg, has_key);
    Ok(LlmStatus { provider: cfg.provider, base_url: cfg.base_url, model: cfg.model, has_key, configured })
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

// ---------- Commands ----------

#[tauri::command]
pub async fn llm_get_config(storage: State<'_, Storage>) -> Result<LlmStatus, String> {
    let s = storage.inner().clone();
    blocking(move || status_of(&s)).await
}

/// `api_key`: `None` = biarkan key lama; string kosong = hapus; selain itu = simpan.
#[tauri::command]
pub async fn llm_set_config(
    storage: State<'_, Storage>,
    provider: Provider,
    base_url: String,
    model: String,
    api_key: Option<String>,
) -> Result<LlmStatus, String> {
    let base_url = normalize_base_url(&base_url)?;
    let model = model.trim().to_string();
    if model.len() > 200 || model.chars().any(|c| c.is_control()) {
        return Err("Nama model tidak valid".into());
    }
    let key = api_key.map(|k| k.trim().to_string());
    if let Some(k) = &key {
        if k.len() > 512 || k.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return Err("API key tidak valid".into());
        }
    }
    let cfg = LlmConfig { provider, base_url, model };
    let cfg_json = serde_json::to_string(&cfg).map_err(|e| e.to_string())?;

    let s = storage.inner().clone();
    blocking(move || {
        match key.as_deref() {
            Some("") => s.delete(SECRET_KEY)?,
            Some(k) => s.set(SECRET_KEY, k)?,
            None => {}
        }
        s.set(CONFIG_KEY, &cfg_json)?;
        status_of(&s)
    })
    .await
}

/// Satu permintaan non-streaming. `system` boleh kosong. Menahan Deep Sleep selama berjalan
/// agar WebView tidak dihancurkan sebelum hasilnya diterima.
#[tauri::command]
pub async fn llm_complete(
    app: AppHandle,
    storage: State<'_, Storage>,
    system: String,
    user: String,
    max_tokens: Option<u32>,
) -> Result<String, String> {
    if user.trim().is_empty() {
        return Err("Prompt kosong".into());
    }
    if system.chars().count() > MAX_SYSTEM_CHARS || user.chars().count() > MAX_USER_CHARS {
        return Err("Prompt terlalu panjang".into());
    }

    let s = storage.inner().clone();
    let (cfg, key) = blocking(move || Ok((load_config(&s)?, load_key(&s)?))).await?;
    let cfg = LlmConfig { base_url: normalize_base_url(&cfg.base_url)?, ..cfg };
    if cfg.model.trim().is_empty() {
        return Err("Model belum diatur. Buka Pengaturan → AI".into());
    }
    if !is_configured(&cfg, key.is_some()) {
        return Err("API key belum diatur. Buka Pengaturan → AI".into());
    }

    let _awake = AwakeGuard::acquire(&app);
    let plan = build_request(&cfg, key.as_deref(), &system, &user, max_tokens.unwrap_or(DEFAULT_MAX_TOKENS).clamp(16, 16_000));

    // Tanpa redirect: key tidak boleh ikut terkirim ke host lain.
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;

    let body = serde_json::to_vec(&plan.body).map_err(|e| e.to_string())?;
    let mut req = client.post(&plan.url).header("content-type", "application/json");
    for (name, value) in &plan.headers {
        req = req.header(*name, value.as_str());
    }
    let res = req.body(body).send().await.map_err(net_error)?;

    let status = res.status();
    if res.content_length().map_or(false, |n| n as usize > MAX_RESPONSE_BYTES) {
        return Err("Respons penyedia terlalu besar".into());
    }
    let bytes = res.bytes().await.map_err(net_error)?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err("Respons penyedia terlalu besar".into());
    }
    let text = String::from_utf8_lossy(&bytes);
    if !status.is_success() {
        return Err(api_error(status.as_u16(), &text));
    }
    parse_response(cfg.provider, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(provider: Provider, base: &str) -> LlmConfig {
        LlmConfig { provider, base_url: base.into(), model: "m1".into() }
    }

    #[test]
    fn base_url_https_wajib_kecuali_localhost() {
        assert_eq!(normalize_base_url(" https://api.openai.com/v1/ ").unwrap(), "https://api.openai.com/v1");
        assert_eq!(normalize_base_url("http://localhost:11434/v1").unwrap(), "http://localhost:11434/v1");
        assert!(normalize_base_url("http://127.0.0.1:8080").is_ok());
        assert!(normalize_base_url("http://[::1]:8080/v1").is_ok());
        assert!(normalize_base_url("http://example.com/v1").is_err());
        assert!(normalize_base_url("ftp://example.com").is_err());
        assert!(normalize_base_url("api.openai.com").is_err());
        assert!(normalize_base_url("https://user:pw@example.com").is_err());
        assert!(normalize_base_url("https://").is_err());
        assert!(normalize_base_url("https://exa mple.com").is_err());
        // Trik host: "localhost" hanya di path/userinfo tidak boleh dianggap lokal.
        assert!(normalize_base_url("http://evil.com/localhost").is_err());
        assert!(normalize_base_url("http://localhost@evil.com").is_err());
    }

    #[test]
    fn is_configured_butuh_model_dan_key_kecuali_ollama() {
        let remote = cfg(Provider::Openai, "https://api.openai.com/v1");
        assert!(!is_configured(&remote, false));
        assert!(is_configured(&remote, true));
        assert!(is_configured(&cfg(Provider::Openai, "http://localhost:11434/v1"), false));
        assert!(!is_configured(&cfg(Provider::Anthropic, "http://localhost:1234"), false));
        assert!(!is_configured(&LlmConfig { model: "  ".into(), ..remote }, true));
    }

    #[test]
    fn request_openai_dan_anthropic() {
        let p = build_request(&cfg(Provider::Openai, "https://x.io/v1"), Some("k"), "sys", "hai", 100);
        assert_eq!(p.url, "https://x.io/v1/chat/completions");
        assert_eq!(p.headers, vec![("authorization", "Bearer k".to_string())]);
        assert_eq!(p.body["messages"][0]["role"], "system");
        assert_eq!(p.body["messages"][1]["content"], "hai");
        assert!(p.body.get("max_tokens").is_none() && p.body.get("temperature").is_none());

        let p = build_request(&cfg(Provider::Openai, "http://localhost:11434/v1"), None, "  ", "hai", 100);
        assert!(p.headers.is_empty());
        assert_eq!(p.body["messages"].as_array().unwrap().len(), 1, "system kosong dilewati");

        let p = build_request(&cfg(Provider::Anthropic, "https://api.anthropic.com"), Some("k"), "sys", "hai", 256);
        assert_eq!(p.url, "https://api.anthropic.com/v1/messages");
        assert!(p.headers.contains(&("x-api-key", "k".to_string())));
        assert_eq!(p.body["max_tokens"], 256);
        assert_eq!(p.body["system"], "sys");
        let p = build_request(&cfg(Provider::Anthropic, "https://api.anthropic.com/v1"), Some("k"), "", "hai", 256);
        assert_eq!(p.url, "https://api.anthropic.com/v1/messages");
        assert!(p.body.get("system").is_none());
    }

    #[test]
    fn parse_respons() {
        let o = r#"{"choices":[{"message":{"content":"  Halo  "}}]}"#;
        assert_eq!(parse_response(Provider::Openai, o).unwrap(), "Halo");
        let parts = r#"{"choices":[{"message":{"content":[{"type":"text","text":"A"},{"type":"text","text":"B"}]}}]}"#;
        assert_eq!(parse_response(Provider::Openai, parts).unwrap(), "AB");
        let a = r#"{"content":[{"type":"thinking","thinking":"x"},{"type":"text","text":"Hai"}]}"#;
        assert_eq!(parse_response(Provider::Anthropic, a).unwrap(), "Hai");
        assert!(parse_response(Provider::Openai, r#"{"choices":[]}"#).is_err());
        assert!(parse_response(Provider::Anthropic, "bukan json").is_err());
    }

    #[test]
    fn pesan_error_api() {
        let m = api_error(401, r#"{"error":{"message":"Incorrect API key"}}"#);
        assert!(m.contains("HTTP 401") && m.contains("Incorrect API key"));
        assert!(api_error(429, "").contains("kuota"));
        assert!(api_error(502, "<html>").contains("HTTP 502"));
        assert!(api_error(400, r#"{"error":"plain"}"#).contains("plain"));
        assert!(api_error(400, &format!(r#"{{"error":{{"message":"{}"}}}}"#, "x".repeat(900))).len() < 400);
    }

    #[test]
    fn config_round_trip_dan_secret_tidak_bocor_ke_status() {
        let s = Storage::open_in_memory().unwrap();
        let st = status_of(&s).unwrap();
        assert!(!st.configured && !st.has_key);

        s.set(SECRET_KEY, "rahasia").unwrap();
        let c = LlmConfig { provider: Provider::Anthropic, base_url: "https://api.anthropic.com".into(), model: "m".into() };
        s.set(CONFIG_KEY, &serde_json::to_string(&c).unwrap()).unwrap();
        let st = status_of(&s).unwrap();
        assert!(st.configured && st.has_key);
        assert!(!serde_json::to_string(&st).unwrap().contains("rahasia"));

        s.set(CONFIG_KEY, "{rusak").unwrap();
        assert_eq!(load_config(&s).unwrap(), LlmConfig::default(), "config rusak -> default");
    }
}
