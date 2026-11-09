// Logika murni untuk overlay Coret layar (dipakai Draw.tsx dan test:web).
export type Pt = { x: number; y: number };
export type Tool = 'pen' | 'marker' | 'eraser' | 'line' | 'arrow' | 'rect' | 'text';

export const PALETTE = ['#ff3b30', '#ffcc00', '#34c759', '#0a84ff', '#ffffff', '#111111'] as const;
export const MIN_SIZE = 2;
export const MAX_SIZE = 24;

/** Batasi ketebalan agar slider/keyboard tidak pernah menghasilkan nilai aneh. */
export function clampSize(n: number): number {
  if (!Number.isFinite(n)) return 6;
  return Math.min(MAX_SIZE, Math.max(MIN_SIZE, Math.round(n)));
}

/** Kotak dari dua titik, termasuk saat pengguna menyeret ke kiri/atas. */
export function boxOf(a: Pt, b: Pt) {
  return {
    left: Math.min(a.x, b.x),
    top: Math.min(a.y, b.y),
    width: Math.abs(b.x - a.x),
    height: Math.abs(b.y - a.y),
  };
}

/** Tiga titik kepala panah: ujung, sayap kiri, sayap kanan. */
export function arrowHead(a: Pt, b: Pt, size: number): [Pt, Pt, Pt] {
  const len = Math.hypot(b.x - a.x, b.y - a.y);
  const angle = Math.atan2(b.y - a.y, b.x - a.x);
  const wing = Math.max(9, clampSize(size) * 2.2);
  const spread = 0.48;
  const left = { x: b.x - wing * Math.cos(angle - spread), y: b.y - wing * Math.sin(angle - spread) };
  const right = { x: b.x - wing * Math.cos(angle + spread), y: b.y - wing * Math.sin(angle + spread) };
  return [b, left, right];
}

/** Jarak minimal sebelum sebuah coretan dianggap nyata (mencegah titik tak sengaja). */
export function moved(a: Pt, b: Pt, min = 0.75): boolean {
  return Math.hypot(b.x - a.x, b.y - a.y) >= min;
}

/** Ukuran font teks mengikuti ketebalan pena. */
export function fontSize(size: number): number {
  return Math.round(14 + clampSize(size) * 2.4);
}

/** Warna gelap butuh latar terang saat mengetik teks di overlay. */
export function isDark(hex: string): boolean {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return false;
  const n = parseInt(m[1], 16);
  const r = (n >> 16) & 255;
  const g = (n >> 8) & 255;
  const b = n & 255;
  return 0.299 * r + 0.587 * g + 0.114 * b < 140;
}
