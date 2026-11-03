import { bridge } from '../../core/bridge';
import { DEFAULT_PREFS, PREFS_KEY, parsePrefs, type OutputPrefs } from './prefs';
import type { JobEvent, JobRequest, ProbeResult } from './types';

// Satu-satunya tempat modul ini memanggil Rust. Argumen top-level dikirim camelCase (aturan Tauri 2);
// isi `job` tetap snake_case karena mengikuti struct serde di Rust.
export const converterApi = {
  probe: (path: string) => bridge.invoke<ProbeResult>('converter_probe', { path }),

  /** Mengembalikan job_id setelah job masuk antrean; hasil akhir datang lewat `onEvent`. */
  start: (job: JobRequest, onEvent: (ev: JobEvent) => void) =>
    bridge.invoke<string>('converter_start', { job, onEvent: bridge.channel<JobEvent>(onEvent) }),

  /**
   * Simpan byte file (dari drop/paste DOM, yang tidak punya path) ke folder staging sementara di Rust,
   * lalu kembalikan path-nya. Dikirim sebagai badan biner mentah; nama lewat header (di-encode).
   */
  stage: (name: string, bytes: Uint8Array) =>
    bridge.invoke<string>('converter_stage_upload', bytes, { headers: { 'x-file-name': encodeURIComponent(name) } }),

  cancel: (jobId: string) => bridge.invoke<boolean>('converter_cancel', { jobId }),
};

// ---------- Preferensi folder output (disimpan di kv Rust) ----------

export const loadPrefs = (): Promise<OutputPrefs> =>
  bridge.invoke<string | null>('kv_get', { key: PREFS_KEY }).then(parsePrefs, () => DEFAULT_PREFS);

export const savePrefs = (p: OutputPrefs) =>
  bridge.invoke('kv_set', { key: PREFS_KEY, value: JSON.stringify(p) }).catch(() => {});
