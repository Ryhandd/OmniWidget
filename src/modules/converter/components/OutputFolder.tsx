import { shortenMiddle } from '../paths';
import type { OutputPrefs } from '../prefs';

type Props = {
  prefs: OutputPrefs;
  locked: boolean;
  onPick: () => void;
  onUseSource: () => void;
  /** Sumber hasil paste/drop (disimpan sementara): "folder sumber" berarti folder Unduhan. */
  staged?: boolean;
};

export function OutputFolder({ prefs, locked, onPick, onUseSource, staged }: Props) {
  const custom = prefs.mode === 'custom';
  return (
    <div class="conv__out">
      <span class="muted conv__out-label">Simpan ke</span>
      <span class="conv__out-path" title={custom ? prefs.dir : undefined}>
        {custom ? shortenMiddle(prefs.dir, 34) : staged ? 'Folder Unduhan' : 'Folder yang sama dengan file sumber'}
      </span>
      <button type="button" class="btn btn--ghost" onClick={onPick} disabled={locked}>Ubah…</button>
      {custom && (
        <button type="button" class="btn btn--ghost" onClick={onUseSource} disabled={locked} title="Kembali ke folder file sumber">
          ↺
        </button>
      )}
    </div>
  );
}
