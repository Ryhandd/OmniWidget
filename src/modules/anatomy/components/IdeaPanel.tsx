import {
  LANGS, MAX_IDEA_CHARS, PERSONAS, PRESETS, SEGMENTS,
  isLangId, isPersonaId, type LangId, type PersonaId, type Preset,
} from '../segments';

type Props = {
  idea: string;
  persona: PersonaId;
  lang: LangId;
  loading: boolean;
  /** Memuat draft dari penyimpanan; kolom dikunci supaya tidak menimpa draft yang belum terbaca. */
  disabled: boolean;
  /** null = belum diketahui (status AI sedang dimuat atau bukan di Tauri). */
  aiReady: boolean | null;
  error: string;
  onIdea: (v: string) => void;
  onPersona: (v: PersonaId) => void;
  onLang: (v: LangId) => void;
  onClear: () => void;
  onPreset: (p: Preset) => void;
  onGenerate: () => void;
  onCancel: () => void;
};

// Kolom kiri: input ide, preset, kustomisasi, tombol generate, dan panduan anatomi.
export function IdeaPanel(p: Props) {
  const locked = p.disabled || p.loading;

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.stopPropagation();
      (e.target as HTMLElement).blur();
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      if (!locked) p.onGenerate();
    }
  };

  return (
    <div class="anat__panel">
      <section class="anat__sect">
        <div class="anat__row">
          <label class="anat__title" for="anat-idea">01. Ide / Prompt Singkat</label>
          <button type="button" class="anat__mini" onClick={p.onClear} disabled={locked || p.idea === ''}>Clear</button>
        </div>
        <textarea
          id="anat-idea"
          class="field anat__idea"
          rows={4}
          value={p.idea}
          maxLength={MAX_IDEA_CHARS}
          disabled={locked}
          spellcheck={false}
          placeholder="Mis. buatkan prompt singkat untuk membuat skrip game adventure godot 2d"
          onInput={(e) => p.onIdea((e.target as HTMLTextAreaElement).value)}
          onKeyDown={onKeyDown}
        />
      </section>

      <section class="anat__sect">
        <div class="anat__title">Contoh Ide Cepat</div>
        <div class="anat__chips">
          {PRESETS.map((pr) => (
            <button key={pr.label} type="button" class="anat__chip" disabled={locked} onClick={() => p.onPreset(pr)} title={pr.idea}>
              + {pr.label}
            </button>
          ))}
        </div>
      </section>

      <section class="anat__sect">
        <div class="anat__title">Pengaturan Kustomisasi</div>
        <div class="anat__selects">
          <label class="anat__lbl">
            <span>Target Persona</span>
            <select
              class="field"
              value={p.persona}
              disabled={locked}
              onChange={(e) => { const v = (e.target as HTMLSelectElement).value; if (isPersonaId(v)) p.onPersona(v); }}
            >
              {PERSONAS.map((x) => <option key={x.id} value={x.id}>{x.label}</option>)}
            </select>
          </label>
          <label class="anat__lbl">
            <span>Bahasa Prompt Master</span>
            <select
              class="field"
              value={p.lang}
              disabled={locked}
              onChange={(e) => { const v = (e.target as HTMLSelectElement).value; if (isLangId(v)) p.onLang(v); }}
            >
              {LANGS.map((x) => <option key={x.id} value={x.id}>{x.label}</option>)}
            </select>
          </label>
        </div>
      </section>

      <section class="anat__sect">
        {p.loading ? (
          <div class="anat__row">
            <button type="button" class="btn anat__go" disabled aria-busy="true">
              <span class="anat__spin" aria-hidden="true" /> Membuat…
            </button>
            <button type="button" class="btn btn--ghost" onClick={p.onCancel}>Batal</button>
          </div>
        ) : (
          <button type="button" class="btn anat__go" onClick={p.onGenerate} disabled={p.disabled || p.idea.trim() === ''} title="Generate (Ctrl+Enter)">
            ✨ Generate Master Prompt
          </button>
        )}
        {p.aiReady === false && (
          <p class="anat__note" role="status">AI belum diatur. Buka tab Pengaturan → AI untuk mengisi model dan API key.</p>
        )}
        {p.error && <p class="anat__note err" role="alert">{p.error}</p>}
      </section>

      <section class="anat__guide" aria-label="Panduan anatomi prompt">
        <div class="anat__title">Anatomi Prompt Claude</div>
        <div class="anat__legend">
          {SEGMENTS.map((s) => (
            <span key={s.id} class="anat__tag" data-seg={s.id} title={s.hint}>
              <i class="anat__dot" aria-hidden="true" />{s.label}
            </span>
          ))}
        </div>
        <p class="anat__note">
          Prompt terbaik berurutan: siapa AI-nya (Role), apa tugasnya (Task), bahan yang diketahui (Context), cara menilai
          kualitas (Reasoning), kapan berhenti atau bertanya (Stop conditions), dan bentuk jawaban (Output).
        </p>
      </section>
    </div>
  );
}
