import { useEffect, useState } from 'preact/hooks';
import { fmtBytes, relTime } from '../state';
import type { ThumbCache } from '../thumbCache';
import type { ClipItem } from '../types';
import { useThumb } from '../useClipboard';
import { IconExpand, IconImage, IconPin, IconText, IconTrash } from './icons';

type Props = {
  item: ClipItem;
  now: number;
  thumbs: ThumbCache;
  onPaste: (id: string) => void;
  onPin: (id: string, pinned: boolean) => void;
  onRemove: (id: string) => void;
  loadText: (id: string) => Promise<string>;
};

/**
 * Satu entri. Klik / Enter = tempel kembali (tulis ke clipboard sistem). Teks penuh hanya diminta saat dibuka
 * dan dibuang saat ditutup, jadi RAM UI tidak menampung 20 x 64 KB.
 */
export function ClipRow({ item, now, thumbs, onPaste, onPin, onRemove, loadText }: Props) {
  const isImage = item.kind === 'image';
  const thumb = useThumb(thumbs, item.id, isImage);
  const [open, setOpen] = useState(false);
  const [full, setFull] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  // Muat teks penuh saat dibuka; lepaskan saat ditutup atau item berganti.
  useEffect(() => {
    if (!open || isImage) { setFull(null); setFailed(false); return; }
    let cancelled = false;
    loadText(item.id).then((t) => { if (!cancelled) setFull(t); }).catch(() => { if (!cancelled) setFailed(true); });
    return () => { cancelled = true; setFull(null); };
  }, [open, isImage, item.id, loadText]);

  // Item berganti (ring bergeser) di komponen yang sama: tutup detailnya.
  useEffect(() => { setOpen(false); }, [item.id]);

  const meta = [
    relTime(item.ts, now),
    isImage && item.bytes != null ? fmtBytes(item.bytes) : null,
    item.truncated ? 'dipotong 64 KB' : null,
  ].filter(Boolean).join(' · ');

  const onKeyDown = (e: KeyboardEvent) => {
    // Pintasan hanya saat fokus di tombol utama baris: Spasi/Enter pada tombol aksi harus tetap menekan tombolnya.
    if (!(e.target as HTMLElement).hasAttribute('data-row')) return;
    if (e.key === 'Delete') { e.preventDefault(); onRemove(item.id); }
    else if (e.key === 'p' || e.key === 'P') { e.preventDefault(); onPin(item.id, !item.pinned); }
    else if ((e.key === ' ' || e.key === 'ArrowRight') && !isImage) { e.preventDefault(); setOpen(true); }
    else if (e.key === 'ArrowLeft') { e.preventDefault(); setOpen(false); }
  };

  return (
    <li class="clip__row" data-pinned={item.pinned} data-open={open} onKeyDown={onKeyDown}>
      <button
        type="button"
        class="clip__main"
        data-row
        title="Tempel kembali (Enter) · Hapus (Del) · Sematkan (P)"
        onClick={() => onPaste(item.id)}
      >
        <span class="clip__thumb" aria-hidden="true">
          {isImage ? (
            thumb ? <img src={thumb} alt="" width={40} height={40} draggable={false} /> : <IconImage />
          ) : (
            <IconText />
          )}
        </span>
        <span class="clip__body">
          <span class="clip__preview" data-image={isImage}>{item.preview || '(kosong)'}</span>
          <span class="clip__meta">{meta}</span>
        </span>
      </button>

      <span class="clip__actions">
        {!isImage && (
          <button
            type="button"
            class="clip__icon"
            aria-label={open ? 'Tutup teks penuh' : 'Lihat teks penuh'}
            aria-expanded={open}
            title={open ? 'Tutup' : 'Lihat penuh'}
            onClick={() => setOpen((v) => !v)}
          >
            <IconExpand open={open} />
          </button>
        )}
        <button
          type="button"
          class="clip__icon"
          data-on={item.pinned}
          aria-pressed={item.pinned}
          aria-label={item.pinned ? 'Lepas sematan' : 'Sematkan'}
          title={item.pinned ? 'Lepas sematan' : 'Sematkan'}
          onClick={() => onPin(item.id, !item.pinned)}
        >
          <IconPin filled={item.pinned} />
        </button>
        <button type="button" class="clip__icon clip__icon--danger" aria-label="Hapus" title="Hapus" onClick={() => onRemove(item.id)}>
          <IconTrash />
        </button>
      </span>

      {open && !isImage && (
        <pre class="clip__full" aria-live="polite">
          {failed ? 'Gagal memuat teks penuh.' : full ?? 'Memuat…'}
        </pre>
      )}
    </li>
  );
}
