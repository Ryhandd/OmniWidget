import { listen } from '@tauri-apps/api/event';
import { bridge } from '../../core/bridge';
import type { ClipSettingsPatch, ClipSnapshot } from './types';

// Satu-satunya tempat modul ini menyentuh Rust. Argumen top-level camelCase (aturan Tauri 2).
//
// KONTRAK COMMAND YANG DIHARAPKAN DARI BACKEND (nama bisa disesuaikan di sini saja):
//   clipboard_snapshot()                          -> ClipSnapshot            (urutan: terbaru dulu)
//   clipboard_thumb(id)                           -> string | null           (data URI PNG, <= 20 KB; null = tak ada)
//   clipboard_text(id)                            -> string                  (teks penuh, <= 64 KB)
//   clipboard_paste(id)                           -> void                    (tulis ke clipboard sistem dgn flag self_write,
//                                                                             lalu pindahkan item ke depan ring)
//   clipboard_pin(id, pinned)                     -> void
//   clipboard_remove(id)                          -> void                    (hapus juga berkas gambar di disk)
//   clipboard_clear(keepPinned)                   -> number                  (jumlah item terhapus)
//   clipboard_set_settings(patch)                 -> ClipSettings
// EVENT (Rust -> UI, di-debounce 50 ms, payload diabaikan UI):
//   "clipboard://changed"                         -> UI memanggil ulang clipboard_snapshot
export const clipApi = {
  snapshot: () => bridge.invoke<ClipSnapshot>('clipboard_snapshot'),
  /** null = thumbnail tidak tersedia (sudah dilepas Rust di trim L2+ dan gagal dimuat ulang dari disk). */
  thumb: (id: string) => bridge.invoke<string | null>('clipboard_thumb', { id }),
  text: (id: string) => bridge.invoke<string>('clipboard_text', { id }),
  paste: (id: string) => bridge.invoke<void>('clipboard_paste', { id }),
  pin: (id: string, pinned: boolean) => bridge.invoke<void>('clipboard_pin', { id, pinned }),
  remove: (id: string) => bridge.invoke<void>('clipboard_remove', { id }),
  clear: (keepPinned: boolean) => bridge.invoke<number>('clipboard_clear', { keepPinned }),
  setSettings: (patch: ClipSettingsPatch) => bridge.invoke<ClipSnapshot['settings']>('clipboard_set_settings', { patch }),
};

/**
 * Berlangganan perubahan riwayat. Mengembalikan fungsi pelepas (sinkron, aman dipanggil sebelum listen selesai).
 * Di luar Tauri (vite di browser) listen gagal diam-diam: UI tetap bisa dipratinjau.
 */
export function onClipChanged(fn: () => void): () => void {
  let off: (() => void) | undefined;
  let disposed = false;
  listen('clipboard://changed', fn)
    .then((u) => (disposed ? u() : (off = u)))
    .catch(() => {});
  return () => {
    disposed = true;
    off?.();
  };
}
