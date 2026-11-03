import { useEffect, useRef, useState } from 'preact/hooks';
import { errText, llm } from '../../../core/llm';
import { buildSandboxMessage } from '../generate';

type Props = {
  /** Master prompt gabungan dalam format terpilih (kosong bila semua blok kosong). */
  prompt: string;
  onCopy: (text: string, label: string) => void;
};

// Uji master prompt langsung ke LLM. Hasil hanya di memori (sengaja tidak disimpan ke draft).
export function Sandbox({ prompt, onCopy }: Props) {
  const [input, setInput] = useState('');
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState('');
  const [error, setError] = useState('');
  const run = useRef(0); // generasi permintaan: hasil basi (dibatalkan/ditimpa) diabaikan
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; };
  }, []);

  const execute = async () => {
    if (!prompt || running) return;
    const id = ++run.current;
    setRunning(true); setError(''); setResult('');
    try {
      const out = await llm.complete('', buildSandboxMessage(prompt, input), 4096);
      if (alive.current && id === run.current) setResult(out);
    } catch (e) {
      if (alive.current && id === run.current) setError(errText(e));
    } finally {
      if (alive.current && id === run.current) setRunning(false);
    }
  };

  const cancel = () => { run.current++; setRunning(false); };

  return (
    <section class="anat__sandbox" aria-label="AI Test Sandbox">
      <div class="anat__title">AI Test Sandbox</div>
      <p class="anat__note">Uji master prompt di atas langsung ke model. Data uji bersifat opsional.</p>
      <textarea
        class="field anat__sbin"
        rows={2}
        value={input}
        maxLength={20_000}
        spellcheck={false}
        placeholder="Data / input uji (opsional), mis. teks yang akan diringkas"
        disabled={running}
        onInput={(e) => setInput((e.target as HTMLTextAreaElement).value)}
        onKeyDown={(e: KeyboardEvent) => { if (e.key === 'Escape') { e.stopPropagation(); (e.target as HTMLElement).blur(); } }}
      />
      <div class="anat__row">
        {running ? (
          <>
            <button type="button" class="btn" disabled aria-busy="true"><span class="anat__spin" aria-hidden="true" /> Menjalankan…</button>
            <button type="button" class="btn btn--ghost" onClick={cancel}>Batal</button>
          </>
        ) : (
          <button type="button" class="btn" onClick={execute} disabled={!prompt}>▶ Jalankan Prompt</button>
        )}
      </div>
      {error && <p class="anat__note err" role="alert">{error}</p>}
      {result && (
        <div class="anat__result">
          <div class="anat__row">
            <span class="anat__title">Hasil uji</span>
            <button type="button" class="anat__mini" onClick={() => onCopy(result, 'Hasil uji')}>Salin hasil</button>
          </div>
          <pre class="anat__pre">{result}</pre>
        </div>
      )}
    </section>
  );
}
