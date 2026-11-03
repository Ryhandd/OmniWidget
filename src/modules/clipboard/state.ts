import type { ClipItem, ClipSettings, ClipSnapshot } from './types';

// Logika murni modul Clipboard (tanpa DOM/Tauri) agar teruji di `npm run test:web`.

export type State = {
  phase: 'loading' | 'ready';
  items: ClipItem[];
  settings: ClipSettings;
  capacity: number;
  query: string;
  error: string | null;
};

export type Action =
  | { type: 'snapshot'; snap: ClipSnapshot }
  | { type: 'settings'; settings: ClipSettings }
  | { type: 'query'; query: string }
  | { type: 'error'; message: string }
  | { type: 'clear-error' }
  // Efek lokal instan sebelum Rust mengonfirmasi; snapshot berikutnya jadi sumber kebenaran.
  | { type: 'optimistic-pin'; id: string; pinned: boolean }
  | { type: 'optimistic-remove'; id: string }
  | { type: 'optimistic-clear'; keepPinned: boolean };

export const DEFAULT_SETTINGS: ClipSettings = { paused: false, persist: false, clearOnExit: false };

export const initialState: State = {
  phase: 'loading',
  items: [],
  settings: DEFAULT_SETTINGS,
  capacity: 20,
  query: '',
  error: null,
};

export function reducer(s: State, a: Action): State {
  switch (a.type) {
    case 'snapshot':
      return { ...s, phase: 'ready', items: a.snap.items, settings: a.snap.settings, capacity: a.snap.capacity, error: null };
    case 'settings': return { ...s, settings: a.settings };
    case 'query': return { ...s, query: a.query };
    case 'error': return { ...s, error: a.message };
    case 'clear-error': return { ...s, error: null };
    case 'optimistic-pin':
      return { ...s, items: s.items.map((it) => (it.id === a.id ? { ...it, pinned: a.pinned } : it)) };
    case 'optimistic-remove':
      return { ...s, items: s.items.filter((it) => it.id !== a.id) };
    case 'optimistic-clear':
      return { ...s, items: a.keepPinned ? s.items.filter((it) => it.pinned) : [] };
  }
}

/** Cocokkan kueri (tanpa peduli huruf besar/kecil & spasi di tepi) ke pratinjau item. Kueri kosong = cocok semua. */
export function matches(it: ClipItem, query: string): boolean {
  const q = query.trim().toLowerCase();
  return !q || it.preview.toLowerCase().includes(q);
}

export type Groups = { pinned: ClipItem[]; recent: ClipItem[] };

/** Pisahkan item tersemat; urutan relatif dari Rust (terbaru dulu) dipertahankan. */
export function group(items: readonly ClipItem[], query: string): Groups {
  const pinned: ClipItem[] = [];
  const recent: ClipItem[] = [];
  for (const it of items) {
    if (!matches(it, query)) continue;
    (it.pinned ? pinned : recent).push(it);
  }
  return { pinned, recent };
}

/** Urutan tampil (pin dulu, lalu terbaru): dasar navigasi keyboard. */
export const flatOrder = (g: Groups): ClipItem[] => [...g.pinned, ...g.recent];

export function relTime(ts: number, now: number): string {
  const sec = Math.max(0, Math.floor((now - ts) / 1000));
  if (sec < 10) return 'baru saja';
  if (sec < 60) return `${sec} dtk lalu`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min} mnt lalu`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr} jam lalu`;
  return `${Math.floor(hr / 24)} hr lalu`;
}

export function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(n < 10 * 1024 ? 1 : 0)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

/** Jumlah item tidak tersemat vs kapasitas ring, mis. "14/20". */
export const usage = (items: readonly ClipItem[], capacity: number) =>
  `${items.filter((i) => !i.pinned).length}/${capacity}`;
