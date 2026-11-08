// Store now-playing untuk seluruh shell: pill, MiniPlayer, dan modul Musik membaca sumber ini.
// Polling berjalan di App (bukan di modul) supaya pill tetap hidup saat panel ciut.
import { useEffect, useState } from 'preact/hooks';
import { bridge } from './bridge';
import {
  parseHistory,
  parsePlaylists,
  parseSpotifyLink,
  pushHistory,
  type HistoryEntry,
  type PlaylistShortcut,
  type RepeatMode,
  type Snapshot,
} from '../modules/music/logic';

export type MusicState = {
  snap: Snapshot | null;
  /** Data URI sampul terakhir yang berhasil diunduh (bertahan saat polling berikutnya tidak mengirim ulang). */
  art: string;
  fetchedAt: number;
  error: string | null;
  volume: number | null;
  history: HistoryEntry[];
  playlists: PlaylistShortcut[];
};

const HISTORY_KEY = 'omniwidget.music.history.v1';
const PLAYLIST_KEY = 'omniwidget.music.playlists.v1';

const ls = {
  get(k: string) {
    try { return localStorage.getItem(k); } catch { return null; }
  },
  set(k: string, v: string) {
    try { localStorage.setItem(k, v); } catch { /* kuota penuh: riwayat tetap hidup di memori */ }
  },
};

let state: MusicState = {
  snap: null,
  art: '',
  fetchedAt: 0,
  error: null,
  volume: null,
  history: parseHistory(ls.get(HISTORY_KEY)),
  playlists: parsePlaylists(ls.get(PLAYLIST_KEY)),
};

const subs = new Set<() => void>();
const emit = () => subs.forEach((f) => f());

function update(patch: Partial<MusicState>) {
  state = { ...state, ...patch };
  emit();
}

let started = false;
let inflight = false;
let timer: number | undefined;
let artKey = '';
let lastHistoryKey = '';
let volumeTimer: number | undefined;
let pendingVolume = 0;

function persistHistory() { ls.set(HISTORY_KEY, JSON.stringify(state.history)); }
function persistPlaylists() { ls.set(PLAYLIST_KEY, JSON.stringify(state.playlists)); }

function friendlyError(e: unknown): string {
  const msg = e instanceof Error ? e.message : typeof e === 'string' ? e : '';
  if (/invoke|__TAURI/i.test(msg)) return 'Musik hanya berjalan di aplikasi OmniWidget.';
  return msg || 'Gagal membaca sesi media Windows.';
}

async function poll() {
  if (inflight) return;
  inflight = true;
  try {
    const snap = await bridge.invoke<Snapshot>('nowplaying_state', { knownArt: artKey || null });
    let art = state.art;
    if (snap.art) {
      art = snap.art;
      artKey = snap.artKey;
    } else if (snap.artKey !== artKey) {
      // Sampul baru belum berhasil diunduh; kosongkan agar tidak menampilkan sampul lagu lama.
      art = '';
    }
    if (!snap.available) {
      art = '';
      artKey = '';
    }

    if (snap.available && snap.artKey && snap.artKey !== lastHistoryKey) {
      lastHistoryKey = snap.artKey;
      const history = pushHistory(state.history, snap, Date.now());
      if (history !== state.history) {
        state = { ...state, history };
        persistHistory();
      }
    }

    update({ snap, art, fetchedAt: Date.now(), error: null });
  } catch (e) {
    update({ error: friendlyError(e) });
  } finally {
    inflight = false;
  }
}

function schedule() {
  window.clearTimeout(timer);
  const delay = document.visibilityState === 'hidden' ? 5000 : 1000;
  timer = window.setTimeout(async () => {
    await poll();
    if (started) schedule();
  }, delay);
}

/** Mulai polling (idempoten). Dipanggil sekali dari App saat widget hidup. */
export function startNowPlaying() {
  if (started) return;
  started = true;
  void poll().then(schedule);
}

async function refreshNow() {
  await poll();
  if (started) schedule();
}

