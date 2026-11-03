import { useEffect, useRef, useState } from 'preact/hooks';
import { bridge } from '../../core/bridge';
import './styles.css';

type Sample = { cpu: number; cores: number; memUsed: number; memTotal: number; swapUsed: number; swapTotal: number; selfRss: number };
type Group = { name: string; pids: number[]; memory: number; protected: boolean };

const N = 60; // 60 detik sparkline
const gb = (b: number) => (b / 1024 ** 3).toFixed(1);
const mb = (b: number) => Math.round(b / 1024 ** 2).toLocaleString();

export default function SysmonModule() {
  const [s, setS] = useState<Sample | null>(null);
  const cpu = useRef<number[]>([]);
  const mem = useRef<number[]>([]);
  const [err, setErr] = useState('');
  const [showTop, setShowTop] = useState(false);
  const [trim, setTrim] = useState('');

  // Sampler 1 dtk HANYA selama tab terlihat; saat ditinggalkan struktur sysinfo di Rust dilepas.
  useEffect(() => {
    let alive = true;
    const tick = () =>
      bridge.invoke<Sample>('sysmon_sample').then((x) => {
        if (!alive) return;
        cpu.current = [...cpu.current, x.cpu].slice(-N);
        mem.current = [...mem.current, (x.memUsed / x.memTotal) * 100].slice(-N);
        setS(x);
      }).catch((e) => setErr(String(e)));
    void tick();
    const t = setInterval(tick, 1000);
    return () => { alive = false; clearInterval(t); bridge.invoke('sysmon_stop').catch(() => {}); };
  }, []);

  const trimSelf = () => {
    setTrim('Membersihkan…');
    bridge.invoke<{ before: number; after: number }>('sysmon_trim_self')
      .then((r) => setTrim(`Selesai: ${mb(r.before)} → ${mb(r.after)} MB (dibebaskan ${mb(Math.max(0, r.before - r.after))} MB)`))
      .catch((e) => setTrim(String(e)));
  };

  const memPct = s ? (s.memUsed / s.memTotal) * 100 : 0;
  return (
    <div class="sys">
      {err && <div class="err">{err}</div>}
      <div class="sys__cards">
        <Card label="CPU" value={s ? `${s.cpu.toFixed(0)}%` : '–'} sub={s ? `${s.cores} core` : ''} pct={s?.cpu ?? 0} data={cpu.current} />
        <Card label="Memori" value={s ? `${memPct.toFixed(0)}%` : '–'} sub={s ? `${gb(s.memUsed)} / ${gb(s.memTotal)} GB` : ''} pct={memPct} data={mem.current} />
      </div>
      {s && (
        <div class="sys__line">
          <span>Swap/pagefile</span><strong>{s.swapTotal ? `${gb(s.swapUsed)} / ${gb(s.swapTotal)} GB` : 'tidak ada'}</strong>
          <span>OmniWidget (inti)</span><strong>{mb(s.selfRss)} MB</strong>
        </div>
      )}

      <section class="sys__sec">
        <h4>Pembersih memori</h4>
        <div class="sys__row">
          <button type="button" class="btn btn--ghost" onClick={trimSelf}>Bersihkan memori OmniWidget</button>
        </div>
        {trim && <div class="sys__note">{trim}</div>}
        <p class="muted sys__fine">
          Tidak ada tombol aman yang “membersihkan RAM” seluruh sistem: Windows sengaja memakai RAM kosong sebagai cache.
          Cara yang benar-benar efektif adalah menutup aplikasi yang memakan memori besar di bawah.
        </p>
        <button type="button" class="sys__toggle" onClick={() => setShowTop((v) => !v)}>{showTop ? '▾' : '▸'} Aplikasi pemakai memori terbesar</button>
        {showTop && <TopApps />}
      </section>
    </div>
  );
}

