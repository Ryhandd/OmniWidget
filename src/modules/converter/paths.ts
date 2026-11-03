// Utilitas path murni (tanpa Node/Tauri). Menangani pemisah Windows (\) maupun Unix (/).

export function splitPath(p: string): { dir: string; base: string; stem: string; ext: string } {
  const cut = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  const dir = cut > 0 ? p.slice(0, cut) : cut === 0 ? p.slice(0, 1) : '';
  const base = cut > -1 ? p.slice(cut + 1) : p;
  const dot = base.lastIndexOf('.');
  const stem = dot > 0 ? base.slice(0, dot) : base;
  const ext = dot > 0 ? base.slice(dot + 1).toLowerCase() : '';
  return { dir, base, stem, ext };
}

/** Potong di tengah supaya nama berkas di ujung tetap terbaca: "C:\Users\…\laporan-final.pdf". */
export function shortenMiddle(p: string, max = 48): string {
  if (p.length <= max) return p;
  const keepEnd = Math.ceil((max - 1) * 0.6);
  const keepStart = max - 1 - keepEnd;
  return `${p.slice(0, keepStart)}…${p.slice(p.length - keepEnd)}`;
}

export const isImageFormat = (f: string) => f === 'png' || f === 'jpg';
