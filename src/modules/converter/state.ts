import type { JobEvent, PdfPages, ProbeResult, TargetInfo } from './types';

export type Phase = 'idle' | 'probing' | 'ready' | 'queued' | 'running' | 'done' | 'failed' | 'cancelled';

export type State = {
  phase: Phase;
  filePath: string;
  source: string | null;
  targets: TargetInfo[];
  target: string;
  pdfPages: PdfPages;
  progress: number; // 0..100
  cancelling: boolean;
  jobId: string | null;
  resultPath: string | null;
  resultIsFolder: boolean;
  error: string;
};

export const initialState: State = {
  phase: 'idle',
  filePath: '',
  source: null,
  targets: [],
  target: '',
  pdfPages: 'all',
  progress: 0,
  cancelling: false,
  jobId: null,
  resultPath: null,
  resultIsFolder: false,
  error: '',
};

export type Action =
  | { type: 'probe-start'; path: string }
  | { type: 'probe-ok'; result: ProbeResult }
  | { type: 'probe-fail'; message: string }
  | { type: 'set-target'; target: string }
  | { type: 'set-pages'; pages: PdfPages }
  | { type: 'job-start' }
  | { type: 'job-id'; id: string }
  | { type: 'job-event'; ev: JobEvent }
  | { type: 'cancel-requested' }
  | { type: 'start-failed'; message: string }
  | { type: 'reset' };

/** Job yang masih berjalan di Rust (UI boleh ditinggalkan, state tetap terjaga). */
export const isActive = (s: Pick<State, 'phase'>) => s.phase === 'queued' || s.phase === 'running';

export const selectedTarget = (s: State): TargetInfo | undefined => s.targets.find((t) => t.format === s.target);

export const canConvert = (s: State): boolean => {
  const t = selectedTarget(s);
  return !!t && t.available && !isActive(s) && s.phase !== 'probing';
};

const firstAvailable = (targets: TargetInfo[]) => (targets.find((t) => t.available) ?? targets[0])?.format ?? '';

export function reducer(s: State, a: Action): State {
  switch (a.type) {
    case 'probe-start':
      return { ...initialState, pdfPages: s.pdfPages, phase: 'probing', filePath: a.path };

    case 'probe-ok': {
      if (a.result.targets.length === 0) {
        return { ...s, phase: 'failed', source: a.result.source, targets: [], target: '', error: 'Tidak ada format tujuan untuk file ini.' };
      }
      return { ...s, phase: 'ready', source: a.result.source, targets: a.result.targets, target: firstAvailable(a.result.targets), error: '' };
    }

    case 'probe-fail':
      return { ...s, phase: 'failed', targets: [], target: '', error: a.message };

    case 'set-target':
      return { ...s, target: a.target };

    case 'set-pages':
      return { ...s, pdfPages: a.pages };

    case 'job-start':
      return { ...s, phase: 'running', progress: 0, cancelling: false, jobId: null, resultPath: null, resultIsFolder: false, error: '' };

    case 'job-id':
      return { ...s, jobId: a.id };

    case 'cancel-requested':
      return isActive(s) ? { ...s, cancelling: true } : s;

    case 'start-failed':
      return { ...s, phase: 'failed', cancelling: false, error: a.message };

    case 'job-event': {
      const ev = a.ev;
      switch (ev.event) {
        case 'queued': return { ...s, phase: 'queued' };
        case 'started': return { ...s, phase: 'running' };
        case 'progress': return isActive(s) ? { ...s, progress: Math.round(ev.pct * 100) } : s;
        case 'complete':
          return { ...s, phase: 'done', progress: 100, cancelling: false, resultPath: ev.path, resultIsFolder: ev.folder, error: '' };
        case 'error': return { ...s, phase: 'failed', cancelling: false, error: ev.message };
        case 'cancelled': return { ...s, phase: 'cancelled', progress: 0, cancelling: false };
      }
      return s;
    }

    case 'reset':
      return { ...initialState, pdfPages: s.pdfPages };
  }
}
