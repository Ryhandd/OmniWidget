import { useEffect, useRef, useState } from 'preact/hooks';
import { bridge } from '../core/bridge';
import {
  arrowHead,
  boxOf,
  clampSize,
  fontSize,
  isDark,
  MAX_SIZE,
  MIN_SIZE,
  moved,
  PALETTE,
  type Pt,
  type Tool,
} from './draw';

type Shape = {
  tool: Tool;
  color: string;
  size: number;
  a: Pt;
  b?: Pt;
  pts?: Pt[];
  text?: string;
};

const TOOLS: Array<{ id: Tool; label: string; icon: string }> = [
  { id: 'pen', label: 'Pena', icon: 'M4 20l4-1L18 9l-3-3L5 16zM14 5l3 3' },
  { id: 'marker', label: 'Stabilo', icon: 'M4 20h6M6 17l9-9 3 3-9 9z' },
  { id: 'eraser', label: 'Penghapus', icon: 'M7 17l-3-3 8-8 6 6-5 5zM4 21h16' },
  { id: 'line', label: 'Garis', icon: 'M4 20L20 4' },
  { id: 'arrow', label: 'Panah', icon: 'M4 20L20 4M12 4h8v8' },
  { id: 'rect', label: 'Kotak', icon: 'M4 6h16v12H4z' },
  { id: 'text', label: 'Teks', icon: 'M5 5h14M12 5v14M9 19h6' },
];

const close = () => bridge.invoke('capture_close', {}).catch(() => window.close());

function paint(ctx: CanvasRenderingContext2D, s: Shape) {
  ctx.save();
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  if (s.tool === 'eraser') {
    ctx.globalCompositeOperation = 'destination-out';
    ctx.strokeStyle = '#000';
    ctx.fillStyle = '#000';
    ctx.lineWidth = s.size * 3;
  } else {
    ctx.strokeStyle = s.color;
    ctx.fillStyle = s.color;
    ctx.lineWidth = s.tool === 'marker' ? Math.max(10, s.size * 3) : s.size;
    if (s.tool === 'marker') ctx.globalAlpha = 0.32;
  }

  if (s.tool === 'pen' || s.tool === 'marker' || s.tool === 'eraser') {
    const pts = s.pts ?? [];
    if (pts.length === 1) {
      ctx.beginPath();
      ctx.arc(pts[0].x, pts[0].y, ctx.lineWidth / 2, 0, Math.PI * 2);
      ctx.fill();
    } else if (pts.length > 1) {
      ctx.beginPath();
      ctx.moveTo(pts[0].x, pts[0].y);
      for (let i = 1; i < pts.length; i++) ctx.lineTo(pts[i].x, pts[i].y);
      ctx.stroke();
    }
  } else if (s.tool === 'line' || s.tool === 'arrow') {
    const b = s.b ?? s.a;
    ctx.beginPath();
    ctx.moveTo(s.a.x, s.a.y);
    ctx.lineTo(b.x, b.y);
    ctx.stroke();
    if (s.tool === 'arrow') {
      const head = arrowHead(s.a, b, s.size);
      ctx.beginPath();
      ctx.moveTo(head[0].x, head[0].y);
      ctx.lineTo(head[1].x, head[1].y);
      ctx.lineTo(head[2].x, head[2].y);
      ctx.closePath();
      ctx.fill();
    }
  } else if (s.tool === 'rect') {
    const r = boxOf(s.a, s.b ?? s.a);
    ctx.strokeRect(r.left, r.top, r.width, r.height);
  } else if (s.tool === 'text' && s.text) {
    ctx.font = `700 ${fontSize(s.size)}px 'Segoe UI', system-ui, sans-serif`;
    ctx.textBaseline = 'top';
    ctx.fillText(s.text, s.a.x, s.a.y);
  }
  ctx.restore();
}

