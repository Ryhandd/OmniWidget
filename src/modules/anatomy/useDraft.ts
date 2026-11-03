import { useCallback, useEffect, useRef, useState } from 'preact/hooks';
import { bridge } from '../../core/bridge';
import type { OutputFormat } from './compose';
import { DRAFT_KEY, emptyDraft, parseDraft, serializeDraft, type Draft } from './draft';
import type { Fields, LangId, PersonaId, SegmentId } from './segments';

export type SaveState = 'idle' | 'pending' | 'saved' | 'error';

/** Debounce simpan (TDD 4.10 aturan 6). */
const SAVE_DELAY_MS = 500;

/**
 * Draft prompt yang tersimpan di Rust (SQLite kv) lewat bridge.
 * - Debounce 500 ms saat mengetik.
 * - Disimpan segera saat modul di-unmount (pindah tab / ciut ke pill) atau jendela disembunyikan,
 *   sehingga tidak ada ketikan yang hilang. Tidak ada state yang bertahan di memori setelah unmount.
 */
export function useDraft() {
  const [draft, setDraft] = useState<Draft>(emptyDraft);
  const [ready, setReady] = useState(false);
  const [saveState, setSaveState] = useState<SaveState>('idle');

  const latest = useRef(draft); // nilai terbaru, dibaca oleh flush tanpa menunggu render
  const dirty = useRef(false);
  const timer = useRef<number | undefined>(undefined);
  const alive = useRef(true);

  const flush = useCallback(async () => {
    window.clearTimeout(timer.current);
    if (!dirty.current) return;
    dirty.current = false;
    try {
      await bridge.invoke('kv_set', { key: DRAFT_KEY, value: serializeDraft(latest.current) });
      if (alive.current) setSaveState(dirty.current ? 'pending' : 'saved');
    } catch {
      dirty.current = true; // coba lagi pada ketikan/flush berikutnya
      if (alive.current) setSaveState('error');
    }
  }, []);

  useEffect(() => {
    alive.current = true;
    bridge
      .invoke<string | null>('kv_get', { key: DRAFT_KEY })
      .then((raw) => {
        if (!alive.current) return;
        const loaded = parseDraft(raw);
        latest.current = loaded;
        setDraft(loaded);
      })
      .catch(() => { /* penyimpanan tidak tersedia (mis. di browser biasa): mulai dari draft kosong */ })
      .finally(() => { if (alive.current) setReady(true); });

    const onHide = () => { void flush(); };
    window.addEventListener('pagehide', onHide);
    return () => {
      window.removeEventListener('pagehide', onHide);
      alive.current = false;
      void flush();
    };
  }, [flush]);

  const update = useCallback(
    (fn: (d: Draft) => Draft) => {
      const next = fn(latest.current);
      latest.current = next;
      setDraft(next);
      dirty.current = true;
      setSaveState('pending');
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => { void flush(); }, SAVE_DELAY_MS);
    },
    [flush],
  );

  const setField = useCallback(
    (id: SegmentId, value: string) => update((d) => ({ ...d, fields: { ...d.fields, [id]: value } })),
    [update],
  );
  const setFields = useCallback((fields: Fields) => update((d) => ({ ...d, fields })), [update]);
  const setFormat = useCallback((format: OutputFormat) => update((d) => ({ ...d, format })), [update]);
  const setIdea = useCallback((idea: string) => update((d) => ({ ...d, idea })), [update]);
  const setPersona = useCallback((persona: PersonaId) => update((d) => ({ ...d, persona })), [update]);
  const setLang = useCallback((lang: LangId) => update((d) => ({ ...d, lang })), [update]);
  /** Draft terbaru tanpa menunggu render: aman dibaca setelah `await` (state di closure bisa basi). */
  const getDraft = useCallback(() => latest.current, []);

  return { draft, ready, saveState, setField, setFields, setFormat, setIdea, setPersona, setLang, getDraft };
}
