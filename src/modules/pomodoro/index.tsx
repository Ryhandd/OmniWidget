import { useEffect, useState } from 'preact/hooks';
import { fmt, PHASE_LABEL, pomo, refresh, remaining, usePomodoro, type PomoConfig } from '../../core/pomodoro';
import './styles.css';

const R = 76;
const C = 2 * Math.PI * R;

export default function PomodoroModule() {
  const s = usePomodoro();
  const [showCfg, setShowCfg] = useState(false);
  useEffect(() => { void refresh(); }, []);

  if (!s) return <div class="host__loading">Memuat…</div>;

  const total = s.phase === 'idle' ? s.config.focusMin * 60_000 : minutesOf(s.config, s.phase) * 60_000;
  const left = s.phase === 'idle' ? total : remaining(s);
  const progress = total ? 1 - left / total : 0;
  const waiting = !s.running && s.phase !== 'idle' && left === total; // fase berikut menunggu "Mulai"
  const phase = s.phase === 'idle' ? 'focus' : s.phase;

  return (
    <div class="pomo" data-phase={phase}>
      <div class="pomo__ring">
        <svg viewBox="0 0 180 180" aria-hidden="true">
          <circle class="pomo__track" cx="90" cy="90" r={R} />
          <circle class="pomo__bar" cx="90" cy="90" r={R} style={`stroke-dasharray:${C};stroke-dashoffset:${C * (1 - progress)}`} />
        </svg>
        <div class="pomo__center">
          <span class="pomo__phase">{s.phase === 'idle' ? 'Siap fokus' : waiting ? `Berikutnya: ${PHASE_LABEL[s.phase]}` : PHASE_LABEL[s.phase]}</span>
          <span class="pomo__time">{fmt(left)}</span>
          <span class="pomo__dots" aria-label={`Sesi ${s.cycle} dari ${s.config.longEvery}`}>
            {Array.from({ length: s.config.longEvery }, (_, i) => <i key={i} class={i < s.cycle ? 'on' : ''} />)}
          </span>
        </div>
      </div>

      <div class="pomo__ctrl">
        <button type="button" class="pomo__side" title="Reset" onClick={pomo.reset} disabled={s.phase === 'idle'}>
          <svg viewBox="0 0 16 16"><path d="M3 8a5 5 0 1 0 1.6-3.7M3 2.5v2.5h2.5" /></svg>
        </button>
        {s.running ? (
          <button type="button" class="pomo__main" onClick={pomo.pause}>Jeda</button>
        ) : (
          <button type="button" class="pomo__main" onClick={pomo.start}>{s.phase === 'idle' || waiting ? 'Mulai' : 'Lanjut'}</button>
        )}
        <button type="button" class="pomo__side" title="Lewati fase" onClick={pomo.skip} disabled={s.phase === 'idle'}>
          <svg viewBox="0 0 16 16"><path d="M4 3.5l6 4.5-6 4.5zM12 3.5v9" /></svg>
        </button>
      </div>

      <div class="pomo__stats">
        <div><strong>{s.todayFocus}</strong><span>sesi fokus hari ini</span></div>
        <div><strong>{s.todayMinutes}</strong><span>menit fokus</span></div>
      </div>

      <button type="button" class="pomo__cfgbtn" onClick={() => setShowCfg((v) => !v)}>{showCfg ? '▾' : '▸'} Atur durasi</button>
      {showCfg && <Config cfg={s.config} />}
      <p class="pomo__note muted">Timer tetap berjalan walau panel ciut atau widget tidur; notifikasi muncul tiap akhir fase.</p>
    </div>
  );
}

function minutesOf(c: PomoConfig, p: 'focus' | 'short' | 'long') {
  return p === 'focus' ? c.focusMin : p === 'short' ? c.shortMin : c.longMin;
}

function Config({ cfg }: { cfg: PomoConfig }) {
  const [f, setF] = useState(cfg);
  const num = (k: keyof PomoConfig, label: string, max: number) => (
    <label>{label}
      <input class="field" type="number" min={1} max={max} value={f[k] as number}
        onInput={(e) => setF({ ...f, [k]: Math.max(1, Math.min(max, Number((e.target as HTMLInputElement).value) || 1)) })} />
    </label>
  );
  const dirty = JSON.stringify(f) !== JSON.stringify(cfg);
  return (
    <div class="pomo__cfg">
      {num('focusMin', 'Fokus (menit)', 180)}
      {num('shortMin', 'Istirahat', 60)}
      {num('longMin', 'Istirahat panjang', 120)}
      {num('longEvery', 'Panjang tiap … sesi', 12)}
      <label class="pomo__auto">
        <input type="checkbox" checked={f.autoStart} onChange={(e) => setF({ ...f, autoStart: (e.target as HTMLInputElement).checked })} />
        Mulai fase berikut otomatis
      </label>
      <button type="button" class="btn" disabled={!dirty} onClick={() => pomo.setConfig(f)}>Simpan</button>
    </div>
  );
}
