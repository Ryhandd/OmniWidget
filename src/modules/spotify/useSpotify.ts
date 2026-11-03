import { useCallback, useEffect, useReducer, useRef, useState } from 'preact/hooks';
import { spotifyApi } from './api';
import { toErr } from './errors';
import { initialState, reducer, type Action } from './state';
import type { Control } from './types';

/** Interval polling normal, dan saat Spotify membatasi (429) / jaringan bermasalah. */
const POLL_MS = 4000;
const BACKOFF_MS = 15000;
/** Spotify butuh sesaat untuk mencerminkan perintah; baca ulang setelah jeda ini. */
const SETTLE_MS = 600;

/**
 * Sumber kebenaran ada di Rust (token + sesi). Hook ini hanya view: saat modul di-unmount
 * (pindah tab / ciut ke pill / Deep Sleep) polling berhenti total, jadi tidak ada beban saat tak dilihat.
 */
export function useSpotify() {
  const [state, dispatch] = useReducer(reducer, initialState);
  const alive = useRef(true);
  const inflight = useRef(false);
  const pollTimer = useRef<number | undefined>(undefined);
  const settleTimer = useRef<number | undefined>(undefined);
  const delay = useRef(POLL_MS);

  const send = useCallback((a: Action) => {
    if (alive.current) dispatch(a);
  }, []);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      window.clearTimeout(pollTimer.current);
      window.clearTimeout(settleTimer.current);
    };
  }, []);

  useEffect(() => {
    spotifyApi.status().then((status) => send({ type: 'status', status })).catch((e) => send({ type: 'error', error: toErr(e) }));
  }, [send]);

  const refresh = useCallback(async () => {
    try {
      const player = await spotifyApi.player();
      delay.current = POLL_MS;
      send({ type: 'player', player, at: Date.now() });
    } catch (e) {
      delay.current = BACKOFF_MS;
      send({ type: 'error', error: toErr(e) });
    }
  }, [send]);

  // Polling hanya saat tersambung dan jendela terlihat. `inflight` mencegah dua rantai timer paralel.
  const polling = state.phase === 'player';
  useEffect(() => {
    if (!polling) return;
    let stop = false;
    const tick = async () => {
      if (inflight.current) return;
      inflight.current = true;
      window.clearTimeout(pollTimer.current);
      try {
        if (!document.hidden) await refresh();
      } finally {
        inflight.current = false;
      }
      if (!stop) pollTimer.current = window.setTimeout(tick, delay.current);
    };
    void tick();
    const onVisible = () => { if (!document.hidden) void tick(); };
    document.addEventListener('visibilitychange', onVisible);
    return () => {
      stop = true;
      window.clearTimeout(pollTimer.current);
      document.removeEventListener('visibilitychange', onVisible);
    };
  }, [polling, refresh]);

  const control = useCallback(async (c: Control) => {
    send({ type: 'optimistic', control: c, at: Date.now() });
    try {
      await spotifyApi.control(c);
      window.clearTimeout(settleTimer.current);
      settleTimer.current = window.setTimeout(() => void refresh(), SETTLE_MS);
    } catch (e) {
      send({ type: 'error', error: toErr(e) });
      void refresh(); // batalkan efek optimistis dengan data asli
    }
  }, [send, refresh]);

  const loadDevices = useCallback(async () => {
    try {
      send({ type: 'devices', devices: await spotifyApi.devices() });
    } catch (e) {
      send({ type: 'error', error: toErr(e) });
    }
  }, [send]);

  const saveClientId = useCallback(async (id: string) => {
    try {
      send({ type: 'status', status: await spotifyApi.setClientId(id) });
      send({ type: 'clear-error' });
    } catch (e) {
      send({ type: 'error', error: toErr(e) });
    }
  }, [send]);

  const login = useCallback(async () => {
    send({ type: 'login-start' });
    try {
      send({ type: 'status', status: await spotifyApi.login() });
    } catch (e) {
      const err = toErr(e);
      if (err.code !== 'cancelled') send({ type: 'error', error: err });
    } finally {
      send({ type: 'login-end' });
    }
  }, [send]);

  const cancelLogin = useCallback(() => { spotifyApi.cancelLogin().catch(() => {}); }, []);

  const logout = useCallback(async () => {
    try {
      send({ type: 'status', status: await spotifyApi.logout() });
    } catch (e) {
      send({ type: 'error', error: toErr(e) });
    }
  }, [send]);

  const dismissError = useCallback(() => send({ type: 'clear-error' }), [send]);

  return { state, actions: { control, loadDevices, saveClientId, login, cancelLogin, logout, dismissError } };
}

/** Jam lokal untuk interpolasi progres; hanya berdetak saat `active` (musik sedang diputar). */
export function useNow(active: boolean, everyMs = 500): number {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!active) return;
    setNow(Date.now());
    const id = window.setInterval(() => setNow(Date.now()), everyMs);
    return () => window.clearInterval(id);
  }, [active, everyMs]);
  return now;
}

/** Sampul album (data URI dari Rust). Sampul lama tetap tampil saat yang baru dimuat: tanpa kedip. */
export function useCover(url: string | null | undefined): string | null {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    if (!url) { setSrc(null); return; }
    let cancelled = false;
    spotifyApi.cover(url).then((d) => { if (!cancelled) setSrc(d); }).catch(() => { if (!cancelled) setSrc(null); });
    return () => { cancelled = true; };
  }, [url]);
  return src;
}
