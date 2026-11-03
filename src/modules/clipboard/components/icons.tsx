// Ikon SVG kecil (mengikuti warna teks). Dekoratif: label ada di tombol pemanggil.
const base = { width: 16, height: 16, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': 2, 'stroke-linecap': 'round', 'stroke-linejoin': 'round', 'aria-hidden': true } as const;

export const IconPin = ({ filled }: { filled?: boolean }) => (
  <svg {...base} fill={filled ? 'currentColor' : 'none'}><path d="M9 4h6l-1 6 3 3H7l3-3zM12 13v7" /></svg>
);
export const IconTrash = () => <svg {...base}><path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3" /></svg>;
export const IconExpand = ({ open }: { open: boolean }) => (
  <svg {...base}>{open ? <path d="M6 15l6-6 6 6" /> : <path d="M6 9l6 6 6-6" />}</svg>
);
export const IconPause = () => <svg {...base} fill="currentColor" stroke="none"><path d="M7 5h3v14H7zM14 5h3v14h-3z" /></svg>;
export const IconRec = () => <svg {...base} fill="currentColor" stroke="none"><circle cx="12" cy="12" r="5" /></svg>;
export const IconGear = () => (
  <svg {...base}><circle cx="12" cy="12" r="3" /><path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M18.4 5.6l-2.1 2.1M7.7 16.3l-2.1 2.1" /></svg>
);
export const IconImage = () => <svg {...base}><rect x="4" y="5" width="16" height="14" rx="2" /><path d="M4 16l4-4 4 4 3-3 5 5" /></svg>;
export const IconText = () => <svg {...base}><path d="M5 6h14M12 6v13M9 19h6" /></svg>;
