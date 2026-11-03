import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { useEffect, useMemo, useState } from 'preact/hooks';
import { bridge } from '../../core/bridge';
import { withHold } from '../../core/hold';
import { buildPayload, classify, contrast, TEMPLATES, type Fields, type Scanned, type Template } from './payload';
import './styles.css';

type Tab = 'make' | 'scan';

export default function QrModule() {
  const [tab, setTab] = useState<Tab>('make');
  const [results, setResults] = useState<{ items: Scanned[]; source: string } | null>(null);

  // Hasil pindai area layar (overlay) menunggu di Rust: tampilkan begitu modul dibuka.
  useEffect(() => {
    bridge.invoke<{ texts: string[]; source: string } | null>('qr_take_last')
      .then((r) => { if (r) { setResults({ items: r.texts.map(classify), source: r.source }); setTab('scan'); } })
      .catch(() => {});
  }, []);

  return (
    <div class="qr">
      <div class="qr__tabs" role="tablist">
        <button type="button" role="tab" aria-selected={tab === 'make'} onClick={() => setTab('make')}>Buat QR</button>
        <button type="button" role="tab" aria-selected={tab === 'scan'} onClick={() => setTab('scan')}>Pindai</button>
      </div>
      {tab === 'make' ? <Maker /> : <Scanner results={results} setResults={setResults} />}
    </div>
  );
}

