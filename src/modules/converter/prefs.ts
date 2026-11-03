// Preferensi folder output (murni; tanpa Tauri sehingga mudah diuji).

export type OutputPrefs = { mode: 'source' | 'custom'; dir: string };
export const PREFS_KEY = 'converter.prefs';
export const DEFAULT_PREFS: OutputPrefs = { mode: 'source', dir: '' };

export function parsePrefs(raw: string | null | undefined): OutputPrefs {
  if (!raw) return DEFAULT_PREFS;
  try {
    const o = JSON.parse(raw) as Partial<OutputPrefs> | null;
    const dir = typeof o?.dir === 'string' ? o.dir : '';
    // Mode "custom" tanpa folder tidak bermakna: kembali ke "sama dengan sumber".
    return { mode: o?.mode === 'custom' && dir ? 'custom' : 'source', dir };
  } catch {
    return DEFAULT_PREFS;
  }
}
