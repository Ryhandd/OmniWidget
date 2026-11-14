import { FormatSelect, type SelectOption } from './FormatSelect';
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

const KIND: Record<string, string> = {
  png: 'Gambar', jpg: 'Gambar', webp: 'Gambar', bmp: 'Gambar', gif: 'Gambar', ico: 'Gambar',
  docx: 'Dokumen', md: 'Dokumen', html: 'Dokumen', txt: 'Teks', pdf: 'PDF',
  mp3: 'Audio', wav: 'Audio', flac: 'Audio', ogg: 'Audio', m4a: 'Audio', opus: 'Audio',
};

const kindOf = (format: string) => KIND[format] ?? 'Berkas';

export function TargetPicker(p: Props) {
  const current = p.targets.find((t) => t.format === p.target);
  const showPages = p.source === 'pdf' && isImageFormat(p.target);
  const options: SelectOption[] = p.targets.map((t) => ({
    value: t.format,
    label: t.format.toUpperCase(),
    hint: kindOf(t.format),
    disabled: !t.available,
  }));

  return (
    <div class="conv__targets">
      <div class="conv__row">
        <FormatSelect
          value={p.target}
          options={options}
          label="Format tujuan"
          disabled={p.locked}
          onChange={p.onTarget}
        />
        {p.active ? (
          <button type="button" class="btn btn--ghost conv__go" onClick={p.onCancel} disabled={p.cancelling}>
            {p.cancelling ? 'Membatalkan…' : 'Batal'}
          </button>
        ) : (
          <button type="button" class="btn conv__go" onClick={p.onConvert} disabled={!p.canConvert}>
            Konversi
          </button>
        )}
      </div>

      {showPages && (
        <div class="conv__opt">
          <span class="conv__opt-label">Halaman</span>
          <div class="seg" role="group" aria-label="Halaman PDF yang diekspor">
            <button
              type="button"
              class="seg__btn"
              aria-pressed={p.pdfPages === 'all'}
              disabled={p.locked}
              onClick={() => p.onPages('all')}
            >
              Semua
            </button>
            <button
              type="button"
              class="seg__btn"
              aria-pressed={p.pdfPages === 'first'}
              disabled={p.locked}
              onClick={() => p.onPages('first')}
            >
              Pertama
            </button>
          </div>
        </div>
      )}

      {current && !current.available && (
        <div class="conv__hint err">Format ini tidak tersedia di perangkat ini.</div>
      )}

      {current?.available && p.source === 'docx' && p.target === 'pdf' && current.engine === 'doc' && (
        <div class="conv__hint err">
          LibreOffice tidak terdeteksi: PDF hanya perkiraan, tabel, gambar, dan tata letak asli tidak ikut.
          Pasang LibreOffice agar hasilnya sama dengan DOCX.
        </div>
      )}

      {current?.available && p.source === 'pdf' && p.target === 'docx' && (
        <div class="conv__hint muted">
          Teks, gaya, jarak, dan gambar dipertahankan sebagai DOCX yang bisa diedit. Tabel dijaga posisinya, bukan
          dibuat ulang sebagai tabel.
        </div>
      )}

      {showPages && p.pdfPages === 'all' && (
        <div class="conv__hint muted">Lebih dari satu halaman disimpan sebagai folder gambar.</div>
      )}
    </div>
  );
}
