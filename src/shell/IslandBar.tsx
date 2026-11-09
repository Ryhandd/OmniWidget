import type { Ref } from 'preact';
import { useEffect, useState } from 'preact/hooks';
import { bridge } from '../core/bridge';
import { useIndicators } from '../core/indicators';
import { useNowPlaying } from '../core/nowplaying';
import { MODULES } from '../core/registry';

const clock = () => new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
const today = () => new Date().toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' });
const snap = () => bridge.invoke('window_snap').catch(() => {});

/** Jam hanya berdetak saat bar terlihat; timer berhenti ketika panel terbuka penuh. */
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

type Props = {
  /** Tab yang sudah dipilih (disimpan). */
  active: string;
  /** Tab yang sedang ditampilkan (bisa berbeda karena hover). */
  shown: string;
  /** false saat widget masih ciut: jam tetap berdetak, label tidak melebar. */
  live: boolean;
  measureRef: Ref<HTMLDivElement>;
  /** Hover island: tampilkan modul itu tanpa mengubah pilihan tersimpan. */
  onHover: (id: string) => void;
  /** Klik island: simpan pilihan lalu buka panel. */
  onSelect: (id: string) => void;
  /** Klik island jam saat widget ciut. */
  onOpen: () => void;
};

/**
 * Deretan island modul di atas layar. Saat ciut hanya ikon yang terlihat; mengarahkan kursor ke
 * salah satu island membuka panel modul tersebut. Klik menyimpan pilihan ke localStorage.
 */
export function IslandBar({ active, shown, live, measureRef, onHover, onSelect, onOpen }: Props) {
  const time = useClock(live);
  const items = useIndicators();
  const np = useNowPlaying();
  const playing = np.snap?.available && np.snap.title ? np.snap : null;

  const badge = (id: string) => {
    if (items.some((i) => i.tab === id || i.id === id)) return true;
    return id === 'music' && !!playing;
  };

  return (
    <div ref={measureRef} class="islebar" role="tablist" aria-label="Modul OmniWidget">
      <div
        class="isle isle--clock"
        data-tauri-drag-region
        title={today()}
        onDblClick={snap}
        onClick={onOpen}
      >
        <i class={`isle__dot${items.length || playing ? ' is-live' : ''}`} data-tauri-drag-region aria-hidden="true" />
        <span class="isle__time" data-tauri-drag-region>{time}</span>
        {playing && (
          <span
            class="isle__eq"
            data-playing={np.snap?.playing ? '' : undefined}
            data-tauri-drag-region
            aria-hidden="true"
          ><i /><i /><i /><i /></span>
        )}
      </div>

      {MODULES.map((m) => (
        <button
          key={m.id}
          type="button"
          role="tab"
          class="isle"
          aria-selected={m.id === active}
          aria-label={m.title}
          title={m.title}
          data-shown={m.id === shown ? '' : undefined}
          onPointerEnter={() => onHover(m.id)}
          onFocus={() => onHover(m.id)}
          onClick={() => onSelect(m.id)}
        >
          <span class="isle__icon" aria-hidden="true">{m.icon}</span>
          {badge(m.id) && <i class="isle__badge" aria-hidden="true" />}
        </button>
      ))}

    </div>
  );
}
