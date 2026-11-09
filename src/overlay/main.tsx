import { render } from 'preact';
import { useEffect, useRef, useState } from 'preact/hooks';
import { bridge } from '../core/bridge';
import { formatColor, inkOn, type ColorFormat, type Rgb } from '../modules/capture/color';
import Draw from './DrawPad';
import './overlay.css';

// Overlay layar penuh on-demand (TDD 4.6). Transparan: desktop asli terlihat langsung; sampling warna
// memakai snapshot di Rust (hanya potongan 15x15 yang dikirim ke sini).
type Mode = 'eyedropper' | 'ruler' | 'qr' | 'draw';
declare global { interface Window { __OVERLAY__?: { mode: Mode; scale: number } } }

const cfg = window.__OVERLAY__ ?? { mode: (new URLSearchParams(location.search).get('mode') as Mode) || 'ruler', scale: devicePixelRatio || 1 };
const close = (openTab?: string) => bridge.invoke('capture_close', { openTab }).catch(() => window.close());
type Pt = { x: number; y: number };

function Eyedropper() {
  const [pos, setPos] = useState<Pt | null>(null);
  const [rgb, setRgb] = useState<Rgb | null>(null);
  const [format, setFormat] = useState<ColorFormat>('hex');
  const canvas = useRef<HTMLCanvasElement>(null);
  const want = useRef<Pt | null>(null);
  const busy = useRef(false);

  useEffect(() => {
    bridge.invoke<string | null>('kv_get', { key: 'capture.format' }).then((v) => v && setFormat(v as ColorFormat)).catch(() => {});
  }, []);

  // Ambil patch maksimal 1 permintaan sekaligus (otomatis ~60 Hz atau lebih lambat bila IPC sibuk).
  const pump = async () => {
    if (busy.current || !want.current) return;
    busy.current = true;
    const p = want.current;
    want.current = null;
    try {
      const buf = new Uint8ClampedArray(await bridge.invoke<ArrayBuffer>('capture_patch', p)) as Uint8ClampedArray<ArrayBuffer>;
      draw(buf);
      const i = (7 * 15 + 7) * 4;
      setRgb([buf[i], buf[i + 1], buf[i + 2]]);
    } catch { /* abaikan satu frame */ }
    busy.current = false;
    if (want.current) requestAnimationFrame(() => void pump());
  };

  const draw = (buf: Uint8ClampedArray<ArrayBuffer>) => {
    const c = canvas.current?.getContext('2d');
    if (!c) return;
    const tmp = new OffscreenCanvas(15, 15);
    tmp.getContext('2d')!.putImageData(new ImageData(buf, 15, 15), 0, 0);
    c.imageSmoothingEnabled = false;
    c.clearRect(0, 0, 150, 150);
    c.drawImage(tmp, 0, 0, 150, 150);
    c.strokeStyle = 'rgba(255,255,255,0.12)';
    c.lineWidth = 1;
    for (let k = 1; k < 15; k++) {
      c.beginPath(); c.moveTo(k * 10 + 0.5, 0); c.lineTo(k * 10 + 0.5, 150); c.stroke();
      c.beginPath(); c.moveTo(0, k * 10 + 0.5); c.lineTo(150, k * 10 + 0.5); c.stroke();
    }
    c.strokeStyle = '#fff'; c.lineWidth = 2; c.strokeRect(70, 70, 10, 10);
    c.strokeStyle = '#000'; c.lineWidth = 1; c.strokeRect(68.5, 68.5, 13, 13);
  };

  const move = (p: Pt) => { setPos(p); want.current = p; void pump(); };

  const pick = async (p: Pt) => {
    if (!rgb) return;
    const label = formatColor(rgb, format);
    try {
      await bridge.invoke('capture_pick', { ...p, label });
      await bridge.invoke('clipboard_write', { text: label });
    } finally {
      void close();
    }
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') return void close();
      if (!pos) return;
      const step = (e.shiftKey ? 10 : 1) / cfg.scale; // 1 piksel fisik
      const d: Record<string, Pt> = { ArrowLeft: { x: -step, y: 0 }, ArrowRight: { x: step, y: 0 }, ArrowUp: { x: 0, y: -step }, ArrowDown: { x: 0, y: step } };
      if (d[e.key]) { e.preventDefault(); move({ x: pos.x + d[e.key].x, y: pos.y + d[e.key].y }); }
      if (e.key === 'Enter') void pick(pos);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [pos, rgb, format]);

  const flipX = pos && pos.x + 190 > innerWidth;
  const flipY = pos && pos.y + 220 > innerHeight;
  return (
    <div class="ov ov--eye" onPointerMove={(e) => move({ x: e.clientX, y: e.clientY })} onClick={() => pos && void pick(pos)}>
      <div class="ov__hint">Klik untuk ambil warna · ←↑→↓ geser 1 px · Enter ambil · Esc batal</div>
      {pos && (
        <div class="loupe" style={`left:${flipX ? pos.x - 174 : pos.x + 24}px;top:${flipY ? pos.y - 204 : pos.y + 24}px`}>
          <canvas ref={canvas} width={150} height={150} />
          {rgb && (
            <div class="loupe__val" style={`background:rgb(${rgb.join(',')});color:${inkOn(rgb)}`}>{formatColor(rgb, format)}</div>
          )}
        </div>
      )}
    </div>
  );
}

