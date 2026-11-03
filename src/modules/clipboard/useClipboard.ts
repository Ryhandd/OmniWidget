import { useCallback, useEffect, useReducer, useRef, useState } from 'preact/hooks';
import { clipApi, onClipChanged } from './api';
import { clipErr } from './errors';
import { initialState, reducer } from './state';
import { ThumbCache } from './thumbCache';
import type { ClipSettingsPatch } from './types';

export type Notice = { id: number; msg: string; tone: 'ok' | 'err' };

/**
 * Sumber kebenaran ada di Rust (ring, privasi, enkripsi, watcher). Hook ini hanya view:
 * tanpa polling (Rust mengirim "clipboard://changed"), dan saat modul di-unmount semua langganan,
 * timer, dan cache thumbnail dilepas (TDD 5.4 L1).
 */
export function useClipboard() {
  const [state, dispatch] = useReducer(reducer, initialState);
  const [notice, setNotice] = useState<Notice | null>(null);

  const alive = useRef(true);
  const noticeTimer = useRef<number | undefined>(undefined);
  const noticeSeq = useRef(0);
  /** Naik tiap mutasi lokal: snapshot yang berangkat sebelum mutasi dibuang agar tidak menimpa efek optimistis. */
  const gen = useRef(0);
  const inflight = useRef(false);
  const queued = useRef(false);
  const thumbs = useRef<ThumbCache | null>(null);
  if (!thumbs.current) thumbs.current = new ThumbCache(clipApi.thumb);

  const say = useCallback((msg: string, tone: Notice['tone'] = 'ok') => {
    if (!alive.current) return;
    window.clearTimeout(noticeTimer.current);
    setNotice({ id: ++noticeSeq.current, msg, tone });
    noticeTimer.current = window.setTimeout(() => setNotice(null), tone === 'err' ? 4200 : 1600);
  }, []);

  /** Satu permintaan pada satu waktu; event yang datang saat berjalan digabung jadi satu pengulangan. */
  const refresh = useCallback(async () => {
    if (inflight.current) { queued.current = true; return; }
    inflight.current = true;
    try {
      do {
        queued.current = false;
        const g = gen.current;
        try {
          const snap = await clipApi.snapshot();
          if (!alive.current) return;
          if (g !== gen.current) { queued.current = true; continue; } // basi: ulangi
          thumbs.current!.retain(snap.items.map((i) => i.id));
          dispatch({ type: 'snapshot', snap });
        } catch (e) {
          if (alive.current) dispatch({ type: 'error', message: clipErr(e) });
        }
      } while (queued.current && alive.current);
    } finally {
      inflight.current = false;
    }
  }, []);

  useEffect(() => {
    alive.current = true;
    void refresh();
    const off = onClipChanged(() => void refresh());
    const onVisible = () => { if (!document.hidden) void refresh(); };
    document.addEventListener('visibilitychange', onVisible);
    const cache = thumbs.current!;
    return () => {
      alive.current = false;
      off();
      document.removeEventListener('visibilitychange', onVisible);
      window.clearTimeout(noticeTimer.current);
      cache.clear();
    };
  }, [refresh]);

  /** Bungkus mutasi: efek optimistis -> panggil Rust -> selalu rekonsiliasi dengan snapshot asli. */
  const mutate = useCallback(async (optimistic: (() => void) | null, run: () => Promise<unknown>, ok?: string) => {
    gen.current++;
    optimistic?.();
    try {
      await run();
      if (ok) say(ok);
    } catch (e) {
      say(clipErr(e), 'err');
    } finally {
      gen.current++;
      void refresh();
    }
  }, [refresh, say]);

  const paste = useCallback(async (id: string) => {
    // Tanpa optimistis: Rust memindahkan item ke depan ring, snapshot berikutnya menampilkannya.
    await mutate(null, () => clipApi.paste(id), 'Tersalin');
  }, [mutate]);

  const pin = useCallback((id: string, pinned: boolean) =>
    mutate(() => dispatch({ type: 'optimistic-pin', id, pinned }), () => clipApi.pin(id, pinned)), [mutate]);

  const remove = useCallback((id: string) =>
    mutate(() => { thumbs.current!.drop(id); dispatch({ type: 'optimistic-remove', id }); }, () => clipApi.remove(id)), [mutate]);

  const clear = useCallback((keepPinned: boolean) =>
    mutate(() => {
      if (!keepPinned) thumbs.current!.clear();
      dispatch({ type: 'optimistic-clear', keepPinned });
    }, () => clipApi.clear(keepPinned), 'Riwayat dibersihkan'), [mutate]);

  const setSettings = useCallback(async (patch: ClipSettingsPatch) => {
    gen.current++;
    try {
      dispatch({ type: 'settings', settings: await clipApi.setSettings(patch) });
    } catch (e) {
      say(clipErr(e), 'err');
    } finally {
      gen.current++;
      void refresh();
    }
  }, [refresh, say]);

  /** Teks penuh sesuai permintaan (tidak disimpan di state global: pemanggil yang memegang & membuangnya). */
  const loadText = useCallback((id: string) => clipApi.text(id), []);

  const setQuery = useCallback((query: string) => dispatch({ type: 'query', query }), []);
  const dismissError = useCallback(() => dispatch({ type: 'clear-error' }), []);

  return {
    state,
    notice,
    thumbs: thumbs.current,
    actions: { paste, pin, remove, clear, setSettings, loadText, setQuery, dismissError },
  };
}

/** Jam kasar untuk label "x mnt lalu": berdetak tiap menit, hanya saat jendela terlihat. */
export function useMinuteClock(): number {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const id = window.setInterval(() => { if (!document.hidden) setNow(Date.now()); }, 60_000);
    return () => window.clearInterval(id);
  }, []);
  return now;
}

/** Thumbnail satu item: tampil seketika bila sudah di cache; thumbnail lama tidak dikosongkan saat id berganti. */
export function useThumb(cache: ThumbCache, id: string, enabled: boolean): string | null {
  const [src, setSrc] = useState<string | null>(() => (enabled ? cache.peek(id) : null));
  useEffect(() => {
    if (!enabled) { setSrc(null); return; }
    const hit = cache.peek(id);
    if (hit) { setSrc(hit); return; }
    let cancelled = false;
    cache.get(id).then((u) => { if (!cancelled) setSrc(u); }).catch(() => { if (!cancelled) setSrc(null); });
    return () => { cancelled = true; };
  }, [cache, id, enabled]);
  return src;
}
