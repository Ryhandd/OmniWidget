import { useEffect, useState } from 'preact/hooks';
import { errText, llm, type LlmProvider, type LlmStatus } from '../../core/llm';

// Penyedia siap pakai. Nama model sengaja tidak diisi otomatis: daftar model cepat berubah, isi sendiri.
const PRESETS: { id: string; name: string; provider: LlmProvider; baseUrl: string }[] = [
  { id: 'openai', name: 'OpenAI', provider: 'openai', baseUrl: 'https://api.openai.com/v1' },
  { id: 'anthropic', name: 'Anthropic (Claude)', provider: 'anthropic', baseUrl: 'https://api.anthropic.com' },
  { id: 'gemini', name: 'Google Gemini', provider: 'openai', baseUrl: 'https://generativelanguage.googleapis.com/v1beta/openai' },
  { id: 'openrouter', name: 'OpenRouter', provider: 'openai', baseUrl: 'https://openrouter.ai/api/v1' },
  { id: 'groq', name: 'Groq', provider: 'openai', baseUrl: 'https://api.groq.com/openai/v1' },
  { id: 'ollama', name: 'Ollama (lokal)', provider: 'openai', baseUrl: 'http://localhost:11434/v1' },
];
const CUSTOM = 'custom';

const presetOf = (provider: LlmProvider, baseUrl: string) =>
  PRESETS.find((p) => p.provider === provider && p.baseUrl === baseUrl.trim().replace(/\/+$/, ''))?.id ?? CUSTOM;

export function AiSection() {
  const [status, setStatus] = useState<LlmStatus | null>(null);
  const [provider, setProvider] = useState<LlmProvider>('openai');
  const [baseUrl, setBaseUrl] = useState('');
  const [model, setModel] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState('');
  const [bad, setBad] = useState(false);

  const adopt = (s: LlmStatus) => {
    setStatus(s); setProvider(s.provider); setBaseUrl(s.baseUrl); setModel(s.model); setApiKey('');
  };

  useEffect(() => {
    llm.status().then(adopt).catch((e) => { setBad(true); setNote(errText(e)); });
  }, []);

  const say = (text: string, isBad = false) => { setNote(text); setBad(isBad); };

  const save = async (keyArg?: string) => {
    setBusy(true); say('');
    try {
      adopt(await llm.save({ provider, baseUrl, model, apiKey: keyArg ?? (apiKey.trim() === '' ? undefined : apiKey.trim()) }));
      say(keyArg === '' ? 'API key dihapus.' : 'Tersimpan.');
    } catch (e) {
      say(errText(e), true);
    } finally {
      setBusy(false);
    }
  };

  const test = async () => {
    setBusy(true); say('Menguji koneksi…');
    try {
      const out = await llm.complete('', 'Balas hanya dengan satu kata: OK', 32);
      say(`Terhubung. Balasan model: ${out.slice(0, 60)}`);
    } catch (e) {
      say(errText(e), true);
    } finally {
      setBusy(false);
    }
  };

  const pickPreset = (id: string) => {
    const p = PRESETS.find((x) => x.id === id);
    if (p) { setProvider(p.provider); setBaseUrl(p.baseUrl); }
  };

  const dirty = !!status && (provider !== status.provider || baseUrl.trim().replace(/\/+$/, '') !== status.baseUrl || model.trim() !== status.model || apiKey.trim() !== '');

  return (
    <div class="set__form">
      <label class="set__lbl">
        <span>Penyedia</span>
        <select class="field" value={presetOf(provider, baseUrl)} onChange={(e) => pickPreset((e.target as HTMLSelectElement).value)} disabled={busy}>
          {PRESETS.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
          <option value={CUSTOM}>Kustom (kompatibel OpenAI)</option>
        </select>
      </label>
      <label class="set__lbl">
        <span>Base URL</span>
        <input class="field" type="url" value={baseUrl} spellcheck={false} disabled={busy} placeholder="https://…"
          onInput={(e) => setBaseUrl((e.target as HTMLInputElement).value)} />
      </label>
      <label class="set__lbl">
        <span>Model</span>
        <input class="field" type="text" value={model} spellcheck={false} disabled={busy} placeholder="Nama model dari penyedia"
          onInput={(e) => setModel((e.target as HTMLInputElement).value)} />
      </label>
      <label class="set__lbl">
        <span>API key {status?.hasKey && <span style={{ color: 'var(--accent-2)' }}>· tersimpan</span>}</span>
        <input class="field" type="password" autocomplete="off" value={apiKey} spellcheck={false} disabled={busy}
          placeholder={status?.hasKey ? 'Kosongkan untuk tetap memakai key tersimpan' : 'Tempel API key (Ollama lokal tidak perlu)'}
          onInput={(e) => setApiKey((e.target as HTMLInputElement).value)} />
      </label>

      <div class="set__row">
        <button type="button" class="btn" onClick={() => save()} disabled={busy || !dirty}>Simpan</button>
        <button type="button" class="btn btn--ghost" onClick={test} disabled={busy || dirty || !status?.configured}>Tes koneksi</button>
        {status?.hasKey && <button type="button" class="btn btn--ghost" onClick={() => save('')} disabled={busy}>Hapus key</button>}
      </div>
      {note && <div class={`set__note ${bad ? 'err' : 'muted'}`} role="status">{note}</div>}
      <div class="set__note muted">
        Key disimpan lokal di database aplikasi (tidak dienkripsi), hanya dibaca backend Rust, dan tidak pernah dikirim balik ke
        antarmuka. Permintaan AI dikirim langsung ke penyedia yang Anda pilih; teks prompt Anda ikut terkirim ke sana.
      </div>
    </div>
  );
}