export default function Draw() {
  const main = useRef<HTMLCanvasElement>(null);
  const base = useRef<HTMLCanvasElement | null>(null);
  const shapes = useRef<Shape[]>([]);
  const redoStack = useRef<Shape[]>([]);
  const cur = useRef<Shape | null>(null);
  const [tool, setTool] = useState<Tool>('pen');
  const [color, setColor] = useState<string>(PALETTE[0]);
  const [size, setSize] = useState(6);
  const [rev, setRev] = useState(0);
  const [status, setStatus] = useState('');
  const [draft, setDraft] = useState<{ p: Pt; value: string } | null>(null);
  const draftRef = useRef<typeof draft>(null);
  draftRef.current = draft;

  const canvasCtx = () => main.current?.getContext('2d') ?? null;
  const baseCtx = () => base.current?.getContext('2d') ?? null;

  /** Salin gambar coretan yang sudah jadi ke canvas utama (tanpa coretan sementara). */
  const blit = () => {
    const c = main.current;
    const b = base.current;
    const ctx = canvasCtx();
    if (!c || !b || !ctx) return;
    const dpr = devicePixelRatio || 1;
    ctx.save();
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, c.width, c.height);
    ctx.restore();
    ctx.drawImage(b, 0, 0, c.width / dpr, c.height / dpr);
  };

  const preview = () => {
    blit();
    const ctx = canvasCtx();
    if (ctx && cur.current) paint(ctx, cur.current);
  };

  const rebuild = () => {
    const b = base.current;
    const bx = baseCtx();
    if (!b || !bx) return;
    bx.save();
    bx.setTransform(1, 0, 0, 1, 0, 0);
    bx.clearRect(0, 0, b.width, b.height);
    bx.restore();
    for (const s of shapes.current) paint(bx, s);
    blit();
  };

  const undo = () => {
    const s = shapes.current.pop();
    if (s) redoStack.current.push(s);
    rebuild();
    setRev((v) => v + 1);
  };

  const redo = () => {
    const s = redoStack.current.pop();
    if (!s) return;
    shapes.current.push(s);
    const bx = baseCtx();
    if (bx) paint(bx, s);
    blit();
    setRev((v) => v + 1);
  };

  const clear = () => {
    shapes.current = [];
    redoStack.current = [];
    cur.current = null;
    rebuild();
    setRev((v) => v + 1);
  };

  // Siapkan canvas fisik (DPI-aware) dan ulangi saat jendela berubah ukuran.
  useEffect(() => {
    const setup = () => {
      const c = main.current;
      if (!c) return;
      const dpr = devicePixelRatio || 1;
      c.width = Math.round(innerWidth * dpr);
      c.height = Math.round(innerHeight * dpr);
      const b = base.current ?? document.createElement('canvas');
      base.current = b;
      b.width = c.width;
      b.height = c.height;
      const ctx = c.getContext('2d');
      const bx = b.getContext('2d');
      if (!ctx || !bx) return;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      bx.setTransform(dpr, 0, 0, dpr, 0, 0);
      rebuild();
    };
    setup();
    window.addEventListener('resize', setup);
    return () => window.removeEventListener('resize', setup);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.ctrlKey || e.metaKey;
      if (e.key === 'Escape') {
        if (draftRef.current) { cancelText(); return; }
        void close();
      } else if (mod && e.key.toLowerCase() === 'z') {
        e.preventDefault();
        if (e.shiftKey) redo(); else undo();
      } else if (mod && e.key.toLowerCase() === 'y') {
        e.preventDefault();
        redo();
      } else if (e.key === '[') {
        setSize((v) => clampSize(v - 2));
      } else if (e.key === ']') {
        setSize((v) => clampSize(v + 2));
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const point = (e: PointerEvent): Pt => ({ x: e.clientX, y: e.clientY });

  const down = (e: PointerEvent) => {
    if (e.button !== 0) return;
    const p = point(e);
    if (tool === 'text') {
      setDraft({ p, value: '' });
      return;
    }
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    cur.current = { tool, color, size, a: p, b: p, pts: [p] };
    preview();
  };

  const move = (e: PointerEvent) => {
    const s = cur.current;
    if (!s) return;
    const p = point(e);
    s.b = p;
    if (s.pts) {
      const last = s.pts[s.pts.length - 1];
      if (moved(last, p, s.tool === 'eraser' ? 1.5 : 0.75)) s.pts.push(p);
    }
    preview();
  };

  const up = () => {
    const s = cur.current;
    if (!s) return;
    cur.current = null;
    shapes.current.push(s);
    redoStack.current = [];
    const bx = baseCtx();
    if (bx) paint(bx, s);
    blit();
    setRev((v) => v + 1);
  };

  const commitText = () => {
    const d = draftRef.current;
    if (!d) return;
    draftRef.current = null;
    const text = d.value.trim();
    setDraft(null);
    if (!text) return;
    const s: Shape = { tool: 'text', color, size, a: d.p, text };
    shapes.current.push(s);
    redoStack.current = [];
    const bx = baseCtx();
    if (bx) paint(bx, s);
    blit();
    setRev((v) => v + 1);
  };

  const cancelText = () => {
    draftRef.current = null;
    setDraft(null);
  };

  const copy = async () => {
    const c = main.current;
    if (!c) return;
    setStatus('Menyalin…');
    try {
      preview();
      await bridge.invoke('capture_annotate', { png: c.toDataURL('image/png') });
      setStatus('Tersalin ke clipboard');
    } catch (e) {
      setStatus(String(e));
    }
    setTimeout(() => setStatus(''), 2200);
  };

  return (
    <div class="drw" style={`--drw-color:${color}`}>
      <canvas
        ref={main}
        class="drw__canvas"
        onPointerDown={down}
        onPointerMove={move}
        onPointerUp={up}
        onPointerCancel={up}
      />

      {draft && (
        <input
          class="drw__text"
          style={`left:${draft.p.x}px;top:${draft.p.y}px;color:${color};font-size:${fontSize(size)}px;background:${
            isDark(color) ? 'rgba(255,255,255,0.94)' : 'rgba(11,12,15,0.92)'
          }`}
          value={draft.value}
          autoFocus
          placeholder="Tulis…"
          onInput={(e) => setDraft({ p: draft.p, value: (e.currentTarget as HTMLInputElement).value })}
          onKeyDown={(e) => {
            if (e.key === 'Enter') { e.preventDefault(); commitText(); }
            else if (e.key === 'Escape') { e.stopPropagation(); cancelText(); }
          }}
          onBlur={commitText}
        />
      )}

      <div class="drw__bar" role="toolbar" aria-label="Alat coret layar" data-rev={rev}>
        <div class="drw__colors" aria-label="Warna">
          {PALETTE.map((c) => (
            <button
              key={c}
              type="button"
              class="drw__sw"
              style={`background:${c}`}
              aria-label={`Warna ${c}`}
              aria-pressed={color === c}
              onClick={() => setColor(c)}
            />
          ))}
        </div>

        <div class="drw__sep" />

        <div class="drw__tools">
          {TOOLS.map((t) => (
            <button
              key={t.id}
              type="button"
              class="drw__btn"
              title={t.label}
              aria-label={t.label}
              aria-pressed={tool === t.id}
              onClick={() => setTool(t.id)}
            >
              <svg viewBox="0 0 24 24"><path d={t.icon} /></svg>
            </button>
          ))}
        </div>

        <div class="drw__sep" />

        <div class="drw__tools">
          <button type="button" class="drw__btn" title="Urungkan (Ctrl+Z)" aria-label="Urungkan" disabled={!shapes.current.length} onClick={undo}>
            <svg viewBox="0 0 24 24"><path d="M9 7L4 12l5 5M4 12h10a5 5 0 015 5v1" /></svg>
          </button>
          <button type="button" class="drw__btn" title="Ulangi (Ctrl+Shift+Z)" aria-label="Ulangi" disabled={!redoStack.current.length} onClick={redo}>
            <svg viewBox="0 0 24 24"><path d="M15 7l5 5-5 5M20 12H10a5 5 0 00-5 5v1" /></svg>
          </button>
          <button type="button" class="drw__btn" title="Hapus semua" aria-label="Hapus semua coretan" onClick={clear}>
            <svg viewBox="0 0 24 24"><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" /></svg>
          </button>
          <button type="button" class="drw__btn" title="Salin hasil + screenshot" aria-label="Salin hasil" onClick={() => void copy()}>
            <svg viewBox="0 0 24 24"><path d="M9 9h10v10H9zM5 15V5h10" /></svg>
          </button>
        </div>

        <div class="drw__sep" />

        <label class="drw__size">
          <span>{size}px</span>
          <input
            type="range"
            min={MIN_SIZE}
            max={MAX_SIZE}
            step={1}
            value={size}
            aria-label="Ketebalan"
            onInput={(e) => setSize(clampSize(Number((e.currentTarget as HTMLInputElement).value)))}
          />
        </label>

        <button type="button" class="drw__action drw__action--main" onClick={() => void copy()}>Salin</button>
        <button type="button" class="drw__action" onClick={() => void close()}>Selesai</button>
      </div>

      {status && <div class="drw__toast">{status}</div>}
      <div class="drw__hint">
        Seret untuk menggambar · Ctrl+Z urungkan · [ ] ketebalan · Esc selesai
      </div>
    </div>
  );
}
