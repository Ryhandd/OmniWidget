import { getCurrentWebview } from '@tauri-apps/api/webview';
import { useEffect, useRef, useState } from 'preact/hooks';

/**
 * Drag & drop file dari OS ke jendela (Tauri memberi path asli, bukan objek File).
 * Mengembalikan `over` (sedang diseret di atas jendela). Dapat dipakai ulang oleh modul lain (mis. QR scanner).
 */
export function useFileDrop(onDrop: (paths: string[]) => void, enabled = true): boolean {
  const [over, setOver] = useState(false);
  const handler = useRef(onDrop);
  const on = useRef(enabled);
  handler.current = onDrop;
  on.current = enabled;

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    try {
      getCurrentWebview()
        .onDragDropEvent((ev) => {
          const p = ev.payload;
          if (!on.current) { setOver(false); return; }
          if (p.type === 'enter' || p.type === 'over') setOver(true);
          else if (p.type === 'leave') setOver(false);
          else if (p.type === 'drop') {
            setOver(false);
            if (p.paths.length) handler.current(p.paths);
          }
        })
        .then((u) => (disposed ? u() : (unlisten = u)))
        .catch(() => {});
    } catch {
      /* di luar Tauri (vite di browser): tidak ada drag & drop native */
    }
    return () => { disposed = true; unlisten?.(); };
  }, []);

  return over;
}
