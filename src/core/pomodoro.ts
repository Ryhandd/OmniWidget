import { useEffect, useState } from 'preact/hooks';
import { listen } from '@tauri-apps/api/event';
import { bridge } from './bridge';
import { indicators } from './indicators';

// Timer Pomodoro hidup di Rust; store ini hanya cermin ringan yang dipakai pill (hitung mundur) dan
// modul Pomodoro. Interval 1 dtk HANYA berjalan saat timer berjalan.

export type Phase = 'idle' | 'focus' | 'short' | 'long';
export type PomoConfig = { focusMin: number; shortMin: number; longMin: number; longEvery: number; autoStart: boolean };
export type PomoSnap = {
  phase: Phase;
  running: boolean;
  endsAt: number | null;
  remainingMs: number;
  startedAt: number | null;
  cycle: number;
  config: PomoConfig;
  now: number;
  todayFocus: number;
  todayMinutes: number;
};

export const PHASE_LABEL: Record<Phase, string> = { idle: 'Siap', focus: 'Fokus', short: 'Istirahat', long: 'Istirahat panjang' };
const ICON: Record<Phase, string> = { idle: '🍅', focus: '🍅', short: '☕', long: '🌿' };

let snap: PomoSnap | null = null;
let offset = 0; // jam Rust - jam JS (untuk sisa waktu yang akurat)
let tick: number | undefined;
const subs = new Set<() => void>();

export const startOfToday = () => { const d = new Date(); d.setHours(0, 0, 0, 0); return d.getTime(); };

export function remaining(s: PomoSnap | null = snap): number {
  if (!s) return 0;
  return s.running && s.endsAt ? Math.max(0, s.endsAt - (Date.now() + offset)) : s.remainingMs;
}

export const fmt = (ms: number) => {
  const t = Math.ceil(ms / 1000);
  return `${String(Math.floor(t / 60)).padStart(2, '0')}:${String(t % 60).padStart(2, '0')}`;
};

function publish() {
  subs.forEach((f) => f());
  const s = snap;
  if (!s || s.phase === 'idle') {
    indicators.apply({ id: 'pomo', clear: true });
  } else {
    indicators.apply({
      id: 'pomo',
      icon: s.running ? ICON[s.phase] : '⏸',
      label: fmt(remaining(s)),
      tone: s.phase === 'focus' ? 'accent' : 'ok',
      tab: 'pomodoro',
    });
  }
  window.clearInterval(tick);
  tick = undefined;
  if (s?.running) tick = window.setInterval(() => { subs.forEach((f) => f()); publishLabel(); }, 1000);
}

function publishLabel() {
  if (snap?.running) indicators.apply({ id: 'pomo', icon: ICON[snap.phase], label: fmt(remaining()), tone: snap.phase === 'focus' ? 'accent' : 'ok', tab: 'pomodoro' });
}

export async function refresh() {
  try {
    const s = await bridge.invoke<PomoSnap>('pomo_get', { since: startOfToday() });
    offset = s.now - Date.now();
    snap = s;
    publish();
  } catch { /* di luar Tauri */ }
}

const act = (cmd: string, args?: Record<string, unknown>) => bridge.invoke(cmd, args).then(refresh).catch(() => {});

export const pomo = {
  start: () => act('pomo_start'),
  pause: () => act('pomo_pause'),
  skip: () => act('pomo_skip'),
  reset: () => act('pomo_reset'),
  setConfig: (config: PomoConfig) => act('pomo_set_config', { config }),
};

/** Dipanggil sekali saat aplikasi mulai (WebView juga dibangun ulang setelah Deep Sleep -> sinkron lagi). */
export function startPomodoroBridge() {
  void refresh();
  listen('pomodoro://changed', () => void refresh()).catch(() => {});
}

export function usePomodoro(): PomoSnap | null {
  const [, force] = useState(0);
  useEffect(() => {
    const f = () => force((n) => n + 1);
    subs.add(f);
    return () => { subs.delete(f); };
  }, []);
  return snap;
}
