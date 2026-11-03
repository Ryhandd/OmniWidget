// Kontrak IPC dengan plugin Rust `converter` (src-tauri/src/plugins/converter/mod.rs).

/** Nama format huruf kecil, mis. 'png', 'pdf', 'mp3'. Daftar valid ditentukan Rust (rute konversi). */
export type Format = string;

export type Engine = 'native' | 'ffmpeg' | 'pandoc' | 'pdfium';

export type TargetInfo = {
  format: Format;
  engine: Engine;
  /** Komponen tambahan yang dibutuhkan (null = bawaan). */
  requires: string | null;
  /** Komponen yang dibutuhkan sudah terpasang? */
  available: boolean;
};

export type ProbeResult = { source: Format; targets: TargetInfo[] };

export type PdfPages = 'first' | 'all';

export type JobRequest = {
  input_path: string;
  target_format: Format;
  /** null/undefined = folder yang sama dengan berkas sumber. */
  output_dir?: string | null;
  output_name?: string | null;
  options?: { pdf_pages?: PdfPages };
};

export type JobEvent =
  | { event: 'queued' }
  | { event: 'started' }
  | { event: 'progress'; pct: number } // 0..1
  | { event: 'complete'; path: string; folder: boolean }
  | { event: 'error'; message: string }
  | { event: 'cancelled' };
