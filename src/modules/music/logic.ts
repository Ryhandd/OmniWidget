// Logika murni modul Musik: format waktu, interpolasi posisi, riwayat, dan tautan Spotify.
// Tanpa Preact/Tauri supaya ikut teruji di `npm run test:web`.

export type RepeatMode = 'off' | 'list' | 'track';

export type Snapshot = {
  available: boolean;
  app: string;
  title: string;
  artist: string;
  album: string;
  playing: boolean;
  positionMs: number;
  durationMs: number;
  shuffle: boolean;
  repeat: RepeatMode;
  canPlay: boolean;
  canPause: boolean;
  canNext: boolean;
  canPrev: boolean;
  canSeek: boolean;
  canShuffle: boolean;
  canRepeat: boolean;
  artKey: string;
  art: string | null;
};

export type TrackInfo = Pick<Snapshot, 'title' | 'artist' | 'album' | 'app'>;

export type HistoryEntry = TrackInfo & { at: number };

export type PlaylistShortcut = { id: string; name: string; uri: string };

export const HISTORY_MAX = 40;

/** `m:ss`, atau `h:mm:ss` untuk rekaman panjang. */
export function fmtTime(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return '0:00';
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const two = (n: number) => String(n).padStart(2, '0');
  return h > 0 ? `${h}:${two(m)}:${two(s)}` : `${m}:${two(s)}`;
}

/** Posisi saat ini menurut waktu baca terakhir; tidak pernah melewati durasi. */
export function interpolatePosition(
  positionMs: number,
  playing: boolean,
  fetchedAt: number,
  now: number,
  durationMs: number,
): number {
  const base = Number.isFinite(positionMs) ? Math.max(0, positionMs) : 0;
  const drift = playing ? Math.max(0, now - fetchedAt) : 0;
  const pos = base + drift;
  return durationMs > 0 ? Math.min(pos, durationMs) : pos;
}

export function progressPct(positionMs: number, durationMs: number): number {
  if (!(durationMs > 0)) return 0;
  return Math.max(0, Math.min(100, (positionMs / durationMs) * 100));
}

/** Spotify: off -> list -> track -> off. */
export function nextRepeat(mode: RepeatMode): RepeatMode {
  return mode === 'off' ? 'list' : mode === 'list' ? 'track' : 'off';
}

export function repeatLabel(mode: RepeatMode): string {
  return mode === 'track' ? 'Ulangi lagu ini' : mode === 'list' ? 'Ulangi antrean' : 'Ulangi mati';
}

/** Terima `spotify:...`, tautan open.spotify.com, atau tautan share dengan parameter `?si=`. */
export function parseSpotifyLink(raw: string): string | null {
  const s = raw.trim();
  if (!s) return null;
  if (s === 'spotify:' || s === 'spotify:collection') return s;
  const direct = /^spotify:(playlist|album|track|artist|episode|show):([A-Za-z0-9]+)$/.exec(s);
  if (direct) return `spotify:${direct[1]}:${direct[2]}`;

  let url: URL;
  try {
    url = new URL(s);
  } catch {
    return null;
  }
  if (url.protocol !== 'https:' && url.protocol !== 'http:') return null;
  if (url.hostname !== 'open.spotify.com') return null;
  const m = /^\/(playlist|album|track|artist|episode|show)\/([A-Za-z0-9]+)\/?$/.exec(url.pathname);
  return m ? `spotify:${m[1]}:${m[2]}` : null;
}

export function searchUri(query: string): string {
  return `spotify:search:${encodeURIComponent(query.trim())}`;
}

export function songSearchUri(track: TrackInfo): string {
  return searchUri([track.title, track.artist].filter(Boolean).join(' '));
}

/** Buang judul iklan dari riwayat; iklan Spotify bukan lagu yang ingin diingat pengguna. */
export function isHistoryWorthy(track: TrackInfo): boolean {
  const title = track.title.trim();
  if (!title) return false;
  return !/^(advertisement|iklan|spotify)$/i.test(title);
}

/** Riwayat terbaru di atas, satu baris per lagu (judul+artis), maksimal `limit`. */
export function pushHistory(
  history: HistoryEntry[],
  track: TrackInfo,
  at: number,
  limit = HISTORY_MAX,
): HistoryEntry[] {
  if (!isHistoryWorthy(track)) return history;
  const key = historyKey(track);
  const rest = history.filter((h) => historyKey(h) !== key);
  return [{ ...track, at }, ...rest].slice(0, limit);
}

export function historyKey(t: TrackInfo): string {
  return `${t.title.trim().toLowerCase()}|${t.artist.trim().toLowerCase()}`;
}

/** Nama aplikasi pemilik sesi media dari AUMID Windows. */
export function appLabel(aumid: string): string {
  const a = aumid.toLowerCase();
  if (a.includes('spotify')) return 'Spotify';
  if (a.includes('chrome')) return 'Google Chrome';
  if (a.includes('msedge')) return 'Microsoft Edge';
  if (a.includes('firefox')) return 'Firefox';
  if (a.includes('vlc')) return 'VLC';
  if (a.includes('foobar')) return 'foobar2000';
  if (a.includes('zune') || a.includes('groove')) return 'Groove Music';
  if (a.includes('mediaplayer') || a.includes('media player')) return 'Media Player';
  const tail = aumid.split('!').pop() ?? aumid;
  const name = tail.split('.').pop() || tail;
  return name ? name.charAt(0).toUpperCase() + name.slice(1) : 'Media';
}

export function parseHistory(raw: string | null): HistoryEntry[] {
  if (!raw) return [];
  try {
    const v = JSON.parse(raw);
    if (!Array.isArray(v)) return [];
    return v
      .filter((x): x is HistoryEntry =>
        !!x && typeof x.title === 'string' && typeof x.artist === 'string' && typeof x.at === 'number')
      .slice(0, HISTORY_MAX);
  } catch {
    return [];
  }
}

export function parsePlaylists(raw: string | null): PlaylistShortcut[] {
  if (!raw) return [];
  try {
    const v = JSON.parse(raw);
    if (!Array.isArray(v)) return [];
    return v
      .filter((x): x is PlaylistShortcut =>
        !!x && typeof x.id === 'string' && typeof x.name === 'string' && typeof x.uri === 'string')
      .slice(0, 30);
  } catch {
    return [];
  }
}
