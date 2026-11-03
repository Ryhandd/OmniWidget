import { useEffect, useState } from 'preact/hooks';
import { indicators } from '../../core/indicators';
import { converterApi } from './api';
import { isImageFormat } from './paths';
import { canConvert, initialState, isActive, reducer, type Action, type State } from './state';
import type { PdfPages } from './types';

// State konverter hidup di level modul (bukan di komponen): UI boleh di-unmount saat pindah tab
// atau ciut ke pill sementara job tetap berjalan di Rust, lalu tersambung kembali saat dibuka lagi.
// Begitu tidak ada job aktif dan UI ditinggalkan, state dibuang (lihat `useConverter`).
let state: State = initialState;
const subs = new Set<() => void>();
let probeSeq = 0;

function dispatch(a: Action) {
  const prev = state.phase;
  state = reducer(state, a);
  subs.forEach((fn) => fn());
  if (state.phase !== prev || state.phase === 'running') syncIndicator(prev);
}

// Live activity di pill: progres tetap terlihat walau panel ciut (UI modul sudah di-unmount).
const IND = 'converter';
function syncIndicator(prev: State['phase']) {
  const wasJob = prev === 'queued' || prev === 'running';
  const base = { id: IND, icon: '⇄', tab: 'converter' } as const;
  switch (state.phase) {
    case 'queued':
    case 'running': return indicators.apply({ ...base, label: `${state.progress}%`, tone: 'accent' });
    case 'done': return wasJob && indicators.flash({ ...base, label: 'Selesai', tone: 'ok' }, 5000);
    case 'failed': return wasJob && indicators.flash({ ...base, label: 'Gagal', tone: 'warn' }, 6000);
    case 'cancelled': return indicators.apply({ id: IND, clear: true });
  }
}

export const actions = {
  async loadFile(path: string) {
    if (isActive(state) || !path.trim()) return;
    const seq = ++probeSeq;
    dispatch({ type: 'probe-start', path });
    try {
      const result = await converterApi.probe(path);
      if (seq === probeSeq) dispatch({ type: 'probe-ok', result });
    } catch (e) {
      if (seq === probeSeq) dispatch({ type: 'probe-fail', message: String(e) });
    }
  },

  setTarget: (target: string) => dispatch({ type: 'set-target', target }),
  setPages: (pages: PdfPages) => dispatch({ type: 'set-pages', pages }),
  reset: () => { if (!isActive(state)) { probeSeq++; dispatch({ type: 'reset' }); } },

  /** `outputDir` null = folder yang sama dengan berkas sumber. */
  async convert(outputDir: string | null) {
    if (!canConvert(state)) return;
    const { filePath, target, source, pdfPages } = state;
    dispatch({ type: 'job-start' });
    try {
      const id = await converterApi.start(
        {
          input_path: filePath,
          target_format: target,
          output_dir: outputDir,
          options: source === 'pdf' && isImageFormat(target) ? { pdf_pages: pdfPages } : undefined,
        },
        (ev) => dispatch({ type: 'job-event', ev }),
      );
      dispatch({ type: 'job-id', id });
      // Pembatalan ditekan sebelum id diketahui: teruskan sekarang.
      if (state.cancelling) void converterApi.cancel(id).catch(() => {});
    } catch (e) {
      dispatch({ type: 'start-failed', message: String(e) });
    }
  },

  cancel() {
    if (!isActive(state) || state.cancelling) return;
    dispatch({ type: 'cancel-requested' });
    if (state.jobId) void converterApi.cancel(state.jobId).catch(() => {});
  },
};

export function useConverter(): State {
  const [s, setS] = useState(state);
  useEffect(() => {
    const sync = () => setS(state);
    subs.add(sync);
    sync();
    return () => {
      subs.delete(sync);
      // UI ditinggalkan tanpa job aktif: lepaskan state (UI adalah view yang dapat dibuang).
      if (!isActive(state)) { probeSeq++; dispatch({ type: 'reset' }); }
    };
  }, []);
  return s;
}
