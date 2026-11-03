// Murni (tanpa Tauri) agar teruji di `npm run test:web`.
import type { SpotifyErr } from './types';

/** Normalisasi apa pun yang dilempar invoke menjadi { code, message }. */
export function toErr(e: unknown): SpotifyErr {
  if (e && typeof e === 'object' && 'code' in e && 'message' in e) {
    const o = e as { code: unknown; message: unknown };
    if (typeof o.code === 'string' && typeof o.message === 'string') return { code: o.code, message: o.message };
  }
  if (typeof e === 'string') return { code: 'api', message: e };
  const msg = e instanceof Error ? e.message : '';
  if (/invoke|__TAURI/i.test(msg)) {
    return { code: 'no_tauri', message: 'Spotify hanya berjalan di aplikasi OmniWidget (bukan di browser biasa).' };
  }
  return { code: 'api', message: msg || 'Terjadi kesalahan tak terduga' };
}
