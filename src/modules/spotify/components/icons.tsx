// Ikon SVG kecil (mengikuti warna teks). Dekoratif: label ada di tombol pemanggil.
const base = { width: 18, height: 18, viewBox: '0 0 24 24', fill: 'currentColor', 'aria-hidden': true } as const;

export const IconPlay = () => <svg {...base}><path d="M8 5v14l11-7z" /></svg>;
export const IconPause = () => <svg {...base}><path d="M6 5h4v14H6zM14 5h4v14h-4z" /></svg>;
export const IconPrev = () => <svg {...base}><path d="M6 6h2v12H6zM9.5 12 18 6v12z" /></svg>;
export const IconNext = () => <svg {...base}><path d="M16 6h2v12h-2zM6 18V6l8.5 6z" /></svg>;
export const IconShuffle = () => (
  <svg {...base}><path d="M16 4h5v5l-1.8-1.8-4.3 4.3-1.4-1.4 4.3-4.3zM3 6h4.5l3.6 5-1.2 1.6L6.5 8H3zM3 16h3.5l3.4-4.6 1.2 1.6L7.6 18H3zM16.2 12.5l1.4-1.4 1.6 1.6L21 11v5h-5l1.8-1.8z" /></svg>
);
export const IconRepeat = () => (
  <svg {...base}><path d="M7 7h10v3l4-4-4-4v3H5v6h2zM17 17H7v-3l-4 4 4 4v-3h12v-6h-2z" /></svg>
);
export const IconVolume = () => (
  <svg {...base}><path d="M3 10v4h4l5 4V6L7 10zM15.5 12a4.5 4.5 0 0 0-2.5-4v8a4.5 4.5 0 0 0 2.5-4z" /></svg>
);
