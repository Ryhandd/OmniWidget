import { shortenMiddle, splitPath } from '../paths';
import type { DropHandlers, Notice } from '../useFileIntake';

type Props = {
  filePath: string;
  source: string | null;
  /** Sedang diseret di atas jendela/kotak. */
  over: boolean;
  /** Job berjalan: kotak tidak menerima file baru. */
  locked: boolean;
  /** File sedang disalin ke penyimpanan sementara (drop/paste). */
  staging: boolean;
  /** File berasal dari paste/drop DOM (disimpan sementara), bukan lokasi aslinya. */
  staged: boolean;
  notice: Notice;
  dropProps: DropHandlers;
  onBrowse: () => void;
};

export function DropZone({ filePath, source, over, locked, staging, staged, notice, dropProps, onBrowse }: Props) {
  const name = filePath ? splitPath(filePath).base : '';
  const idle = locked || staging;
  return (
    <>
      <div
        class="conv__drop"
        role="group"
        aria-label="Area file: seret dan jatuhkan, tempel dengan Ctrl+V, atau pilih file"
        aria-busy={staging}
        data-over={over && !idle}
        data-busy={staging}
        {...dropProps}
      >
        {staging ? (
          <div>Menyiapkan file…</div>
        ) : filePath ? (
          <div class="conv__file" title={staged ? name : filePath}>
            <strong>{name}</strong>
            {source && <span class="conv__badge">{source.toUpperCase()}</span>}
            <div class="muted conv__path">{staged ? 'Dari tempel/seret · disimpan sementara' : shortenMiddle(filePath, 52)}</div>
          </div>
        ) : (
          <div>
            <div>Jatuhkan file di sini</div>
            <div class="muted conv__hint">atau tempel dengan Ctrl+V</div>
          </div>
        )}
        <div class="conv__drop-actions">
          <button type="button" class="btn btn--ghost" onClick={onBrowse} disabled={idle}>
            {filePath ? 'Ganti file…' : 'Pilih file…'}
          </button>
        </div>
      </div>
      {notice && (
        <div class={`conv__notice conv__notice--${notice.tone}`} role={notice.tone === 'err' ? 'alert' : 'status'}>
          {notice.text}
        </div>
      )}
    </>
  );
}
