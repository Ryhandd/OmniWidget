import type { Ref } from 'preact';
import { useEffect, useState } from 'preact/hooks';
import { bridge } from '../core/bridge';
import { useIndicators } from '../core/indicators';

const clock = () => new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
const today = () => new Date().toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' });
const snap = () => bridge.invoke('window_snap').catch(() => {});

/** Jam yang berdetak tepat di pergantian menit, dan hanya saat `on` (tanpa timer ketika tidak terlihat). */
function useClock(on: boolean) {
  const [time, setTime] = useState(clock());
  useEffect(() => {
    if (!on) return;
    let t: number;
    const tick = () => {
      setTime(clock());
      t = window.setTimeout(tick, 60_000 - (Date.now() % 60_000) + 50);
    };
    tick();
    return () => clearTimeout(t);
  }, [on]);
  return time;
}

type PillProps = {
  /** false saat panel terbuka penuh: jam berhenti berdetak. */
  live: boolean;
  measureRef: Ref<HTMLDivElement>;
  /** Klik chip live-activity: buka panel ke modul terkait. */
  onChip: (tab?: string) => void;
};

// Atribut drag-region hanya berlaku pada elemen itu sendiri (bukan anak), jadi dipasang berulang.
export function Pill({ live, measureRef, onChip }: PillProps) {
  const time = useClock(live);
  const items = useIndicators();

  return (
    <div ref={measureRef} class="pill" data-tauri-drag-region onDblClick={snap} title={today()}>
      <i class={`pill__dot${items.length ? ' is-live' : ''}`} data-tauri-drag-region aria-hidden="true" />
      <span class="pill__time" data-tauri-drag-region>{time}</span>
      {items.map((i) => (
        <button
          key={i.id}
          type="button"
          class="chip"
          data-tone={i.tone ?? 'accent'}
          title={i.label}
          onPointerDown={(e) => e.stopPropagation()}
          onClick={(e) => { e.stopPropagation(); onChip(i.tab); }}
        >
          {i.icon && <span aria-hidden="true">{i.icon}</span>}
          <span class="chip__label">{i.label}</span>
        </button>
      ))}
    </div>
  );
}

type HeaderProps = {
  icon: string;
  title: string;
  pinned: boolean;
  onPin: () => void;
  onCollapse: () => void;
};

export function Header({ icon, title, pinned, onPin, onCollapse }: HeaderProps) {
  return (
    <div class="ihead" data-tauri-drag-region onDblClick={snap}>
      <span class="ihead__icon" data-tauri-drag-region aria-hidden="true">{icon}</span>
      <span class="ihead__title" data-tauri-drag-region>{title}</span>
      <button
        type="button"
        class="ibtn"
        aria-pressed={pinned}
        aria-label="Sematkan panel"
        title={pinned ? 'Lepas sematan (ciut otomatis lagi)' : 'Sematkan (tetap terbuka)'}
        onClick={onPin}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M9.5 2.5l4 4-2 1-2.5 2.5.5 2.5-1 1-6-6 1-1 2.5.5L8.5 4.5z" />
          <path d="M5 11l-2.5 2.5" />
        </svg>
      </button>
      <button type="button" class="ibtn" aria-label="Ciutkan" title="Ciutkan (Esc)" onClick={onCollapse}>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3.5 10l4.5-4.5 4.5 4.5" /></svg>
      </button>
    </div>
  );
}
