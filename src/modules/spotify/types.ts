// Cermin tipe serde di Rust (`plugins/spotify`). Field respons camelCase; isi `Control` snake_case.

export type RepeatMode = 'off' | 'context' | 'track';

export type Device = {
  id: string | null;
  name: string;
  kind: string;
  isActive: boolean;
  volumePercent: number | null;
  supportsVolume: boolean;
};

export type Track = {
  kind: 'track' | 'episode' | string;
  name: string;
  artists: string;
  album: string;
  durationMs: number;
  imageUrl: string | null;
};

export type Player = {
  isPlaying: boolean;
  progressMs: number;
  shuffle: boolean;
  repeat: RepeatMode;
  device: Device | null;
  /** null saat iklan atau sesi privat. */
  item: Track | null;
};

export type Status = {
  clientId: string | null;
  connected: boolean;
  /** Harus didaftarkan PERSIS di dashboard Spotify. */
  redirectUri: string;
};

/** Semua error modul tiba dari Rust sebagai objek ini (lihat `error.rs`). */
export type SpotifyErr = { code: string; message: string };

export type Control =
  | { action: 'play' }
  | { action: 'pause' }
  | { action: 'next' }
  | { action: 'previous' }
  | { action: 'seek'; position_ms: number }
  | { action: 'volume'; percent: number }
  | { action: 'shuffle'; on: boolean }
  | { action: 'repeat'; mode: RepeatMode }
  | { action: 'transfer'; device_id: string; play: boolean };