function Maker() {
  const [t, setT] = useState<Template>('url');
  const [f, setF] = useState<Fields>({ security: 'WPA' });
  const [level, setLevel] = useState('M');
  const [fg, setFg] = useState('#000000');
  const [bg, setBg] = useState('#ffffff');
  const [quiet, setQuiet] = useState(true);
  const [svg, setSvg] = useState('');
  const [err, setErr] = useState('');
  const [note, setNote] = useState('');

  const payload = useMemo(() => buildPayload(t, f), [t, f]);
  const lowContrast = contrast(fg, bg) < 3;

  useEffect(() => {
    if (!payload) { setSvg(''); setErr(''); return; }
    const id = setTimeout(() => {
      bridge.invoke<string>('qr_generate', { text: payload, level, fg, bg, quiet })
        .then((s) => { setSvg(s); setErr(''); })
        .catch((e) => { setSvg(''); setErr(String(e)); });
    }, 120);
    return () => clearTimeout(id);
  }, [payload, level, fg, bg, quiet]);

  const set = (k: string) => (e: Event) => setF({ ...f, [k]: (e.target as HTMLInputElement).value });
  const input = (k: string, ph: string, type = 'text') => <input class="field" type={type} placeholder={ph} value={f[k] ?? ''} onInput={set(k)} />;

  const exportAs = async (format: 'svg' | 'png') => {
    setNote('');
    const path = await withHold(() => saveDialog({ title: 'Simpan QR', defaultPath: `qr.${format}`, filters: [{ name: format.toUpperCase(), extensions: [format] }] })).catch(() => null);
    if (!path) return;
    bridge.invoke('qr_export', { path, text: payload, level, fg, bg, quiet, format })
      .then(() => setNote(`Tersimpan: ${path}`))
      .catch((e) => setErr(String(e)));
  };

  return (
    <div class="qr__make">
      <div class="qr__chips">
        {TEMPLATES.map((x) => (
          <button key={x.id} type="button" aria-pressed={t === x.id} onClick={() => setT(x.id)}>{x.label}</button>
        ))}
      </div>
      <div class="qr__form">
        {t === 'url' && input('url', 'https://contoh.com')}
        {t === 'text' && <textarea class="field" rows={3} placeholder="Teks apa saja" value={f.text ?? ''} onInput={set('text')} />}
        {t === 'wifi' && (
          <>
            {input('ssid', 'Nama Wi-Fi (SSID)')}
            <div class="qr__row">
              <select class="field" value={f.security} onChange={set('security')}>
                <option value="WPA">WPA/WPA2/WPA3</option><option value="WEP">WEP</option><option value="nopass">Tanpa sandi</option>
              </select>
              {f.security !== 'nopass' && input('password', 'Kata sandi', 'password')}
            </div>
            <label class="qr__check"><input type="checkbox" checked={f.hidden === '1'} onChange={(e) => setF({ ...f, hidden: (e.target as HTMLInputElement).checked ? '1' : '' })} /> Jaringan tersembunyi</label>
          </>
        )}
        {t === 'email' && (<>{input('to', 'alamat@email.com', 'email')}{input('subject', 'Subjek (opsional)')}{input('body', 'Isi (opsional)')}</>)}
        {t === 'tel' && input('tel', '+62 812 …', 'tel')}
        {t === 'vcard' && (<>{input('name', 'Nama lengkap')}{input('org', 'Organisasi (opsional)')}<div class="qr__row">{input('tel', 'Telepon')}{input('email', 'Email', 'email')}</div></>)}
      </div>

      <div class="qr__out">
        <div class="qr__preview" style={`background:${bg}`}>
          {svg ? <img alt="Pratinjau QR" src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`} /> : <span class="muted">Isi data untuk melihat QR</span>}
        </div>
        <div class="qr__opts">
          <label>Koreksi
            <select class="field" value={level} onChange={(e) => setLevel((e.target as HTMLSelectElement).value)}>
              <option value="L">L · 7%</option><option value="M">M · 15%</option><option value="Q">Q · 25%</option><option value="H">H · 30%</option>
            </select>
          </label>
          <div class="qr__colors">
            <label>Depan<input type="color" value={fg} onInput={(e) => setFg((e.target as HTMLInputElement).value)} /></label>
            <label>Latar<input type="color" value={bg} onInput={(e) => setBg((e.target as HTMLInputElement).value)} /></label>
          </div>
          <label class="qr__check"><input type="checkbox" checked={quiet} onChange={(e) => setQuiet((e.target as HTMLInputElement).checked)} /> Margin (quiet zone)</label>
          <div class="qr__row">
            <button type="button" class="btn btn--ghost" disabled={!svg} onClick={() => void exportAs('svg')}>SVG</button>
            <button type="button" class="btn" disabled={!svg} onClick={() => void exportAs('png')}>PNG</button>
          </div>
        </div>
      </div>
      {lowContrast && <div class="qr__warn">Kontras warna rendah: QR bisa sulit dipindai.</div>}
      {err && <div class="err qr__msg">{err}</div>}
      {note && <div class="muted qr__msg">{note}</div>}
    </div>
  );
}

function Scanner({ results, setResults }: { results: { items: Scanned[]; source: string } | null; setResults: (r: { items: Scanned[]; source: string } | null) => void }) {
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState('');

  const run = async (source: string, job: () => Promise<string[]>) => {
    setBusy(true); setErr('');
    try { setResults({ items: (await job()).map(classify), source }); }
    catch (e) { setErr(String(e)); }
    setBusy(false);
  };

  const fromFile = async () => {
    const path = await withHold(() => openDialog({ multiple: false, directory: false, title: 'Pilih gambar QR', filters: [{ name: 'Gambar', extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif'] }] })).catch(() => null);
    if (typeof path === 'string') void run('file', () => bridge.invoke<string[]>('qr_scan_file', { path }));
  };
  const fromClipboard = () => void run('clipboard', () => bridge.invoke<string[]>('qr_scan_clipboard'));
  const fromScreen = () => { setErr(''); bridge.invoke('capture_start', { mode: 'qr' }).catch((e) => setErr(String(e))); };

  return (
    <div class="qr__scan">
      <div class="qr__srcs">
        <button type="button" class="qr__src" onClick={fromScreen} disabled={busy}><span>⛶</span>Area layar</button>
        <button type="button" class="qr__src" onClick={() => void fromFile()} disabled={busy}><span>🖼</span>Dari file</button>
        <button type="button" class="qr__src" onClick={fromClipboard} disabled={busy}><span>⧉</span>Clipboard</button>
      </div>
      {busy && <div class="muted">Memindai…</div>}
      {err && <div class="err qr__msg">{err}</div>}
      {results && (
        results.items.length === 0 ? (
          <div class="qr__none">Tidak ada QR yang terbaca dari {results.source}.</div>
        ) : (
          <div class="qr__results">{results.items.map((r, i) => <Result key={i} r={r} />)}</div>
        )
      )}
      {!results && !busy && <p class="muted qr__fine">Tautan tidak pernah dibuka otomatis: URL ditampilkan utuh dan harus dikonfirmasi. Data Wi-Fi hanya ditampilkan/disalin.</p>}
    </div>
  );
}

function Result({ r }: { r: Scanned }) {
  const [confirm, setConfirm] = useState(false);
  const [show, setShow] = useState(false);
  const [msg, setMsg] = useState('');
  const copy = (text: string, what = 'Tersalin') => bridge.invoke('clipboard_write', { text }).then(() => { setMsg(what); setTimeout(() => setMsg(''), 1300); });

  return (
    <div class="qr__res" data-kind={r.kind}>
      {r.kind === 'url' ? (
        <>
          <div class="qr__kind">Tautan {r.safe ? '' : '· skema tidak didukung'}</div>
          <div class="qr__url">{highlight(r.text, r.host)}</div>
          <div class="qr__row">
            <button type="button" class="btn btn--ghost" onClick={() => void copy(r.text)}>Salin</button>
            {r.safe && (confirm ? (
              <button type="button" class="btn" onClick={() => bridge.invoke('qr_open_url', { url: r.text }).catch((e) => setMsg(String(e)))}>Ya, buka {r.host}</button>
            ) : (
              <button type="button" class="btn" onClick={() => setConfirm(true)}>Buka…</button>
            ))}
          </div>
        </>
      ) : r.kind === 'wifi' ? (
        <>
          <div class="qr__kind">Wi-Fi</div>
          <div class="qr__kv"><span>Jaringan</span><strong>{r.ssid}</strong></div>
          <div class="qr__kv"><span>Keamanan</span><strong>{r.security}</strong></div>
          {r.password && <div class="qr__kv"><span>Sandi</span><strong>{show ? r.password : '•'.repeat(Math.min(12, r.password.length))}</strong></div>}
          <div class="qr__row">
            {r.password && <button type="button" class="btn btn--ghost" onClick={() => setShow((v) => !v)}>{show ? 'Sembunyikan' : 'Lihat'}</button>}
            {r.password && <button type="button" class="btn" onClick={() => void copy(r.password, 'Sandi tersalin')}>Salin sandi</button>}
          </div>
        </>
      ) : (
        <>
          <div class="qr__kind">{{ email: 'Email', tel: 'Telepon', vcard: 'Kontak', text: 'Teks' }[r.kind]}</div>
          <pre class="qr__text">{r.text}</pre>
          <div class="qr__row"><button type="button" class="btn btn--ghost" onClick={() => void copy(r.text)}>Salin</button></div>
        </>
      )}
      {msg && <div class="qr__msg muted">{msg}</div>}
    </div>
  );
}

/** URL utuh dengan nama domain disorot (mudah melihat domain palsu). */
function highlight(url: string, host: string) {
  const i = url.indexOf(host);
  if (i < 0) return url;
  return <>{url.slice(0, i)}<mark>{host}</mark>{url.slice(i + host.length)}</>;
}
