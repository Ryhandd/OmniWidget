import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'preact/hooks';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { bridge } from '../core/bridge';
import { islandHold } from '../core/hold';
import { startNowPlaying } from '../core/nowplaying';
import { MODULES } from '../core/registry';
import { Header } from './Handle';
import { IslandBar } from './IslandBar';
import { ModuleHost } from './ModuleHost';
import { MiniPlayer } from './MiniPlayer';

/**
 * Shell bergaya Dynamic Island: baris island modul di top-center. Saat ciut hanya deretan island
 * yang terlihat; hover salah satu island membuka panel modul itu, klik menyimpan pilihannya.
 * Urutan selalu: jendela membesar dulu -> CSS memekarkan clip-path; saat ciut,
 * CSS menyusut dulu -> baru jendela mengecil. Jadi tidak ada area transparan yang tertinggal.
 *
 *   pill --hover 160ms / klik / hotkey / drag file--> opening -> open
 *   open --kursor keluar 420ms / blur / Esc--> closing -> pill
 */
type Phase = 'pill' | 'opening' | 'open' | 'closing';

const HOVER_OPEN_MS = 160; // hover-intent: lewat sekilas tidak membuka
const LEAVE_CLOSE_MS = 420;
const BLUR_CLOSE_MS = 160;
const SAFETY_MS = 1500; // pengaman bila pointerleave terlewat
const PILL_W = { min: 160, max: 900 };
const morphMs = () => (matchMedia('(prefers-reduced-motion: reduce)').matches ? 0 : 300);

const PIN_KEY = 'omniwidget.pin';
const TAB_KEY = 'omniwidget.tab';
const read = (k: string) => { try { return localStorage.getItem(k); } catch { return null; } };
const write = (k: string, v: string) => { try { localStorage.setItem(k, v); } catch { /* abaikan */ } };

/** Sedang mengetik di dalam widget: jangan ciut hanya karena mouse bergeser keluar. */
function isEditing() {
  const el = document.activeElement as HTMLElement | null;
  return !!el && (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName));
}

type T = { current: number | undefined };
const clear = (t: T) => { window.clearTimeout(t.current); t.current = undefined; };

