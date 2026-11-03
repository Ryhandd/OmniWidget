// Format warna eyedropper (dipakai modul Capture dan overlay). Logika murni -> teruji.
export type Rgb = [number, number, number];
export type ColorFormat = 'hex' | 'rgb' | 'hsl' | 'hsv' | 'css';
export const FORMATS: Array<{ id: ColorFormat; label: string }> = [
  { id: 'hex', label: 'HEX' }, { id: 'rgb', label: 'RGB' }, { id: 'hsl', label: 'HSL' }, { id: 'hsv', label: 'HSV' }, { id: 'css', label: 'CSS' },
];

export const toHex = ([r, g, b]: Rgb) => '#' + [r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('').toUpperCase();

export function toHsl([r, g, b]: Rgb): [number, number, number] {
  const [R, G, B] = [r / 255, g / 255, b / 255];
  const max = Math.max(R, G, B), min = Math.min(R, G, B), d = max - min;
  const l = (max + min) / 2;
  const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
  return [Math.round(hue(R, G, B, max, d)), Math.round(s * 100), Math.round(l * 100)];
}

export function toHsv([r, g, b]: Rgb): [number, number, number] {
  const [R, G, B] = [r / 255, g / 255, b / 255];
  const max = Math.max(R, G, B), d = max - Math.min(R, G, B);
  return [Math.round(hue(R, G, B, max, d)), Math.round(max === 0 ? 0 : (d / max) * 100), Math.round(max * 100)];
}

function hue(R: number, G: number, B: number, max: number, d: number) {
  if (d === 0) return 0;
  const h = max === R ? ((G - B) / d) % 6 : max === G ? (B - R) / d + 2 : (R - G) / d + 4;
  return (h * 60 + 360) % 360;
}

export function formatColor(c: Rgb, f: ColorFormat): string {
  switch (f) {
    case 'hex': return toHex(c);
    case 'rgb': return `rgb(${c[0]}, ${c[1]}, ${c[2]})`;
    case 'hsl': { const [h, s, l] = toHsl(c); return `hsl(${h}, ${s}%, ${l}%)`; }
    case 'hsv': { const [h, s, v] = toHsv(c); return `hsv(${h}, ${s}%, ${v}%)`; }
    case 'css': return `rgb(${c[0]} ${c[1]} ${c[2]} / 100%)`;
  }
}

/** Teks gelap/terang yang terbaca di atas warna ini. */
export const inkOn = ([r, g, b]: Rgb) => (0.299 * r + 0.587 * g + 0.114 * b > 150 ? '#111' : '#fff');
