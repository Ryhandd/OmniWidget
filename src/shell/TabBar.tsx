import { useEffect, useLayoutEffect, useRef, useState } from 'preact/hooks';
import { MODULES } from '../core/registry';
import { splitTabs } from '../core/tabs';

type Props = { active: string; onSelect: (id: string) => void };

// Dock segmented: ikon saja (label = tooltip/aria-label), sorotan aktif MENGGESER ke tab terpilih.
// Tab aktif selalu terlihat; sisanya masuk menu "Lainnya".
export function TabBar({ active, onSelect }: Props) {
  const { visible, overflow } = splitTabs(MODULES, active);
  const [more, setMore] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const [glow, setGlow] = useState<{ x: number; w: number; ready: boolean } | null>(null);

  // Posisi sorotan diukur dari tombol aktif (transform saja -> murah); ukur ulang bila lebar dock berubah.
  useLayoutEffect(() => {
    const measure = () => {
      const btn = list.current?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]');
      if (!btn) { setGlow(null); return; }
      setGlow((g) => (g && g.x === btn.offsetLeft && g.w === btn.offsetWidth ? g : { x: btn.offsetLeft, w: btn.offsetWidth, ready: !!g }));
    };
    measure();
    const ro = new ResizeObserver(measure);
    if (list.current) ro.observe(list.current);
    return () => ro.disconnect();
  }, [active, visible.map((m) => m.id).join()]);

  // Tutup menu More: klik di luar atau Escape (Escape tidak sampai ke handler "ciutkan" milik App).
  useEffect(() => {
    if (!more) return;
    const away = (e: PointerEvent) => !root.current?.contains(e.target as Node) && setMore(false);
    const esc = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { e.stopPropagation(); setMore(false); }
    };
    window.addEventListener('pointerdown', away);
    window.addEventListener('keydown', esc, true);
    return () => { window.removeEventListener('pointerdown', away); window.removeEventListener('keydown', esc, true); };
  }, [more]);

  // Navigasi roving: panah kiri/kanan memindahkan fokus antar tab.
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
    const tabs = [...(list.current?.querySelectorAll<HTMLButtonElement>('[role="tab"], [data-more]') ?? [])];
    const i = tabs.indexOf(document.activeElement as HTMLButtonElement);
    if (i < 0) return;
    tabs[(i + (e.key === 'ArrowRight' ? 1 : tabs.length - 1)) % tabs.length].focus();
    e.preventDefault();
  };

  return (
    <div ref={root} class="dockwrap">
      <div ref={list} class="dock" role="tablist" aria-label="Modul" onKeyDown={onKeyDown}>
        {glow && (
          <i
            class={`dock__glow${glow.ready ? ' is-ready' : ''}`}
            style={`width:${glow.w}px;transform:translateX(${glow.x}px)`}
            aria-hidden="true"
          />
        )}
        {visible.map((m, i) => (
          <button
            key={m.id}
            type="button"
            role="tab"
            class="dock__tab"
            aria-selected={m.id === active}
            aria-label={m.title}
            title={i < 9 ? `${m.title}  (Alt+${i + 1})` : m.title}
            tabIndex={m.id === active ? 0 : -1}
            onClick={() => onSelect(m.id)}
          >
            <span aria-hidden="true">{m.icon}</span>
          </button>
        ))}
        {overflow.length > 0 && (
          <button
            type="button"
            data-more
            class="dock__tab dock__tab--more"
            aria-label="Modul lainnya"
            aria-haspopup="menu"
            aria-expanded={more}
            title="Lainnya"
            onClick={() => setMore((v) => !v)}
          >
            <span aria-hidden="true">⋯</span>
          </button>
        )}
      </div>
      {more && (
        <div role="menu" class="dock__menu">
          {overflow.map((m) => (
            <button
              key={m.id}
              type="button"
              role="menuitem"
              class="dock__item"
              onClick={() => { setMore(false); onSelect(m.id); }}
            >
              <span aria-hidden="true">{m.icon}</span>
              <span>{m.title}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
