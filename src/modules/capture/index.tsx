import { useEffect, useState } from 'preact/hooks';
import { bridge } from '../../core/bridge';
import { FORMATS, formatColor, inkOn, type ColorFormat, type Rgb } from './color';
import './styles.css';

const FORMAT_KEY = 'capture.format';

export default function CaptureModule() {
  const [format, setFormat] = useState<ColorFormat>('hex');
  const [colors, setColors] = useState<Rgb[]>([]);
  const [err, setErr] = useState('');
  const [copied, setCopied] = useState<string | null>(null);

  useEffect(() => {
    bridge.invoke<string | null>('kv_get', { key: FORMAT_KEY }).then((v) => { if (v && FORMATS.some((f) => f.id === v)) setFormat(v as ColorFormat); }).catch(() => {});
    bridge.invoke<{ colors: Rgb[] }>('capture_recent').then((r) => setColors(r.colors)).catch(() => {});
  }, []);

  const pickFormat = (f: ColorFormat) => { setFormat(f); bridge.invoke('kv_set', { key: FORMAT_KEY, value: f }).catch(() => {}); };

  const start = (mode: 'eyedropper' | 'ruler') => {
    setErr('');
    bridge.invoke('capture_start', { mode }).catch((e) => setErr(String(e)));
  };

  const copy = (c: Rgb) => {
    const text = formatColor(c, format);
    bridge.invoke('clipboard_write', { text }).then(() => { setCopied(text); setTimeout(() => setCopied(null), 1400); }).catch((e) => setErr(String(e)));
  };

  return (
    <div class="cap">
      <div class="cap__tools">
        <button type="button" class="cap__tool" onClick={() => start('eyedropper')}>
          <span class="cap__ico" aria-hidden="true">
            <svg viewBox="0 0 24 24"><path d="M14.5 4.5l5 5M17 2.5l4.5 4.5-3 3-4.5-4.5zM14 8l-9.5 9.5L3 21l3.5-1.5L16 10" /></svg>
          </span>
          <strong>Ambil warna</strong>
          <span class="muted">Klik piksel mana pun di layar</span>
        </button>
        <button type="button" class="cap__tool" onClick={() => start('ruler')}>
          <span class="cap__ico" aria-hidden="true">
            <svg viewBox="0 0 24 24"><path d="M3 17L17 3l4 4L7 21zM7 13l2 2M10 10l2 2M13 7l2 2" /></svg>
          </span>
          <strong>Penggaris piksel</strong>
          <span class="muted">Seret untuk ukur kotak / garis</span>
        </button>
      </div>
      {err && <div class="cap__err err">{err}</div>}

      <section>
        <div class="cap__row">
          <h4>Format salin</h4>
          <div class="cap__seg" role="radiogroup" aria-label="Format warna">
            {FORMATS.map((f) => (
              <button key={f.id} type="button" role="radio" aria-checked={format === f.id} onClick={() => pickFormat(f.id)}>{f.label}</button>
            ))}
          </div>
        </div>
      </section>

      <section>
        <h4>Warna terakhir</h4>
        {colors.length === 0 ? (
          <p class="muted cap__empty">Belum ada. Warna yang diambil otomatis tersalin ke clipboard dan muncul di sini.</p>
        ) : (
          <div class="cap__swatches">
            {colors.map((c) => (
              <button key={c.join()} type="button" class="cap__sw" style={`background:rgb(${c[0]},${c[1]},${c[2]});color:${inkOn(c)}`}
                title={`Salin ${formatColor(c, format)}`} onClick={() => copy(c)}>
                <span>{formatColor(c, 'hex')}</span>
              </button>
            ))}
          </div>
        )}
        {copied && <div class="cap__copied">Tersalin: <code>{copied}</code></div>}
      </section>

      <section class="cap__keys muted">
        <div><kbd>Klik</kbd> ambil · <kbd>←↑→↓</kbd> geser 1 px · <kbd>Enter</kbd> ambil · <kbd>Esc</kbd> batal</div>
        <div>Penggaris: <kbd>B</kbd> kotak · <kbd>L</kbd> garis · <kbd>C</kbd> salin ukuran · <kbd>Esc</kbd> selesai</div>
      </section>
    </div>
  );
}
