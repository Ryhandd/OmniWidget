import type { Control, Device, Player, RepeatMode, SpotifyErr, Status } from './types';

// Logika murni modul Spotify (tanpa DOM/Tauri) agar teruji di `npm run test:web`.

export type Phase = 'loading' | 'setup' | 'connect' | 'player';

export type State = {
  phase: Phase;
  status: Status | null;
  /** null = tidak ada sesi pemutaran aktif (belum ada perangkat yang memutar). */
  player: Player | null;
  /** Date.now() saat `player` diterima: dasar interpolasi progres di UI. */
  receivedAt: number;
  devices: Device[];
  error: SpotifyErr | null;
  logging: boolean;
};

export type Action =
  | { type: 'status'; status: Status }
  | { type: 'login-start' }
  | { type: 'login-end' }
  | { type: 'player'; player: Player | null; at: number }
  | { type: 'devices'; devices: Device[] }
  | { type: 'error'; error: SpotifyErr }
  | { type: 'clear-error' }
  | { type: 'optimistic'; control: Control; at: number };

export const initialState: State = {
  phase: 'loading',
  status: null,
  player: null,
  receivedAt: 0,
  devices: [],
  error: null,
  logging: false,
};

export function phaseOf(s: Status): Phase {
  if (!s.clientId) return 'setup';
  return s.connected ? 'player' : 'connect';
}

/** Error sementara: hilang sendiri begitu polling berikutnya sukses. Sisanya menunggu pengguna. */
const TRANSIENT = new Set(['network', 'rate_limited', 'api']);
export const isTransient = (e: SpotifyErr | null) => !!e && TRANSIENT.has(e.code);

/** Error yang berarti sesi tidak lagi valid: kembali ke layar yang sesuai. */
const NEEDS_CONNECT = new Set(['session_expired', 'not_connected']);

const REPEAT_CYCLE: RepeatMode[] = ['off', 'context', 'track'];
export const nextRepeat = (m: RepeatMode): RepeatMode => REPEAT_CYCLE[(REPEAT_CYCLE.indexOf(m) + 1) % REPEAT_CYCLE.length];

/** Efek lokal instan dari sebuah perintah, sebelum Spotify mengonfirmasi (UI terasa responsif). */
export function applyControl(p: Player | null, c: Control): Player | null {
  if (!p) return p;
  switch (c.action) {
    case 'play': return { ...p, isPlaying: true };
    case 'pause': return { ...p, isPlaying: false };
    case 'shuffle': return { ...p, shuffle: c.on };
    case 'repeat': return { ...p, repeat: c.mode };
    case 'seek': return { ...p, progressMs: c.position_ms };
    case 'volume': return p.device ? { ...p, device: { ...p.device, volumePercent: c.percent } } : p;
    default: return p; // next/previous/transfer: tunggu data asli dari Spotify
  }
}

export function reducer(s: State, a: Action): State {
  switch (a.type) {
    case 'status': {
      const phase = phaseOf(a.status);
      // Keluar dari 'player' (logout / ganti Client ID): buang data pemutaran lama.
      return { ...s, status: a.status, phase, player: phase === 'player' ? s.player : null, devices: phase === 'player' ? s.devices : [] };
    }
    case 'login-start': return { ...s, logging: true, error: null };
    case 'login-end': return { ...s, logging: false };
    case 'player': return { ...s, player: a.player, receivedAt: a.at, error: isTransient(s.error) ? null : s.error };
    case 'devices': return { ...s, devices: a.devices };
    case 'clear-error': return { ...s, error: null };
    case 'optimistic': return { ...s, player: applyControl(s.player, a.control), receivedAt: a.at, error: null };
    case 'error': {
      const code = a.error.code;
      if (NEEDS_CONNECT.has(code) && s.status) {
        return { ...s, error: a.error, phase: 'connect', status: { ...s.status, connected: false }, player: null, devices: [] };
      }
      if (code === 'no_client_id' && s.status) {
        return { ...s, error: a.error, phase: 'setup', status: { ...s.status, clientId: null, connected: false }, player: null, devices: [] };
      }
      return { ...s, error: a.error };
    }
  }
}

/** Progres sekarang: bila sedang memutar, maju sejak data diterima (tanpa melewati durasi). */
export function progressNow(p: Player, receivedAt: number, now: number): number {
  const dur = p.item?.durationMs ?? 0;
  const raw = p.isPlaying ? p.progressMs + Math.max(0, now - receivedAt) : p.progressMs;
  return dur > 0 ? Math.min(dur, Math.max(0, raw)) : Math.max(0, raw);
}

export function fmtTime(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const sec = String(total % 60).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`;
}

/** Client ID Spotify: 32 heksadesimal (huruf besar diterima; Rust menormalkan ke huruf kecil). */
export const isClientId = (s: string) => /^[0-9a-f]{32}$/i.test(s.trim());
