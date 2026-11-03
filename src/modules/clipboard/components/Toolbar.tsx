import { useEffect, useRef, useState } from 'preact/hooks';
import type { ClipSettings } from '../types';
import { IconGear, IconPause, IconRec } from './icons';

type Props = {
  query: string;
  settings: ClipSettings;
  count: number;
  usage: string;
  settingsOpen: boolean;
  onQuery: (q: string) => void;
  onTogglePause: () => void;
  onToggleSettings: () => void;
  onClear: (keepPinned: boolean) => void;
};

export function Toolbar({ query, settings, count, usage, settingsOpen, onQuery, onTogglePause, onToggleSettings, onClear }: Props) {
  // Konfirmasi dua langkah inline untuk aksi destruktif; batal otomatis bila fokus berpindah.
  const [confirm, setConfirm] = useState(false);
  const box = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    if (!confirm) return;
    const away = (e: PointerEvent) => !box.current?.contains(e.target as Node) && setConfirm(false);
    const esc = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { e.stopPropagation(); setConfirm(false); }
    };
    window.addEventListener('pointerdown', away);
    window.addEventListener('keydown', esc, true);
    return () => { window.removeEventListener('pointerdown', away); window.removeEventListener('keydown', esc, true); };
  }, [confirm]);

  return (
    <div class="clip__bar">
      <input
        class="field clip__search"
        type="search"
        placeholder="Cari di riwayat…"
        aria-label="Cari di riwayat clipboard"
        value={query}
        onInput={(e) => onQuery((e.currentTarget as HTMLInputElement).value)}
        onKeyDown={(e) => { if (e.key === 'Escape' && query) { e.stopPropagation(); onQuery(''); } }}
      />
      <button
        type="button"
        class="clip__icon"
        data-on={settings.paused}
        aria-pressed={settings.paused}
        aria-label={settings.paused ? 'Lanjutkan perekaman' : 'Jeda perekaman'}
        title={settings.paused ? 'Perekaman dijeda: klik untuk melanjutkan' : 'Jeda perekaman'}
        onClick={onTogglePause}
      >
        {settings.paused ? <IconRec /> : <IconPause />}
      </button>
      <button
        type="button"
        class="clip__icon"
        data-on={settingsOpen}
        aria-expanded={settingsOpen}
        aria-label="Pengaturan clipboard"
        title="Pengaturan"
        onClick={onToggleSettings}
      >
        <IconGear />
      </button>

      <span class="clip__sum muted" aria-live="polite">{usage}</span>

      <span ref={box} class="clip__clear">
        {confirm ? (
          <>
            <button type="button" class="btn btn--ghost clip__small" onClick={() => { setConfirm(false); onClear(true); }}>Kecuali sematan</button>
            <button type="button" class="btn clip__small" onClick={() => { setConfirm(false); onClear(false); }}>Semua</button>
          </>
        ) : (
          <button type="button" class="btn btn--ghost clip__small" disabled={count === 0} onClick={() => setConfirm(true)}>
            Bersihkan
          </button>
        )}
      </span>
    </div>
  );
}
