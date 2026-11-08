import { useEffect, useRef, useState } from 'preact/hooks';

export type SelectOption = {
  value: string;
  label: string;
  hint?: string;
  disabled?: boolean;
};

type Props = {
  value: string;
  options: SelectOption[];
  label: string;
  disabled?: boolean;
  onChange: (value: string) => void;
};

/**
 * Dropdown format dengan panel bergaya tema aplikasi.
 *
 * `select` bawaan WebView memunculkan daftar putih bawaan OS yang tidak mengikuti tema.
 * Komponen ini memakai pola combobox/listbox: keyboard (panah, Home/End, Enter, Escape),
 * klik di luar untuk menutup, dan panel otomatis membuka ke atas bila ruang bawah sempit.
 */
export function FormatSelect({ value, options, label, disabled, onChange }: Props) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [dropUp, setDropUp] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const idRef = useRef(`fsel-${Math.random().toString(36).slice(2, 8)}`);
  const id = idRef.current;

  const selectedIndex = options.findIndex((o) => o.value === value);
  const selected = options[selectedIndex >= 0 ? selectedIndex : 0];

  useEffect(() => {
    if (!open) return;
    const onPointer = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', onPointer);
    return () => document.removeEventListener('pointerdown', onPointer);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const rect = trigger.current?.getBoundingClientRect();
    if (!rect) return;
    const below = window.innerHeight - rect.bottom;
    setDropUp(below < 248 && rect.top > below);
  }, [open]);

  const step = (from: number, direction: 1 | -1) => {
    let next = from;
    for (let i = 0; i < options.length; i++) {
      next = (next + direction + options.length) % options.length;
      if (!options[next].disabled) return next;
    }
    return from;
  };

  const edge = (end: boolean) => {
    const start = end ? options.length - 1 : 0;
    const direction = end ? -1 : 1;
    for (let i = start; i >= 0 && i < options.length; i += direction) {
      if (!options[i].disabled) return i;
    }
    return active;
  };

  const openMenu = () => {
    setActive(selectedIndex >= 0 && !options[selectedIndex].disabled ? selectedIndex : step(0, 1));
    setOpen(true);
  };

  const choose = (option: SelectOption | undefined) => {
    if (!option || option.disabled) return;
    onChange(option.value);
    setOpen(false);
    trigger.current?.focus();
  };

  return (
    <div class="fsel" ref={root} data-open={open} data-up={dropUp}>
      <button
        type="button"
        class="fsel__trigger"
        ref={trigger}
        role="combobox"
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={`${id}-menu`}
        aria-activedescendant={open ? `${id}-opt-${active}` : undefined}
        disabled={disabled}
        onClick={() => (open ? setOpen(false) : openMenu())}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
            event.preventDefault();
            const direction = event.key === 'ArrowDown' ? 1 : -1;
            if (open) setActive(step(active, direction));
            else openMenu();
          } else if (event.key === 'Home' || event.key === 'End') {
            if (!open) return;
            event.preventDefault();
            setActive(edge(event.key === 'End'));
          } else if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            if (open) choose(options[active]);
            else openMenu();
          } else if (event.key === 'Escape' && open) {
            event.preventDefault();
            setOpen(false);
          } else if (event.key === 'Tab') {
            setOpen(false);
          }
        }}
      >
        <span class="fsel__code">{selected?.label ?? value.toUpperCase()}</span>
        {selected?.hint && <span class="fsel__hint">{selected.hint}</span>}
        <svg class="fsel__chev" viewBox="0 0 16 16" aria-hidden="true">
          <path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </button>

      {open && (
        <ul class="fsel__menu" id={`${id}-menu`} role="listbox" aria-label={label}>
          {options.map((option, i) => (
            <li
              key={option.value}
              id={`${id}-opt-${i}`}
              role="option"
              aria-selected={option.value === value}
              aria-disabled={option.disabled || undefined}
              class="fsel__opt"
              data-active={i === active}
              data-selected={option.value === value}
              data-disabled={option.disabled}
              onMouseDown={(event) => event.preventDefault()}
              onMouseEnter={() => !option.disabled && setActive(i)}
              onClick={() => choose(option)}
            >
              <span class="fsel__code">{option.label}</span>
              <span class="fsel__opt-hint">
                {option.hint}
                {option.disabled ? ' · tidak tersedia' : ''}
              </span>
              {option.value === value && (
                <svg class="fsel__check" viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M3.5 8.5l3 3 6-7" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
