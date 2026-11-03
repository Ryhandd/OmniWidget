// Logika murni penerimaan file (drag & drop DOM, paste, dialog): validasi dan pemilihan.
// Tanpa Tauri/DOM global supaya mudah diuji. Rust tetap otoritas akhir: ia memvalidasi isi berkas
// (magic bytes) lagi saat menyimpan dan saat probe, jadi daftar di sini hanya untuk gagal cepat & pesan yang jelas.

/** Cermin format sumber yang dikenal Rust (plugins/converter/formats.rs `detect`). */
export const SUPPORTED_EXTS = [
  'png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif', 'ico',
  'pdf', 'docx', 'md', 'markdown', 'html', 'htm', 'txt',
  'mp3', 'wav', 'flac', 'ogg', 'm4a', 'opus',
] as const;

const MIME_TO_EXT: Record<string, string> = {
  'image/png': 'png', 'image/jpeg': 'jpg', 'image/webp': 'webp', 'image/bmp': 'bmp', 'image/gif': 'gif',
  'image/x-icon': 'ico', 'image/vnd.microsoft.icon': 'ico',
  'application/pdf': 'pdf',
  'application/vnd.openxmlformats-officedocument.wordprocessingml.document': 'docx',
  'text/markdown': 'md', 'text/html': 'html', 'text/plain': 'txt',
  'audio/mpeg': 'mp3', 'audio/wav': 'wav', 'audio/x-wav': 'wav', 'audio/flac': 'flac', 'audio/x-flac': 'flac',
  'audio/ogg': 'ogg', 'audio/mp4': 'm4a', 'audio/x-m4a': 'm4a', 'audio/m4a': 'm4a', 'audio/opus': 'opus',
};

/** Batas file yang dikirim lewat IPC (harus sama dengan MAX_UPLOAD_BYTES di Rust staging.rs). */
export const MAX_UPLOAD_BYTES = 100 * 1024 * 1024;

/** Penanda folder staging pada path hasil simpan sementara (sama dengan STAGING_DIR di Rust). */
export const STAGING_DIR = 'omniwidget-staging';

export type Candidate = { name: string; size: number; type: string };
export type Verdict = { ok: true } | { ok: false; message: string };

export function extOf(name: string): string {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : '';
}

const mimeBase = (t: string) => t.split(';')[0].trim().toLowerCase();

/** Ekstensi untuk file tanpa ekstensi di namanya, ditebak dari MIME (mis. gambar dari clipboard). */
export const extFromMime = (type: string): string => MIME_TO_EXT[mimeBase(type)] ?? '';

const isSupportedExt = (e: string) => (SUPPORTED_EXTS as readonly string[]).includes(e);

export const mb = (bytes: number) => Math.round((bytes / 1024 / 1024) * 10) / 10;

export function validateCandidate(c: Candidate): Verdict {
  if (c.size <= 0) return { ok: false, message: `"${c.name || 'File'}" kosong atau berupa folder. Pilih file, bukan folder.` };
  if (c.size > MAX_UPLOAD_BYTES) {
    return { ok: false, message: `"${c.name}" terlalu besar (${mb(c.size)} MB, maks ${mb(MAX_UPLOAD_BYTES)} MB untuk tempel/seret). Gunakan "Pilih file…" untuk file besar.` };
  }
  const ext = extOf(c.name);
  if (ext ? !isSupportedExt(ext) : !isSupportedExt(extFromMime(c.type))) {
    const shown = ext ? `.${ext}` : c.type || 'tanpa ekstensi';
    return { ok: false, message: `Format ${shown} belum didukung. Didukung: ${[...new Set(SUPPORTED_EXTS.map((e) => e.toUpperCase()))].join(', ')}.` };
  }
  return { ok: true };
}

/** Pilih file pertama yang valid; sisanya dihitung sebagai "diabaikan" (konverter memproses satu file). */
export function pickUsable<T extends Candidate>(files: readonly T[]): { file: T | null; error: string | null; extra: number } {
  let error: string | null = null;
  for (const f of files) {
    const v = validateCandidate(f);
    if (v.ok) return { file: f, error: null, extra: files.length - 1 };
    error ??= v.message;
  }
  return { file: null, error: error ?? 'Tidak ada file yang bisa diproses.', extra: 0 };
}

const pad = (n: number) => String(n).padStart(2, '0');

/**
 * Nama untuk file hasil paste. Screenshot/gambar clipboard biasanya bernama generik ("image.png") atau kosong:
 * beri nama berstempel waktu. File yang disalin dari Explorer sudah punya nama asli: pertahankan.
 */
export function nameForPasted(file: { name: string; type: string }, now: Date): string {
  const ext = extOf(file.name) || extFromMime(file.type);
  const generic = file.name.trim() === '' || /^image\.[a-z0-9]+$/i.test(file.name.trim());
  if (!generic) return ext && !extOf(file.name) ? `${file.name}.${ext}` : file.name;
  const stamp = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  return `tempel-${stamp}${ext ? `.${ext}` : ''}`;
}

// ---------- Pembacaan DataTransfer (duck-typed) ----------

type ItemLike = { kind: string; getAsFile(): File | null };

/** Ada file pada operasi seret? (isi `files` baru terbaca saat drop; saat dragover hanya `types` yang tersedia.) */
export function hasFiles(dt: { types?: ArrayLike<string> } | null | undefined): boolean {
  return !!dt?.types && Array.from(dt.types).includes('Files');
}

/**
 * File dari clipboard. Wajib dipanggil SINKRON di dalam event paste: `DataTransferItem` tidak valid
 * lagi setelah handler selesai. Memakai `items` (kind === 'file'); `files` sebagai cadangan.
 */
export function filesFromClipboard(cd: { items?: ArrayLike<ItemLike>; files?: ArrayLike<File> } | null | undefined): File[] {
  if (!cd) return [];
  const out: File[] = [];
  const items = cd.items ? Array.from(cd.items) : [];
  for (const it of items) {
    if (it.kind !== 'file') continue;
    const f = it.getAsFile();
    if (f) out.push(f);
  }
  if (out.length === 0 && cd.files) out.push(...Array.from(cd.files));
  return out;
}

export const listFiles = (files: ArrayLike<File> | null | undefined): File[] => (files ? Array.from(files) : []);

/** Jangan membajak Ctrl+V saat pengguna sedang mengetik di kolom isian. */
export function isEditableTarget(t: EventTarget | null): boolean {
  const el = t as { tagName?: string; isContentEditable?: boolean } | null;
  const tag = el?.tagName?.toUpperCase();
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || el?.isContentEditable === true;
}

/** Path ini hasil simpan sementara (bukan lokasi asli milik pengguna)? */
export const isStagedPath = (p: string): boolean => new RegExp(`[\\\\/]${STAGING_DIR}[\\\\/]`).test(p);
