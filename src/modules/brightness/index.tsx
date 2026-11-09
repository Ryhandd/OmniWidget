import { useEffect, useRef, useState } from 'preact/hooks';
import { bridge } from '../../core/bridge';
import './styles.css';

const PRESETS = [25, 50, 75, 100];
const SET_DEBOUNCE_MS = 140;

/**
 * Kecerahan hardware monitor lewat WMI (`WmiMonitorBrightnessMethods`).
 * Banyak monitor eksternal tidak mengekspos kelas ini: `brightness_get` mengembalikan null,
 * dan modul menampilkan penjelasan alih-alih slider yang tidak berfungsi.
 */
export default function BrightnessModule() {
  const [level, setLevel] = useState(60);
  const [supported, setSupported] = useState<boolean | null>(null);
  const [err, setErr] = useState('');
  const [busy, setBusy] = useState(false);
  const timer = useRef<number>();
  const latest = useRef(60);

  useEffect(() => {
    let alive = true;
    bridge.invoke<number | null>('brightness_get')
      .then((v) => {
        if (!alive) return;
        if (v === null) { setSupported(false); return; }
        latest.current = v;
        setLevel(v);
        setSupported(true);
      })
      .catch((e) => { if (alive) { setErr(String(e)); setSupported(false); } });
    return () => {
      alive = false;
      if (timer.current) window.clearTimeout(timer.current);
    };
  }, []);

  /** Tampilkan nilai seketika, tulis ke hardware setelah jeda singkat (slider tidak membanjiri WMI). */
  const apply = (value: number, immediate = false) => {
    const next = Math.min(100, Math.max(0, Math.round(value)));
    latest.current = next;
    setLevel(next);
    setErr('');
    if (timer.current) window.clearTimeout(timer.current);
    const send = () => {
      setBusy(true);
      bridge.invoke('brightness_set', { level: latest.current })
        .catch((e) => setErr(String(e)))
        .finally(() => setBusy(false));
    };
    if (immediate) send();
    else timer.current = window.setTimeout(send, SET_DEBOUNCE_MS);
  };

  if (supported === null) return <div class="bri"><div class="bri__loading muted">Memeriksa kecerahan monitor…</div></div>;

  if (!supported) {
    return (
      <div class="bri">
        <div class="bri__card bri__card--unsupported">
          <span class="bri__icon" aria-hidden="true">☀</span>
          <strong>Brightness tidak didukung</strong>
          <p class="muted">
            Monitor ini tidak menyediakan pengaturan kecerahan lewat Windows (umumnya monitor eksternal).
            Coba gunakan tombol fisik monitor, atau atur dari menu OSD perangkat.
          </p>
          {err && !/0x80004005/i.test(err) && <div class="err bri__err">{err}</div>}
        </div>
      </div>
    );
  }

  const pct = `${level}%`;

  return (
    <div class="bri">
      <div class="bri__card">
        <div class="bri__head">
          <span class={`bri__icon${busy ? ' is-busy' : ''}`} aria-hidden="true">☀</span>
          <div class="bri__meta">
            <strong>Kecerahan monitor</strong>
            <span class="muted">Hardware · {pct}</span>
          </div>
          <output class="bri__pct" aria-live="polite">{pct}</output>
        </div>

        <div class="bri__slider">
          <button type="button" class="bri__step" aria-label="Kurangi kecerahan" onClick={() => apply(latest.current - 5)}>−</button>
          <input
            type="range"
            min={0}
            max={100}
            step={1}
            value={level}
            aria-label="Kecerahan monitor"
            style={`--bri:${level}%`}
            onInput={(e) => apply(Number((e.currentTarget as HTMLInputElement).value))}
          />
          <button type="button" class="bri__step" aria-label="Tambah kecerahan" onClick={() => apply(latest.current + 5)}>+</button>
        </div>

        <div class="bri__presets" aria-label="Preset kecerahan">
          {PRESETS.map((p) => (
            <button
              key={p}
              type="button"
              class="bri__preset"
              aria-pressed={level === p}
              onClick={() => apply(p, true)}
            >
              {p}%
            </button>
          ))}
        </div>

        {err && <div class="err bri__err">{err}</div>}
      </div>

      <div class="bri__ticks" aria-hidden="true">
        <span>0</span><span>25</span><span>50</span><span>75</span><span>100</span>
      </div>
    </div>
  );
}