export const music = {
  refresh: refreshNow,

  async control(action: 'toggle' | 'play' | 'pause' | 'next' | 'prev', value?: unknown) {
    const snap = state.snap;
    if (snap) {
      let next = snap;
      if (action === 'toggle') next = { ...snap, playing: !snap.playing };
      else if (action === 'play') next = { ...snap, playing: true };
      else if (action === 'pause') next = { ...snap, playing: false };
      if (next !== snap) update({ snap: next, fetchedAt: Date.now() });
    }
    try {
      await bridge.invoke('nowplaying_control', { action, value: value ?? null });
    } catch (e) {
      update({ error: friendlyError(e) });
      await refreshNow();
      return;
    }
    window.setTimeout(() => { if (started) void poll(); }, 450);
  },

  async seek(positionMs: number) {
    const snap = state.snap;
    if (!snap) return;
    const ms = Math.max(0, Math.min(positionMs, snap.durationMs || positionMs));
    update({ snap: { ...snap, positionMs: ms }, fetchedAt: Date.now() });
    try {
      await bridge.invoke('nowplaying_control', { action: 'seek', value: ms });
    } catch (e) {
      update({ error: friendlyError(e) });
    }
    window.setTimeout(() => { if (started) void poll(); }, 450);
  },

  async toggleShuffle() {
    const snap = state.snap;
    if (!snap) return;
    update({ snap: { ...snap, shuffle: !snap.shuffle } });
    try {
      await bridge.invoke('nowplaying_control', { action: 'shuffle', value: !snap.shuffle });
    } catch (e) {
      update({ error: friendlyError(e), snap });
    }
    window.setTimeout(() => { if (started) void poll(); }, 400);
  },

  async cycleRepeat() {
    const snap = state.snap;
    if (!snap) return;
    const next = snap.repeat === 'off' ? 'list' : snap.repeat === 'list' ? 'track' : 'off';
    update({ snap: { ...snap, repeat: next as RepeatMode } });
    try {
      await bridge.invoke('nowplaying_control', { action: 'repeat', value: next });
    } catch (e) {
      update({ error: friendlyError(e), snap });
    }
    window.setTimeout(() => { if (started) void poll(); }, 400);
  },

  async refreshVolume() {
    try {
      const level = await bridge.invoke<number | null>('nowplaying_volume');
      update({ volume: typeof level === 'number' ? level : null });
    } catch {
      /* volume opsional: jangan ganggu pemutaran */
    }
  },

  setVolume(level: number) {
    const v = Math.max(0, Math.min(1, level));
    pendingVolume = v;
    update({ volume: v });
    window.clearTimeout(volumeTimer);
    volumeTimer = window.setTimeout(() => {
      bridge
        .invoke<number>('nowplaying_set_volume', { level: pendingVolume })
        .then((applied) => update({ volume: applied }))
        .catch((e) => update({ error: friendlyError(e) }));
    }, 90);
  },

  openUri(uri: string) {
    return bridge.invoke('nowplaying_open_uri', { uri }).catch((e) => {
      update({ error: friendlyError(e) });
    });
  },

  addPlaylist(name: string, link: string): boolean {
    const uri = parseSpotifyLink(link);
    if (!uri) return false;
    if (state.playlists.some((p) => p.uri === uri)) return true;
    const entry: PlaylistShortcut = { id: uri, name: name.trim() || 'Playlist', uri };
    update({ playlists: [entry, ...state.playlists].slice(0, 30) });
    persistPlaylists();
    return true;
  },

  removePlaylist(id: string) {
    update({ playlists: state.playlists.filter((p) => p.id !== id) });
    persistPlaylists();
  },

  clearHistory() {
    update({ history: [] });
    persistHistory();
    lastHistoryKey = '';
  },

  dismissError() {
    update({ error: null });
  },
};

/** Berlangganan state; komponen otomatis render ulang setiap polling. */
export function useNowPlaying(): MusicState {
  const [, force] = useState(0);
  useEffect(() => {
    const f = () => force((n) => n + 1);
    subs.add(f);
    return () => { subs.delete(f); };
  }, []);
  return state;
}

/** Detak 500 ms hanya saat lagu berjalan, untuk progress bar yang halus. */
export function useMusicTick(active: boolean) {
  const [, force] = useState(0);
  useEffect(() => {
    if (!active) return;
    const t = window.setInterval(() => force((n) => n + 1), 500);
    return () => window.clearInterval(t);
  }, [active]);
}

export function useVolumeOnce() {
  useEffect(() => {
    void music.refreshVolume();
  }, []);
}