function Card({ label, value, sub, pct, data }: { label: string; value: string; sub: string; pct: number; data: number[] }) {
  const pts = data.map((v, i) => `${(i / (N - 1)) * 100},${30 - (Math.min(100, v) / 100) * 28}`).join(' ');
  const tone = pct > 85 ? 'hot' : pct > 65 ? 'warm' : '';
  return (
    <div class={`sys__card ${tone}`}>
      <div class="sys__head"><span>{label}</span><span class="muted">{sub}</span></div>
      <div class="sys__val">{value}</div>
      <svg viewBox="0 0 100 30" preserveAspectRatio="none" class="sys__spark" aria-hidden="true">
        {data.length > 1 && <polyline points={pts} />}
      </svg>
    </div>
  );
}

function TopApps() {
  const [list, setList] = useState<Group[] | null>(null);
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [step, setStep] = useState<'idle' | 'confirm' | 'force'>('idle');
  const [msg, setMsg] = useState('');

  // Enumerasi proses paling mahal: 3 dtk, hanya saat panel ini terbuka.
  useEffect(() => {
    let alive = true;
    const load = () => bridge.invoke<Group[]>('sysmon_top', { limit: 10 }).then((v) => alive && setList(v)).catch((e) => setMsg(String(e)));
    void load();
    const t = setInterval(load, 3000);
    return () => { alive = false; clearInterval(t); };
  }, []);

  const chosen = (list ?? []).filter((g) => sel.has(g.name));
  const pids = chosen.flatMap((g) => g.pids);

  const close = async (force: boolean) => {
    try {
      const n = await bridge.invoke<number>('sysmon_close', { pids, force });
      setMsg(`${n} proses ${force ? 'dipaksa tutup' : 'diminta tutup'}.`);
      if (!force) {
        // Aplikasi yang menolak (mis. ada dokumen belum disimpan) tetap hidup: tawarkan paksa tutup.
        setTimeout(async () => {
          const now = await bridge.invoke<Group[]>('sysmon_top', { limit: 30 }).catch(() => []);
          const still = now.filter((g) => sel.has(g.name));
          if (still.length) { setStep('force'); setMsg(`${still.map((g) => g.name).join(', ')} masih berjalan (mungkin menunggu simpan).`); }
          else { setStep('idle'); setSel(new Set()); }
        }, 2500);
      } else { setStep('idle'); setSel(new Set()); }
    } catch (e) { setMsg(String(e)); setStep('idle'); }
  };

  const toggle = (name: string) => { const n = new Set(sel); n.has(name) ? n.delete(name) : n.add(name); setSel(n); setStep('idle'); };

  return (
    <div class="sys__top">
      {list === null ? <div class="muted">Memuat…</div> : list.map((g) => (
        <label key={g.name} class={`sys__proc${g.protected ? ' is-prot' : ''}`} title={g.protected ? 'Proses sistem — dilindungi' : ''}>
          <input type="checkbox" disabled={g.protected} checked={sel.has(g.name)} onChange={() => toggle(g.name)} />
          <span class="sys__pname">{g.name}{g.pids.length > 1 && <span class="muted"> ×{g.pids.length}</span>}</span>
          <span class="sys__pmem">{mb(g.memory)} MB</span>
        </label>
      ))}
      {chosen.length > 0 && step === 'idle' && <button type="button" class="btn" onClick={() => setStep('confirm')}>Tutup {chosen.length} aplikasi…</button>}
      {step === 'confirm' && (
        <div class="sys__confirm">
          <span>Tutup {chosen.map((g) => g.name).join(', ')}? Data yang belum disimpan bisa hilang.</span>
          <div class="sys__row"><button type="button" class="btn btn--ghost" onClick={() => setStep('idle')}>Batal</button><button type="button" class="btn" onClick={() => void close(false)}>Tutup</button></div>
        </div>
      )}
      {step === 'force' && (
        <div class="sys__confirm is-danger">
          <span>Paksa tutup? Pekerjaan yang belum disimpan <strong>pasti hilang</strong>.</span>
          <div class="sys__row"><button type="button" class="btn btn--ghost" onClick={() => setStep('idle')}>Biarkan</button><button type="button" class="btn sys__danger" onClick={() => void close(true)}>Paksa tutup</button></div>
        </div>
      )}
      {msg && <div class="sys__note">{msg}</div>}
    </div>
  );
}
