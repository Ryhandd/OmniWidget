import { bridge } from './bridge';

// Rust yang memegang timer Deep Sleep; UI hanya melapor "masih dipakai", dibatasi 1x per 15 dtk
// supaya tidak ada IPC per gerakan mouse. Listener pasif + capture: nol beban pada alur event lain.
const THROTTLE_MS = 15_000;
const EVENTS = ['pointerdown', 'keydown', 'wheel'] as const;

export function startActivityReporter() {
  let last = 0;
  const ping = () => {
    const now = Date.now();
    if (now - last < THROTTLE_MS) return;
    last = now;
    bridge.invoke('shell_activity').catch(() => {});
  };
  for (const ev of EVENTS) window.addEventListener(ev, ping, { passive: true, capture: true });
}
