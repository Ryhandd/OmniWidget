import { useEffect, useState } from 'preact/hooks';
import { listen } from '@tauri-apps/api/event';

// "Live activity" di pill (gaya Dynamic Island). Sumber bisa Rust (event "shell://indicator") atau
// modul JS (mis. progres konversi) karena modul UI di-unmount saat island ciut.
//   { id, label, tone?, icon?, tab? } -> pasang/perbarui      { id, clear: true } -> hapus
// `tab`: klik chip di pill membuka panel langsung ke modul itu.
export type Indicator = { id: string; label: string; tone?: 'accent' | 'ok' | 'warn'; icon?: string; tab?: string };
/** `ttl` (ms, opsional dari Rust): tampil sebentar lalu hilang sendiri. */
type FromRust = Indicator & { ttl?: number };
type Msg = Indicator | { id: string; clear: true };

const items = new Map<string, Indicator>();
const timers = new Map<string, number>();
const subs = new Set<() => void>();
const emit = () => subs.forEach((f) => f());

export const indicators = {
  apply(m: Msg) {
    window.clearTimeout(timers.get(m.id));
    timers.delete(m.id);
    if ('clear' in m) items.delete(m.id);
    else items.set(m.id, m);
    emit();
  },
  /** Tampil sebentar lalu hilang sendiri (notifikasi singkat di pill). */
  flash(i: Indicator, ms = 1800) {
    indicators.apply(i);
    timers.set(i.id, window.setTimeout(() => indicators.apply({ id: i.id, clear: true }), ms));
  },
  list: (): Indicator[] => [...items.values()],
  subscribe(fn: () => void) {
    subs.add(fn);
    return () => subs.delete(fn);
  },
};

export function startIndicatorBridge() {
  const ignore = () => { /* dijalankan di browser biasa (tanpa Tauri): abaikan */ };
  listen<Msg | FromRust>('shell://indicator', (e) => {
    const m = e.payload;
    if (!('clear' in m) && 'ttl' in m && m.ttl) {
      const { ttl, ...i } = m;
      indicators.flash(i, ttl);
    } else indicators.apply(m as Msg);
  }).catch(ignore);
  // Watcher clipboard (Rust, resident) sudah memancarkan event ini; cukup tampilkan kilasan di pill.
  listen('clipboard://changed', () => {
    if (document.documentElement.dataset.island !== 'pill') return; // panel terbuka: daftar sudah terlihat
    indicators.flash({ id: 'clip', label: 'Disalin', icon: '⧉', tone: 'ok', tab: 'clipboard' });
  }).catch(ignore);
}

export function useIndicators(): Indicator[] {
  const [v, setV] = useState(indicators.list());
  useEffect(() => indicators.subscribe(() => setV(indicators.list())), []);
  return v;
}
