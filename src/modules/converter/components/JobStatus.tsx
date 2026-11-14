import { shortenMiddle } from '../paths';
import { converterApi } from '../api';
import type { State } from '../state';

const label = (s: State): { text: string; tone: 'muted' | 'err' | 'ok' } | null => {
  switch (s.phase) {
    case 'probing': return { text: 'Membaca file…', tone: 'muted' };
    case 'queued': return { text: 'Menunggu giliran konversi…', tone: 'muted' };
    case 'running': return { text: s.cancelling ? 'Membatalkan…' : `Memproses… ${s.progress}%`, tone: 'muted' };
    case 'cancelled': return { text: 'Dibatalkan. Tidak ada berkas yang tersisa.', tone: 'muted' };
    case 'failed': return { text: s.error, tone: 'err' };
    case 'done': return { text: s.resultIsFolder ? 'Selesai. Hasil (folder) disimpan di:' : 'Selesai. Disimpan di:', tone: 'ok' };
    default: return null;
  }
};

export function JobStatus({ state }: { state: State }) {
  const msg = label(state);
  const showBar = state.phase === 'running' || state.phase === 'queued' || state.phase === 'done';
  return (
    <div class="conv__status" role="status" aria-live="polite">
      {showBar && (
        <div
          class="progress"
          data-wait={state.phase === 'queued' || (state.phase === 'running' && state.progress === 0)}
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={state.progress}
        >
          <i style={{ width: `${state.phase === 'queued' ? 0 : state.progress}%` }} />
        </div>
      )}
      {msg && <div class={msg.tone === 'err' ? 'err' : msg.tone === 'ok' ? 'conv__ok' : 'muted'}>{msg.text}</div>}
      {state.phase === 'done' && state.resultPath && (
        <>
          <div class="conv__result" title={state.resultPath}>{shortenMiddle(state.resultPath, 56)}</div>
          <div class="conv__result-actions">
            <button type="button" class="btn" onClick={() => void converterApi.open(state.resultPath!, false).catch(() => {})}>
              {state.resultIsFolder ? 'Buka folder' : 'Buka file'}
            </button>
            {!state.resultIsFolder && (
              <button type="button" class="btn btn--ghost" onClick={() => void converterApi.open(state.resultPath!, true).catch(() => {})}>
                Tampilkan folder
              </button>
            )}
          </div>
        </>
      )}
    </div>
  );
}