function Ruler() {
  const [kind, setKind] = useState<'box' | 'line'>('box');
  const [a, setA] = useState<Pt | null>(null);
  const [b, setB] = useState<Pt | null>(null);
  const [cur, setCur] = useState<Pt>({ x: -10, y: -10 });
  const [drag, setDrag] = useState(false);
  const [copied, setCopied] = useState(false);
  const s = cfg.scale;
  const px = (v: number) => Math.round(v * s);

  const m = a && b ? measure(a, b) : null;
  function measure(p: Pt, q: Pt) {
    const dx = q.x - p.x, dy = q.y - p.y;
    return {
      w: Math.abs(px(dx)), h: Math.abs(px(dy)),
      len: Math.round(Math.hypot(dx, dy) * s), angle: Math.round((Math.atan2(-dy, dx) * 180) / Math.PI),
      dw: Math.round(Math.abs(dx)), dh: Math.round(Math.abs(dy)),
    };
  }
  const text = m ? (kind === 'box' ? `${m.w}×${m.h}` : `${m.len}px ${m.angle}°`) : '';
  const copy = () => { if (!text) return; bridge.invoke('clipboard_write', { text }).then(() => { setCopied(true); setTimeout(() => setCopied(false), 1200); }).catch(() => {}); };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const k = e.key.toLowerCase();
      if (k === 'escape') void close();
      else if (k === 'b') setKind('box');
      else if (k === 'l') setKind('line');
      else if (k === 'c') copy();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [text]);

  const box = a && b && { left: Math.min(a.x, b.x), top: Math.min(a.y, b.y), width: Math.abs(b.x - a.x), height: Math.abs(b.y - a.y) };
  return (
    <div
      class="ov ov--ruler"
      onPointerDown={(e) => { if ((e.target as HTMLElement).closest('.ov__bar')) return; const p = { x: e.clientX, y: e.clientY }; setA(p); setB(p); setDrag(true); (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId); }}
      onPointerMove={(e) => { const p = { x: e.clientX, y: e.clientY }; setCur(p); if (drag) setB(p); }}
      onPointerUp={() => setDrag(false)}
    >
      <i class="guide guide--h" style={`top:${cur.y}px`} />
      <i class="guide guide--v" style={`left:${cur.x}px`} />
      <div class="ov__bar" onPointerDown={(e) => e.stopPropagation()}>
        <button type="button" aria-pressed={kind === 'box'} onClick={() => setKind('box')}>▭ Kotak</button>
        <button type="button" aria-pressed={kind === 'line'} onClick={() => setKind('line')}>╱ Garis</button>
        <span class="ov__val">{m ? (kind === 'box' ? `${m.w} × ${m.h} px` : `${m.len} px · ${m.angle}°`) : 'Seret untuk mengukur'}</span>
        {m && s !== 1 && kind === 'box' && <span class="ov__dim">{m.dw} × {m.dh} DIP</span>}
        <button type="button" disabled={!m} onClick={copy}>{copied ? 'Tersalin' : 'Salin (C)'}</button>
        <button type="button" onClick={() => void close()}>Selesai</button>
      </div>
      {box && kind === 'box' && (
        <div class="rbox" style={`left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px`}>
          {m && <span class="rtag">{m.w} × {m.h}</span>}
        </div>
      )}
      {a && b && kind === 'line' && (
        <svg class="rline" width="100%" height="100%">
          <line x1={a.x} y1={a.y} x2={b.x} y2={b.y} />
          <circle cx={a.x} cy={a.y} r="3" /><circle cx={b.x} cy={b.y} r="3" />
          {m && <text x={(a.x + b.x) / 2 + 8} y={(a.y + b.y) / 2 - 8}>{m.len}px · Δ{m.w},{m.h} · {m.angle}°</text>}
        </svg>
      )}
    </div>
  );
}

function QrRegion() {
  const [a, setA] = useState<Pt | null>(null);
  const [b, setB] = useState<Pt | null>(null);
  const [msg, setMsg] = useState('Seret kotak di sekitar kode QR · Esc batal');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') void close(); };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const finish = async () => {
    if (!a || !b) return;
    const r = { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), w: Math.abs(b.x - a.x), h: Math.abs(b.y - a.y) };
    setBusy(true);
    try {
      const found = await bridge.invoke<string[]>('capture_qr', r);
      if (found.length) return void close('qr');
      setMsg('QR tidak ditemukan. Seret lagi lebih pas, atau Esc.');
    } catch (e) {
      setMsg(String(e));
    }
    setBusy(false);
    setA(null); setB(null);
  };

  const box = a && b && { left: Math.min(a.x, b.x), top: Math.min(a.y, b.y), width: Math.abs(b.x - a.x), height: Math.abs(b.y - a.y) };
  return (
    <div
      class={`ov ov--qr${box ? ' is-sel' : ''}`}
      onPointerDown={(e) => { if (busy) return; const p = { x: e.clientX, y: e.clientY }; setA(p); setB(p); (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId); }}
      onPointerMove={(e) => { if (a && !busy && e.buttons) setB({ x: e.clientX, y: e.clientY }); }}
      onPointerUp={() => void finish()}
    >
      <div class="ov__hint">{busy ? 'Memindai…' : msg}</div>
      {box && <div class="qbox" style={`left:${box.left}px;top:${box.top}px;width:${box.width}px;height:${box.height}px`} />}
    </div>
  );
}

const View = cfg.mode === 'eyedropper' ? Eyedropper : cfg.mode === 'qr' ? QrRegion : cfg.mode === 'draw' ? Draw : Ruler;
render(<View />, document.getElementById('app')!);
