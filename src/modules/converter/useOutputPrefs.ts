import { open } from '@tauri-apps/plugin-dialog';
import { useCallback, useEffect, useState } from 'preact/hooks';
import { withHold } from '../../core/hold';
import { loadPrefs, savePrefs } from './api';
import { DEFAULT_PREFS, type OutputPrefs } from './prefs';

/** Folder output: "sama dengan sumber" (default) atau folder pilihan yang diingat antar sesi. */
export function useOutputPrefs() {
  const [prefs, setPrefs] = useState<OutputPrefs>(DEFAULT_PREFS);

  useEffect(() => {
    let alive = true;
    loadPrefs().then((p) => alive && setPrefs(p));
    return () => { alive = false; };
  }, []);

  const apply = useCallback((p: OutputPrefs) => { setPrefs(p); void savePrefs(p); }, []);

  const chooseSource = useCallback(() => apply({ ...prefs, mode: 'source' }), [prefs, apply]);

  const pickFolder = useCallback(async () => {
    // Dialog native: kursor meninggalkan widget, tahan island tetap terbuka.
    const dir = await withHold(() => open({ directory: true, multiple: false, defaultPath: prefs.dir || undefined, title: 'Pilih folder output' }));
    if (typeof dir === 'string' && dir) apply({ mode: 'custom', dir });
  }, [prefs.dir, apply]);

  /** Nilai untuk `output_dir`: null = folder sumber. */
  const outputDir = prefs.mode === 'custom' && prefs.dir ? prefs.dir : null;

  return { prefs, outputDir, chooseSource, pickFolder };
}
