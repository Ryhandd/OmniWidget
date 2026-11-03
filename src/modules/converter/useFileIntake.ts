import { open } from '@tauri-apps/plugin-dialog';
import { useCallback, useEffect, useRef, useState } from 'preact/hooks';
import { errText } from '../../core/llm';
import { useFileDrop } from '../../core/useFileDrop';
import { withHold } from '../../core/hold';
import { converterApi } from './api';
import {
  SUPPORTED_EXTS, filesFromClipboard, hasFiles, isEditableTarget, listFiles, nameForPasted, pickUsable,
} from './intake';

export type Notice = { tone: 'err' | 'info'; text: string } | null;

export type DropHandlers = {
  onDragEnter: (e: DragEvent) => void;
  onDragOver: (e: DragEvent) => void;
  onDragLeave: (e: DragEvent) => void;
  onDrop: (e: DragEvent) => void;
};

/**
 * Tiga jalur masuk file ke Converter, semuanya berakhir sebagai satu `path` untuk `onPath`:
 *  1. Dialog native OS (`browse`)            -> path asli.
 *  2. Drag & drop: native Tauri (path asli)  + handler DOM `dropProps` (File -> disimpan sementara di Rust).
 *  3. Paste (Ctrl+V) di window               -> File dari clipboard -> disimpan sementara di Rust.
 * Jalur DOM hanya aktif bila WebView mengirim event drop DOM (mis. di browser/vite atau bila drag-drop native
 * Tauri dimatikan); selama native aktif, drop dari OS ditangani jalur native dan lebih hemat (tanpa menyalin byte).
 */
export function useFileIntake(enabled: boolean, onPath: (path: string) => void) {
  const [dragging, setDragging] = useState(false);
  const [staging, setStaging] = useState(false);
  const [notice, setNotice] = useState<Notice>(null);

  const alive = useRef(true);
  const seq = useRef(0);
  const busy = useRef(false); // sedang menyimpan file (jangan terima file kedua)
  const on = useRef(enabled);
  const cb = useRef(onPath);
  on.current = enabled;
  cb.current = onPath;

  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; seq.current++; };
  }, []);

  const accept = useCallback((path: string) => {
    setNotice(null);
    cb.current(path);
  }, []);

  // 2a. Drag & drop native: path asli dari OS.
  const nativeOver = useFileDrop((paths) => { if (!busy.current) accept(paths[0]); }, enabled);

  // File DOM (drop/paste) -> validasi -> simpan sementara di Rust -> path.
  const intake = useCallback(async (files: File[], origin: 'drop' | 'paste') => {
    const pick = pickUsable(files);
    if (!pick.file) {
      setNotice({ tone: 'err', text: pick.error ?? 'Tidak ada file yang bisa diproses.' });
      return;
    }
    const file = pick.file;
    const id = ++seq.current;
    busy.current = true;
    setStaging(true);
    setNotice(null);
    try {
      const name = origin === 'paste' ? nameForPasted(file, new Date()) : file.name;
      const bytes = new Uint8Array(await file.arrayBuffer());
      const path = await converterApi.stage(name, bytes);
      if (!alive.current || id !== seq.current) return;
      cb.current(path);
      if (pick.extra > 0) {
        setNotice({ tone: 'info', text: `Konverter memproses satu file sekali waktu; ${pick.extra} file lain diabaikan.` });
      }
    } catch (e) {
      if (alive.current && id === seq.current) setNotice({ tone: 'err', text: errText(e) });
    } finally {
      if (alive.current && id === seq.current) { busy.current = false; setStaging(false); }
    }
  }, []);

  // 2b. Handler DOM untuk kotak dropzone.
  const onOver = useCallback((e: DragEvent) => {
    if (!hasFiles(e.dataTransfer)) return; // teks/tautan yang diseret: bukan urusan kita
    e.preventDefault(); // wajib: tanpa ini browser tidak mengizinkan drop
    const ok = on.current && !busy.current;
    if (e.dataTransfer) e.dataTransfer.dropEffect = ok ? 'copy' : 'none';
    setDragging(ok);
  }, []);

  const dropProps: DropHandlers = {
    onDragEnter: onOver,
    onDragOver: onOver,
    onDragLeave: (e) => {
      // Berpindah ke elemen anak tidak dihitung "keluar" (dragleave ikut menyala di setiap anak).
      const next = e.relatedTarget as Node | null;
      if (next && (e.currentTarget as Node).contains(next)) return;
      setDragging(false);
    },
    onDrop: (e) => {
      if (!hasFiles(e.dataTransfer)) return;
      e.preventDefault(); // cegah WebView membuka file sebagai halaman
      e.stopPropagation();
      setDragging(false);
      if (!on.current || busy.current) return;
      const files = listFiles(e.dataTransfer?.files); // dibaca sinkron
      if (files.length === 0) {
        setNotice({ tone: 'err', text: 'Tidak ada file yang terbaca dari seretan tadi.' });
        return;
      }
      void intake(files, 'drop');
    },
  };

  // 3. Paste (Ctrl+V) + pengaman agar file yang dijatuhkan di luar kotak tidak membuka halaman baru.
  useEffect(() => {
    const guard = (e: DragEvent) => { if (hasFiles(e.dataTransfer)) e.preventDefault(); };
    const onPaste = (e: ClipboardEvent) => {
      if (!on.current || busy.current || isEditableTarget(e.target)) return;
      const files = filesFromClipboard(e.clipboardData); // wajib sinkron di dalam event
      if (files.length === 0) {
        setNotice({ tone: 'info', text: 'Clipboard tidak berisi file atau gambar. Salin file (atau ambil screenshot) lalu tekan Ctrl+V lagi.' });
        return;
      }
      e.preventDefault();
      void intake(files, 'paste');
    };
    window.addEventListener('dragover', guard);
    window.addEventListener('drop', guard);
    window.addEventListener('paste', onPaste);
    return () => {
      window.removeEventListener('dragover', guard);
      window.removeEventListener('drop', guard);
      window.removeEventListener('paste', onPaste);
    };
  }, [intake]);

  // 1. Dialog file native OS.
  const browse = useCallback(async () => {
    if (!on.current || busy.current) return;
    try {
      // Dialog native: kursor meninggalkan widget, tahan island tetap terbuka.
      const picked = await withHold(() => open({
        multiple: false,
        directory: false,
        title: 'Pilih file untuk dikonversi',
        filters: [
          { name: 'Format yang didukung', extensions: [...SUPPORTED_EXTS] },
          { name: 'Semua file', extensions: ['*'] },
        ],
      }));
      if (typeof picked === 'string' && picked) accept(picked);
    } catch (e) {
      setNotice({ tone: 'err', text: `Tidak bisa membuka dialog file: ${errText(e)}` });
    }
  }, [accept]);

  return { over: nativeOver || dragging, staging, notice, dropProps, browse };
}
