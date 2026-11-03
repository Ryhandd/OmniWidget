import { useCallback, useEffect, useLayoutEffect, useRef } from 'preact/hooks';
import { MAX_FIELD_CHARS, type Segment } from '../segments';

type Props = {
  segment: Segment;
  value: string;
  disabled: boolean;
  onInput: (value: string) => void;
  /** Tombol "Salin Bagian". */
  onCopy: () => void;
  /** Ctrl/⌘+Enter di dalam kolom. */
  onSubmit: () => void;
};

// Kartu satu blok anatomi: judul + panduan + textarea yang tingginya mengikuti isi (auto-resize).
export function SegmentField({ segment, value, disabled, onInput, onCopy, onSubmit }: Props) {
  const id = `anat-${segment.id}`;
  const ref = useRef<HTMLTextAreaElement>(null);
  const nearLimit = value.length > MAX_FIELD_CHARS * 0.9;

  const fit = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = 'auto'; // ciutkan dulu agar scrollHeight menghitung isi sebenarnya
    el.style.height = `${el.scrollHeight + (el.offsetHeight - el.clientHeight)}px`; // + tebal border
  }, []);

  useLayoutEffect(fit, [value, fit]);
  useEffect(() => {
    // Lebar kartu berubah (mis. pindah antara 1 dan 2 kolom) mengubah jumlah baris terbungkus.
    window.addEventListener('resize', fit);
    return () => window.removeEventListener('resize', fit);
  }, [fit]);

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      // Escape pertama hanya melepas fokus; yang kedua baru menciutkan widget (handler di App).
      e.stopPropagation();
      (e.target as HTMLElement).blur();
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      onSubmit();
    }
  };

  return (
    <section class="anat__card" data-seg={segment.id}>
      <header class="anat__card-head">
        <span class="anat__dot" aria-hidden="true" />
        <label class="anat__name" for={id} title={segment.hint}>{segment.label}</label>
        <span class="anat__sub">{segment.subtitle}</span>
        <button
          type="button"
          class="anat__mini"
          onClick={onCopy}
          disabled={value.trim() === ''}
          title={`Salin bagian ${segment.label}`}
          aria-label={`Salin bagian ${segment.label}`}
        >
          Salin Bagian
        </button>
      </header>
      <p class="anat__help">{segment.hint}</p>
      <textarea
        id={id}
        ref={ref}
        class="field anat__text"
        rows={2}
        value={value}
        placeholder={segment.placeholder}
        maxLength={MAX_FIELD_CHARS}
        disabled={disabled}
        spellcheck={false}
        onInput={(e) => onInput((e.target as HTMLTextAreaElement).value)}
        onKeyDown={onKeyDown}
      />
      {nearLimit && (
        <div class="anat__count err" aria-live="polite">
          {value.length.toLocaleString('id-ID')} / {MAX_FIELD_CHARS.toLocaleString('id-ID')} karakter
        </div>
      )}
    </section>
  );
}
