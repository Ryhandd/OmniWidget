//! Plugin Converter (TDD 4.1). Berkas ini hanya berisi *glue* Tauri: validasi permintaan, antrean,
//! dan pelaporan progres. Logika konversi ada di `raster`, `pdf`, `sidecar`; penamaan output di `output`;
//! pembatalan & antrean di `job`.

pub mod error;
pub mod formats;
pub mod job;
pub mod output;
pub mod pdf;
pub mod raster;
pub mod render_budget;
pub mod sidecar;
pub mod staging;

pub use job::CancelToken;

use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};

use serde::{Deserialize, Serialize};
use tauri::{ipc::Channel, AppHandle, Manager};
use tokio::sync::Semaphore;

use crate::core::sidecar::{commands::resolve_dir, SidecarState};
use crate::core::state::AwakeGuard;
use error::ConvError;
use formats::{Engine, Format};
use job::{JobHandle, JobRegistry, DEFAULT_CONCURRENCY};
use output::{OutKind, PartGuard};

/// Daftarkan state plugin. Dipanggil sekali dari `setup` (objeknya kecil: peta id -> token).
pub fn setup(app: &AppHandle) {
    app.manage(JobRegistry::new(DEFAULT_CONCURRENCY));
    staging::spawn_purge(app);
}

// ---------- Tipe IPC ----------

#[derive(Deserialize, Default)]
pub struct JobOptions {
    /// Hanya berlaku untuk PDF -> PNG/JPG.
    #[serde(default)]
    pub pdf_pages: pdf::Pages,
}

#[derive(Deserialize)]
pub struct JobRequest {
    pub input_path: String,
    pub target_format: Format,
    /// Kosong = folder yang sama dengan berkas sumber.
    #[serde(default)]
    pub output_dir: Option<String>,
    /// Kosong = nama berkas sumber tanpa ekstensi. Tidak pernah menimpa: bentrok diberi sufiks -1, -2.
    #[serde(default)]
    pub output_name: Option<String>,
    #[serde(default)]
    pub options: JobOptions,
}

#[derive(Serialize, Clone)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum JobEvent {
    /// Job menunggu giliran (konkurensi dibatasi).
    Queued,
    Started,
    Progress { pct: f32 },
    /// `folder` = true bila hasilnya satu folder (PDF multi-halaman -> banyak gambar).
    Complete { path: String, folder: bool },
    Error { message: String },
    Cancelled,
}

#[derive(Serialize)]
pub struct TargetInfo {
    pub format: Format,
    pub engine: Engine,
    /// Nama komponen tambahan yang dibutuhkan (None = bawaan).
    pub requires: Option<&'static str>,
    /// Komponen yang dibutuhkan sudah terpasang?
    pub available: bool,
}

#[derive(Serialize)]
pub struct ProbeResult {
    pub source: Format,
    pub targets: Vec<TargetInfo>,
}

// ---------- Commands ----------

#[tauri::command]
pub async fn converter_probe(app: AppHandle, path: String) -> Result<ProbeResult, ConvError> {
    let p = Path::new(&path);
    if !p.is_file() {
        return Err(ConvError::InputMissing(path));
    }
    let source = formats::detect(p).ok_or(ConvError::UnknownFormat)?;
    let targets = formats::targets_for(source)
        .into_iter()
        .map(|r| TargetInfo {
            format: r.to,
            engine: r.engine,
            requires: r.engine.sidecar(),
            available: engine_available(&app, r.engine),
        })
        .collect();
    Ok(ProbeResult { source, targets })
}

/// Validasi cepat lalu masukkan job ke antrean. Mengembalikan `job_id`; hasil dilaporkan lewat `on_event`.
#[tauri::command]
pub async fn converter_start(
    app: AppHandle,
    job: JobRequest,
    on_event: Channel<JobEvent>,
) -> Result<String, ConvError> {
    let plan = Plan::build(&app, job)?;

    let (handle, gate) = {
        let registry = app.state::<JobRegistry>();
        (registry.register(), registry.gate())
    };
    let id = handle.id.clone();

    // Tahan Deep Sleep sejak job didaftarkan (termasuk saat masih antre) sampai event akhir terkirim.
    // WebView yang dihancurkan = Channel progres mati; job akan jalan tanpa ada yang menerima hasilnya.
    let awake = AwakeGuard::acquire(&app);

    let app = app.clone();
    tauri::async_runtime::spawn(run_queued(app, plan, handle, gate, Arc::new(on_event), awake));
    Ok(id)
}

#[tauri::command]
pub async fn converter_cancel(app: AppHandle, job_id: String) -> Result<bool, String> {
    Ok(app.state::<JobRegistry>().cancel(&job_id))
}

// ---------- Rencana job ----------

struct Plan {
    input: PathBuf,
    from: Format,
    to: Format,
    engine: Engine,
    out_dir: PathBuf,
    stem: String,
    pages: pdf::Pages,
    app_data: PathBuf,
    pdfium_dir: PathBuf,
}

