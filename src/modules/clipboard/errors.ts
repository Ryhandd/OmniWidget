// Murni (tanpa Tauri) agar teruji di `npm run test:web`.

/** Error dari Rust tiba sebagai string; di browser biasa invoke melempar TypeError. */
export function clipErr(e: unknown): string {
  if (typeof e === 'string' && e) return e;
  if (e && typeof e === 'object' && 'message' in e && typeof (e as { message: unknown }).message === 'string') {
    const m = (e as { message: string }).message;
    if (/invoke|__TAURI/i.test(m)) return 'Clipboard hanya berjalan di aplikasi OmniWidget (bukan di browser biasa).';
    if (m) return m;
  }
  return 'Terjadi kesalahan tak terduga';
}
