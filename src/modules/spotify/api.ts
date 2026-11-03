import { bridge } from '../../core/bridge';
import type { Control, Device, Player, Status } from './types';

// Satu-satunya tempat modul ini memanggil Rust. Argumen top-level camelCase (aturan Tauri 2);
// isi `command` tetap snake_case karena mengikuti serde di Rust. Token tidak pernah melewati sini.
export const spotifyApi = {
  status: () => bridge.invoke<Status>('spotify_status'),
  /** '' = hapus. Mengganti Client ID memutus sesi lama. */
  setClientId: (clientId: string) => bridge.invoke<Status>('spotify_set_client_id', { clientId }),
  /** Baru kembali setelah login selesai/gagal/dibatalkan (browser terbuka selama itu). */
  login: () => bridge.invoke<Status>('spotify_login'),
  cancelLogin: () => bridge.invoke<boolean>('spotify_login_cancel'),
  logout: () => bridge.invoke<Status>('spotify_logout'),
  /** null = tidak ada sesi pemutaran aktif. */
  player: () => bridge.invoke<Player | null>('spotify_player'),
  devices: () => bridge.invoke<Device[]>('spotify_devices'),
  control: (command: Control) => bridge.invoke<void>('spotify_control', { command }),
  /** Sampul diunduh Rust (CSP WebView melarang gambar jarak jauh) dan dikembalikan sebagai data URI. */
  cover: (url: string) => bridge.invoke<string>('spotify_cover', { url }),
};