impl Plan {
    fn build(app: &AppHandle, job: JobRequest) -> Result<Self, ConvError> {
        let input = PathBuf::from(&job.input_path);
        if !input.is_file() {
            return Err(ConvError::InputMissing(job.input_path));
        }
        let from = formats::detect(&input).ok_or(ConvError::UnknownFormat)?;
        let route = formats::find(from, job.target_format).ok_or(ConvError::NoRoute)?;

        if let Some(name) = route.engine.sidecar() {
            if !engine_available(app, route.engine) {
                return Err(ConvError::SidecarMissing(name));
            }
        }

        let out_dir = match job.output_dir.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(d) => PathBuf::from(d),
            // Input hasil paste/drop disimpan di folder sementara: hasilnya ke folder Unduhan, bukan ke sana.
            None => match staging::default_out_dir(app, &input)? {
                Some(dir) => dir,
                None => input.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(".")),
            },
        };
        if !out_dir.is_dir() {
            return Err(ConvError::BadOutDir(out_dir.display().to_string()));
        }

        let stem = match job.output_name.as_deref() {
            Some(n) if !n.trim().is_empty() => output::sanitize_stem(n),
            _ => output::stem_of(&input),
        };

        let state = app.state::<SidecarState>();
        Ok(Self {
            input,
            from,
            to: job.target_format,
            engine: route.engine,
            out_dir,
            stem,
            pages: job.options.pdf_pages,
            app_data: state.app_data_dir.clone(),
            pdfium_dir: resolve_dir(app, "pdfium").unwrap_or_else(|_| state.app_data_dir.join("sidecars").join("pdfium")),
        })
    }
}

fn engine_available(app: &AppHandle, engine: Engine) -> bool {
    let Some(name) = engine.sidecar() else { return true };
    let state = app.state::<SidecarState>();
    match engine {
        // ffmpeg/pandoc juga boleh berasal dari PATH sistem (sama seperti saat konversi dijalankan).
        Engine::Ffmpeg | Engine::Pandoc => sidecar::resolve(&state.app_data_dir, name).is_some(),
        // pdfium dipasang oleh pengunduh sidecar dan ditandai dengan berkas `.ok` (lihat sidecar_status).
        Engine::Pdfium => resolve_dir(app, name).map(|d| d.join(".ok").is_file()).unwrap_or(false),
        Engine::Native => true,
    }
}

// ---------- Eksekusi ----------

async fn run_queued(
    app: AppHandle,
    plan: Plan,
    handle: JobHandle,
    gate: Arc<Semaphore>,
    ch: Arc<Channel<JobEvent>>,
    awake: AwakeGuard,
) {
    // Slot langsung tersedia? Jalan. Jika tidak: kabari UI, lalu tunggu giliran atau pembatalan.
    let permit = match gate.clone().try_acquire_owned() {
        Ok(p) => Some(p),
        Err(_) => {
            let _ = ch.send(JobEvent::Queued);
            tokio::select! {
                p = gate.acquire_owned() => p.ok(),
                _ = handle.cancelled() => None,
            }
        }
    };

    let outcome: Result<(PathBuf, OutKind), ConvError> = match permit {
        None => Err(ConvError::Cancelled),
        Some(_permit) if handle.cancel.load(Ordering::SeqCst) => Err(ConvError::Cancelled),
        Some(_permit) => {
            let _ = ch.send(JobEvent::Started);
            let cancel = handle.cancel.clone();
            let job_id = handle.id.clone();
            let ch_worker = ch.clone();
            // Kerja berat di thread blocking; `_permit` tetap dipegang sampai selesai.
            match tokio::task::spawn_blocking(move || execute(&plan, &job_id, &cancel, &ch_worker)).await {
                Ok(r) => r,
                Err(_) => Err(ConvError::WorkerCrashed),
            }
        }
    };

    app.state::<JobRegistry>().finish(&handle.id);

    let _ = match outcome {
        Ok((path, kind)) => ch.send(JobEvent::Complete {
            path: path.to_string_lossy().into_owned(),
            folder: kind == OutKind::Dir,
        }),
        Err(e) if e.is_cancelled() => ch.send(JobEvent::Cancelled),
        Err(e) => ch.send(JobEvent::Error { message: e.to_string() }),
    };

    // Lepas SETELAH event akhir terkirim: hitung mundur idle dimulai ulang dari sini, jadi pengguna
    // punya waktu penuh untuk melihat hasilnya sebelum widget tidur.
    drop(awake);
}

/// Berjalan di thread blocking. Menulis ke `.part` tersembunyi, lalu rename atomik ke nama final.
/// `PartGuard` menjamin sisa `.part` terhapus pada error maupun pembatalan.
fn execute(
    plan: &Plan,
    job_id: &str,
    cancel: &CancelToken,
    ch: &Channel<JobEvent>,
) -> Result<(PathBuf, OutKind), ConvError> {
    // Kirim progres hanya saat persentase bulat berubah: tidak membanjiri IPC.
    let last = AtomicU8::new(u8::MAX);
    let progress = |p: f32| {
        let pct = (p.clamp(0.0, 1.0) * 100.0) as u8;
        if last.swap(pct, Ordering::Relaxed) != pct {
            let _ = ch.send(JobEvent::Progress { pct: pct as f32 / 100.0 });
        }
    };

    let part = output::part_path(&plan.out_dir, &plan.stem, job_id);
    let mut guard = PartGuard::new(part.clone());

    let kind = match plan.engine {
        Engine::Native => {
            raster::convert(&plan.input, &part, plan.to, cancel, &progress)?;
            OutKind::File
        }
        Engine::Ffmpeg | Engine::Pandoc => {
            sidecar::convert(plan.engine, &plan.app_data, &plan.input, &part, plan.from, plan.to, cancel, &progress)?;
            OutKind::File
        }
        Engine::Pdfium => pdf::convert(&plan.input, &part, plan.to, plan.pages, &plan.pdfium_dir, cancel, &progress)?,
    };

    // Dibatalkan tepat saat selesai: hasil dibuang, bukan dilaporkan sebagai sukses.
    if cancel.load(Ordering::SeqCst) {
        return Err(ConvError::Cancelled);
    }

    let dest = output::finalize(&part, &plan.out_dir, &plan.stem, plan.to.ext(), kind)?;
    guard.disarm();
    Ok((dest, kind))
}
