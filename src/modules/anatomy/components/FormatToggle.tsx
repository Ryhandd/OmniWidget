import { FORMATS, type OutputFormat } from '../compose';

type Props = { value: OutputFormat; onChange: (f: OutputFormat) => void; disabled?: boolean };

// Kelompok radio bergaya segmented-control; panah kiri/kanan berpindah antar pilihan.
export function FormatToggle({ value, onChange, disabled }: Props) {
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
    const i = FORMATS.findIndex((f) => f.id === value);
    const next = FORMATS[(i + (e.key === 'ArrowRight' ? 1 : FORMATS.length - 1)) % FORMATS.length];
    onChange(next.id);
    e.preventDefault();
    ((e.currentTarget as HTMLElement).querySelector(`[data-fmt="${next.id}"]`) as HTMLElement | null)?.focus();
  };

  return (
    <div class="anat__fmt" role="radiogroup" aria-label="Format keluaran" onKeyDown={onKeyDown}>
      {FORMATS.map((f) => (
        <button
          key={f.id}
          type="button"
          role="radio"
          data-fmt={f.id}
          class="anat__fmt-btn"
          aria-checked={f.id === value}
          tabIndex={f.id === value ? 0 : -1}
          title={f.title}
          disabled={disabled}
          onClick={() => onChange(f.id)}
        >
          {f.label}
        </button>
      ))}
    </div>
  );
}
