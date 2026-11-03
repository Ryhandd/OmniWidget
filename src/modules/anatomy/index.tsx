import { useEffect, useMemo, useRef, useState } from 'preact/hooks';
import { copyText } from '../../core/clipboard';
import { errText, llm } from '../../core/llm';
import { FormatToggle } from './components/FormatToggle';
import { IdeaPanel } from './components/IdeaPanel';
import { Sandbox } from './components/Sandbox';
import { SegmentField } from './components/SegmentField';
import { Toast, useToast } from './components/Toast';
import { compose, composeSegment, estimateTokens, filledCount } from './compose';
import { isBlank } from './draft';
import { buildGeneratorRequest, parseGenerated, type GenerateInput } from './generate';
import { emptyFields, SEGMENTS, type Fields, type Preset, type SegmentId } from './segments';
import { useDraft, type SaveState } from './useDraft';
import './styles.css';

const SAVE_LABEL: Record<SaveState, string> = {
  idle: '',
  pending: 'Menyimpan…',
  saved: 'Draft tersimpan',
  error: 'Gagal menyimpan draft',
};

export default function AnatomyModule() {
  const { draft, ready, saveState, setField, setFields, setFormat, setIdea, setPersona, setLang, getDraft } = useDraft();
  const { toast, show } = useToast();

  const [busy, setBusy] = useState(false);
  const [genError, setGenError] = useState('');
  const [aiReady, setAiReady] = useState<boolean | null>(null);
  const [undo, setUndo] = useState<Fields | null>(null); // isi blok sebelum Generate/Kosongkan, untuk Urungkan

  const alive = useRef(true);
  const run = useRef(0); // generasi permintaan: jawaban basi (dibatalkan) diabaikan
  const results = useRef<HTMLDivElement>(null);

  useEffect(() => {
    alive.current = true;
    // Dibaca ulang tiap modul dibuka, jadi pengaturan AI yang baru diisi langsung terdeteksi.
    llm.status().then((s) => { if (alive.current) setAiReady(s.configured); }).catch(() => {});
    return () => { alive.current = false; run.current++; };
  }, []);

  const output = useMemo(() => compose(draft.fields, draft.format), [draft.fields, draft.format]);
  const filled = filledCount(draft.fields);
  const empty = output === '';

  const copy = async (text: string, label: string) => {
    if (!text) return;
    (await copyText(text)) ? show(`${label} tersalin`) : show('Gagal menyalin ke clipboard', 'err');
  };
  const copyAll = () => copy(output, 'Master prompt');

  const onField = (id: SegmentId, value: string) => {
    setUndo(null);
    setField(id, value);
  };

  const onClearBlocks = () => {
    setUndo(draft.fields);
    setFields(emptyFields());
  };
  const onUndo = () => {
    if (!undo) return;
    setFields(undo);
    setUndo(null);
  };

  const generate = async (override?: GenerateInput) => {
    const input = override ?? { idea: draft.idea, persona: draft.persona, lang: draft.lang };
    if (input.idea.trim() === '' || busy) return;
    const id = ++run.current;
    setBusy(true);
    setGenError('');
    try {
      const { system, user } = buildGeneratorRequest(input);
      const raw = await llm.complete(system, user, 4096);
      if (!alive.current || id !== run.current) return;
      const parsed = parseGenerated(raw);
      if (!parsed) throw 'Jawaban AI tidak bisa dibaca sebagai enam blok. Coba Generate lagi.';

      const prev = getDraft().fields; // dibaca setelah await: pengguna mungkin sempat mengedit
      setUndo(isBlank(prev) ? null : prev);
      setFields(parsed);
      setAiReady(true);
      show('Master prompt dibuat');
      // Pada tampilan sempit hasil ada di bawah form: bawa ke layar.
      const calm = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
      results.current?.scrollIntoView({ block: 'start', behavior: calm ? 'auto' : 'smooth' });
    } catch (e) {
      if (alive.current && id === run.current) setGenError(errText(e));
    } finally {
      if (alive.current && id === run.current) setBusy(false);
    }
  };

  const cancelGenerate = () => {
    run.current++;
    setBusy(false);
  };

  const onPreset = (p: Preset) => {
    // Nilai dipakai langsung (bukan dari state) agar Generate tidak membaca ide lama.
    setIdea(p.idea);
    setPersona(p.persona);
    void generate({ idea: p.idea, persona: p.persona, lang: draft.lang });
  };

  return (
    <div class="anat">
      <IdeaPanel
        idea={draft.idea}
        persona={draft.persona}
        lang={draft.lang}
        loading={busy}
        disabled={!ready}
        aiReady={aiReady}
        error={genError}
        onIdea={setIdea}
        onPersona={setPersona}
        onLang={setLang}
        onClear={() => { setIdea(''); setGenError(''); }}
        onPreset={onPreset}
        onGenerate={() => void generate()}
        onCancel={cancelGenerate}
      />

      <div class="anat__results" ref={results} aria-busy={busy}>
        <div class="anat__bar">
          <FormatToggle value={draft.format} onChange={setFormat} disabled={!ready} />
          <span class={`anat__save ${saveState === 'error' ? 'err' : 'muted'}`} role="status" aria-live="polite">
            {SAVE_LABEL[saveState]}
          </span>
          {undo ? (
            <button type="button" class="btn btn--ghost" onClick={onUndo}>Urungkan</button>
          ) : (
            <button type="button" class="btn btn--ghost" onClick={onClearBlocks} disabled={!ready || isBlank(draft.fields)}>
              Kosongkan
            </button>
          )}
          <button type="button" class="btn" onClick={copyAll} disabled={empty} title="Salin semua (Ctrl+Enter di dalam kolom)">
            Salin Semua
          </button>
        </div>

        {SEGMENTS.map((seg) => (
          <SegmentField
            key={seg.id}
            segment={seg}
            value={draft.fields[seg.id]}
            disabled={!ready}
            onInput={(v) => onField(seg.id, v)}
            onCopy={() => copy(composeSegment(seg, draft.fields, draft.format), seg.label)}
            onSubmit={copyAll}
          />
        ))}

        <details class="anat__preview">
          <summary>
            Pratinjau <span class="muted">· {filled}/{SEGMENTS.length} blok · ≈{estimateTokens(output)} token</span>
          </summary>
          {empty ? <p class="muted anat__empty">Isi salah satu blok untuk melihat hasilnya.</p> : <pre class="anat__pre">{output}</pre>}
        </details>

        <Sandbox prompt={output} onCopy={copy} />
      </div>

      <Toast toast={toast} />
    </div>
  );
}