export function App() {
  const [phase, setPhaseState] = useState<Phase>('pill');
  const phaseRef = useRef<Phase>('pill');
  const setPhase = (p: Phase) => {
    phaseRef.current = p;
    document.documentElement.dataset.island = p;
    setPhaseState(p);
  };

  const [tab, setTabState] = useState(() => {
    const t = read(TAB_KEY);
    return MODULES.some((m) => m.id === t) ? t! : MODULES[0].id;
  });
  const setTab = useCallback((id: string) => { setTabState(id); write(TAB_KEY, id); }, []);

  /** Modul yang sedang dipratinjau lewat hover (belum disimpan). */
  const [preview, setPreview] = useState<string | null>(null);

  const [pinned, setPinned] = useState(read(PIN_KEY) === '1');
  const pinnedRef = useRef(pinned);
  pinnedRef.current = pinned;

  const [pw, setPw] = useState(470);
  const pwRef = useRef(470);
  const pillRef = useRef<HTMLDivElement>(null);

  const gen = useRef(0); // generasi transisi: transisi baru membatalkan yang lama
  const inside = useRef(false);
  const suppress = useRef(false); // setelah pointerdown (drag) di pill: jangan buka sampai kursor keluar
  const summoned = useRef(false); // dibuka via hotkey/tray: tahan sampai blur/Esc
  const tOpen: T = useRef<number>();
  const tClose: T = useRef<number>();
  const tSafety: T = useRef<number>();

  const open = useCallback(async () => {
    clear(tOpen); clear(tClose);
    const p = phaseRef.current;
    if (p === 'open' || p === 'opening') return;
    const g = ++gen.current;
    if (p === 'closing') {
      // Kursor kembali saat sedang menyusut: mekar lagi dari posisi sekarang (jendela masih besar).
      setPhase('open');
      bridge.invoke('window_set_mode', { mode: 'expanded' }).catch(() => {});
      return;
    }
    await bridge.invoke('window_set_mode', { mode: 'expanded' }).catch(() => {});
    if (g !== gen.current) return;
    setPhase('opening');
    requestAnimationFrame(() => requestAnimationFrame(() => { if (g === gen.current) setPhase('open'); }));
  }, []);

  const close = useCallback(() => {
    clear(tOpen); clear(tClose); clear(tSafety);
    const p = phaseRef.current;
    if (p === 'pill' || p === 'closing') return;
    summoned.current = false;
    const g = ++gen.current;
    setPhase('closing');
    window.setTimeout(() => {
      if (g !== gen.current) return;
      // Fase pill dulu (clip langsung berbentuk baris island, tanpa transisi), baru jendela mengecil.
      setPhase('pill');
      setPreview(null);
      bridge.invoke('window_set_mode', { mode: 'pill', width: pwRef.current }).catch(() => {});
    }, morphMs());
  }, []);

  const mustStay = () =>
    pinnedRef.current || islandHold.active() || (document.hasFocus() && (summoned.current || isEditing()));

  /** Ciutkan setelah `ms` bila kursor memang di luar (diverifikasi ke Rust: event bisa terlewat saat drag OS). */
  const scheduleClose = (ms: number, onBlur = false) => {
    clear(tClose);
    tClose.current = window.setTimeout(async () => {
      const stay = onBlur ? pinnedRef.current || islandHold.active() : mustStay();
      if (inside.current || stay) return;
      const really = await bridge.invoke<boolean>('window_cursor_inside').catch(() => false);
      if (really) { inside.current = true; return; }
      close();
    }, ms);
  };

  const armOpen = () => {
    if (tOpen.current || suppress.current) return;
    tOpen.current = window.setTimeout(() => {
      tOpen.current = undefined;
      if (inside.current && !suppress.current) void open();
    }, HOVER_OPEN_MS);
  };

  const isOpen = () => phaseRef.current === 'open' || phaseRef.current === 'opening';

  // ---------------------------------------------------------------- pointer
  const onEnter = () => {
    inside.current = true;
    summoned.current = false;
    clear(tClose);
    if (phaseRef.current === 'closing') void open();
    else if (phaseRef.current === 'pill') armOpen();
  };
  const onMove = (e: PointerEvent) => {
    if (phaseRef.current === 'pill') {
      inside.current = true;
      if (e.buttons === 0) armOpen();
      return;
    }
    if (!isOpen()) return;
    clear(tSafety);
    tSafety.current = window.setTimeout(() => {
      bridge.invoke<boolean>('window_cursor_inside')
        .then((ins) => { if (!ins) { inside.current = false; scheduleClose(0); } })
        .catch(() => {});
    }, SAFETY_MS);
  };
  const onDown = () => {
    if (phaseRef.current !== 'pill') return;
    clear(tOpen);
    suppress.current = true; // kemungkinan drag: setelah dilepas jangan langsung mekar
  };
  const onLeave = () => {
    inside.current = false;
    suppress.current = false;
    clear(tOpen); clear(tSafety);
    if (isOpen()) scheduleClose(LEAVE_CLOSE_MS);
    else setPreview(null); // kursor pergi sebelum panel terbuka: jangan tinggalkan pratinjau basi
  };

  // ---------------------------------------------------------------- window, keyboard, hotkey, drag OS
  useEffect(() => {
    // Polling now-playing hidup di shell: pill tetap update walau panel ciut.
    startNowPlaying();
    // Rust sumber kebenaran mode (mis. WebView di-reload saat Expanded).
    bridge.invoke<{ mode: string }>('shell_get_state')
      .then((s) => { if (s.mode === 'expanded') setPhase('open'); })
      .catch(() => {});
    document.documentElement.dataset.island = phaseRef.current;

    const onBlur = () => {
      summoned.current = false;
      if (isOpen()) scheduleClose(BLUR_CLOSE_MS, true);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && phaseRef.current !== 'pill') { close(); return; }
      // Alt+1..9: lompat ke modul sesuai urutan deretan island.
      if (e.altKey && /^[1-9]$/.test(e.key)) {
        const m = MODULES[Number(e.key) - 1];
        if (m) { e.preventDefault(); setPreview(m.id); setTab(m.id); void open(); }
      }
    };
    window.addEventListener('blur', onBlur);
    window.addEventListener('keydown', onKey);
    const offHold = islandHold.onRelease(() => { if (isOpen() && !inside.current) scheduleClose(LEAVE_CLOSE_MS); });

    const offs: Array<() => void> = [];
    const keep = (p: Promise<() => void>) => p.then((u) => offs.push(u)).catch(() => {});
    keep(listen('shell://summon', () => { summoned.current = true; void open(); }));
    // Rust meminta panel terbuka ke modul tertentu (mis. hasil pindai QR area layar).
    keep(listen<{ tab: string }>('shell://open', (e) => {
      if (MODULES.some((m) => m.id === e.payload.tab)) setTab(e.payload.tab);
      summoned.current = true;
      void open();
    }));

    // File diseret dari OS ke atas pill: mekar langsung ke Converter, tahan terbuka selama drag.
    let releaseDrag: (() => void) | null = null;
    try {
      keep(getCurrentWebview().onDragDropEvent((ev) => {
        const t = ev.payload.type;
        if (t === 'enter') {
          releaseDrag ??= islandHold.acquire();
          if (phaseRef.current === 'pill' || phaseRef.current === 'closing') {
            if (MODULES.some((m) => m.id === 'converter')) setTab('converter');
            void open();
          }
        } else if (t === 'leave' || t === 'drop') {
          releaseDrag?.();
          releaseDrag = null;
        }
      }));
    } catch { /* di luar Tauri */ }

    return () => {
      window.removeEventListener('blur', onBlur);
      window.removeEventListener('keydown', onKey);
      offHold();
      offs.forEach((u) => u());
      releaseDrag?.();
    };
  }, []);

  // ---------------------------------------------------------------- lebar pill mengikuti isi
  useLayoutEffect(() => {
    const el = pillRef.current;
    if (!el) return;
    let t: number | undefined;
    const ro = new ResizeObserver(() => {
      const w = Math.min(PILL_W.max, Math.max(PILL_W.min, Math.ceil(el.getBoundingClientRect().width)));
      if (Math.abs(w - pwRef.current) < 2) return;
      pwRef.current = w;
      setPw(w);
      clearTimeout(t);
      t = window.setTimeout(() => {
        if (phaseRef.current === 'pill') bridge.invoke('window_set_mode', { mode: 'pill', width: pwRef.current }).catch(() => {});
      }, 60);
    });
    ro.observe(el);
    return () => { ro.disconnect(); clearTimeout(t); };
  }, []);

  const togglePin = () => setPinned((v) => { write(PIN_KEY, v ? '0' : '1'); return !v; });

  const shown = preview && MODULES.some((m) => m.id === preview) ? preview : tab;
  const def = MODULES.find((m) => m.id === shown) ?? MODULES[0];
  const panel = phase !== 'pill';

  const choose = (id: string) => {
    setPreview(id);
    setTab(id);
    if (!isOpen()) void open();
  };
  const hover = (id: string) => {
    setPreview(id);
    if (phaseRef.current === 'pill') armOpen();
  };

  return (
    <div
      class="island"
      data-phase={phase}
      data-pinned={pinned ? '' : undefined}
      style={`--pw:${pw}px`}
      onPointerEnter={onEnter}
      onPointerMove={onMove}
      onPointerDown={onDown}
      onPointerLeave={onLeave}
    >
      <div class="island__bar">
        <IslandBar
          active={tab}
          shown={shown}
          live
          compact={phase === 'pill'}
          measureRef={pillRef}
          onHover={hover}
          onSelect={choose}
          onOpen={() => void open()}
        />
      </div>
      {panel && (
        <div class="island__panel" aria-hidden={phase !== 'open'}>
          <Header icon={def.icon} title={def.title} pinned={pinned} onPin={togglePin} onCollapse={close} />
          {shown !== 'music' && <MiniPlayer onOpen={() => choose('music')} />}
          <ModuleHost id={shown} />
        </div>
      )}
    </div>
  );
}
