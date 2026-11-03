import { isImageFormat } from '../paths';
import type { PdfPages, TargetInfo } from '../types';

type Props = {
  source: string | null;
  targets: TargetInfo[];
  target: string;
  pdfPages: PdfPages;
  locked: boolean;
  canConvert: boolean;
  active: boolean;
  cancelling: boolean;
  onTarget: (format: string) => void;
  onPages: (p: PdfPages) => void;
  onConvert: () => void;
  onCancel: () => void;
};

const REQUIRES_LABEL: Record<string, string> = { ffmpeg: 'FFmpeg', pandoc: 'Pandoc', pdfium: 'Pdfium' };

export function TargetPicker(p: Props) {
  const current = p.targets.find((t) => t.format === p.target);
  const showPages = p.source === 'pdf' && isImageFormat(p.target);

  return (
    <div class="conv__targets">
      <div class="conv__row">
        <select
          class="field"
          aria-label="Format tujuan"
          value={p.target}
          disabled={p.locked}
          onChange={(e) => p.onTarget((e.target as HTMLSelectElement).value)}
        >
          {p.targets.map((t) => (
            <option key={t.format} value={t.format} disabled={!t.available}>
              {t.format.toUpperCase()}
              {!t.available && t.requires ? ` — perlu ${REQUIRES_LABEL[t.requires] ?? t.requires}` : ''}
            </option>
          ))}
        </select>
        {p.active ? (
          <button type="button" class="btn btn--ghost" onClick={p.onCancel} disabled={p.cancelling}>
            {p.cancelling ? 'Membatalkan…' : 'Batal'}
          </button>
        ) : (
          <button type="button" class="btn" onClick={p.onConvert} disabled={!p.canConvert}>Konversi</button>
        )}
      </div>

      {showPages && (
        <label class="conv__row conv__opt">
          <span class="muted">Halaman</span>
          <select
            class="field"
            value={p.pdfPages}
            disabled={p.locked}
            onChange={(e) => p.onPages((e.target as HTMLSelectElement).value as PdfPages)}
          >
            <option value="all">Semua (folder bila &gt; 1 halaman)</option>
            <option value="first">Halaman pertama saja</option>
          </select>
        </label>
      )}

      {current && !current.available && current.requires && (
        <div class="conv__hint err">
          Butuh {REQUIRES_LABEL[current.requires] ?? current.requires}. Pasang dulu di tab Pengaturan → Komponen tambahan.
        </div>
      )}
    </div>
  );
}
