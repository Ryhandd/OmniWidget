// Definisi enam blok anatomi prompt Claude. Data-driven: draft, komposisi, dan kartu UI mengikuti
// SEGMENTS. Warna aksen sengaja TIDAK ada di sini: ada di styles.css lewat [data-seg] (satu tempat).

export const SEGMENT_IDS = ['role', 'task', 'context', 'reasoning', 'stop_conditions', 'output'] as const;
export type SegmentId = (typeof SEGMENT_IDS)[number];
export type Fields = Record<SegmentId, string>;

export type Segment = {
  id: SegmentId;
  /** Judul kartu, label format teks biasa, dan heading Markdown (huruf besar). */
  label: string;
  /** Nama tag untuk format XML (huruf kecil, tanpa spasi). */
  tag: string;
  /** Keterangan singkat di samping judul. */
  subtitle: string;
  /** Teks panduan di bawah judul kartu (juga tooltip). */
  hint: string;
  placeholder: string;
};

export const SEGMENTS: readonly Segment[] = [
  {
    id: 'role', label: 'Role', tag: 'role', subtitle: 'Peran & Mindset',
    hint: 'Persona, spesialisasi, dan mindset yang harus diadopsi AI.',
    placeholder: 'Mis. Editor teknis senior yang teliti, ringkas, dan berpikir kritis.',
  },
  {
    id: 'task', label: 'Task', tag: 'task', subtitle: 'Tugas Utama',
    hint: 'Tugas utama dan luaran inti yang harus dihasilkan.',
    placeholder: 'Mis. Ringkas bagian di bawah menjadi 5 poin utama.',
  },
  {
    id: 'context', label: 'Context', tag: 'context', subtitle: 'Latar Belakang & Detail',
    hint: 'Latar belakang, fakta, dan asumsi yang perlu diketahui AI.',
    placeholder: 'Mis. Laporan kuliah tentang asisten AI akademik; pembacanya dosen pembimbing.',
  },
  {
    id: 'reasoning', label: 'Reasoning', tag: 'reasoning', subtitle: 'Alasan Strategis & Kualitas',
    hint: 'Alasan strategis dan kriteria kualitas untuk menilai hasil.',
    placeholder: 'Mis. Utamakan akurasi di atas kelengkapan; pertimbangkan pembaca non-teknis.',
  },
  {
    id: 'stop_conditions', label: 'Stop conditions', tag: 'stop_conditions', subtitle: 'Kriteria Berhenti',
    hint: 'Kapan AI harus berhenti atau bertanya, dan syarat wajib yang harus dipenuhi.',
    placeholder: 'Mis. Jika data tidak cukup, ajukan pertanyaan klarifikasi; jangan mengarang sumber.',
  },
  {
    id: 'output', label: 'Output', tag: 'output', subtitle: 'Format Luaran',
    hint: 'Format dan struktur luaran yang diinginkan.',
    placeholder: 'Mis. Bahasa Indonesia formal, maksimal 150 kata, dalam bentuk poin bernomor.',
  },
];

/** Batas per blok. 6 blok × 10.000 karakter (≤ 3 byte/karakter) + ide tetap < batas nilai kv (256 KB). */
export const MAX_FIELD_CHARS = 10_000;
export const MAX_IDEA_CHARS = 4_000;

export const emptyFields = (): Fields =>
  Object.fromEntries(SEGMENT_IDS.map((id) => [id, ''])) as Fields;

// ---------- Persona, bahasa, preset ----------

export const PERSONAS = [
  { id: 'general', label: 'Umum / Serbaguna', brief: 'a general professional audience that values clarity and practicality' },
  { id: 'founder', label: 'Founder / Eksekutif', brief: 'a startup founder or executive who needs decisive, strategic, outcome-oriented output' },
  { id: 'engineer', label: 'Senior Engineer / Developer', brief: 'a senior software engineer who needs precise, technically rigorous output' },
  { id: 'marketer', label: 'Growth Marketer', brief: 'a growth marketer who needs conversion-focused, data-informed output' },
  { id: 'product', label: 'Product Manager', brief: 'a product manager who needs user-centered, prioritized, actionable output' },
  { id: 'analyst', label: 'Analis Riset / Strategi', brief: 'a research and strategy analyst who needs evidence-based, well-structured output' },
  { id: 'writer', label: 'Penulis / Content Strategist', brief: 'a writer or content strategist who needs audience-aware, well-crafted output' },
] as const;
export type PersonaId = (typeof PERSONAS)[number]['id'];
export const DEFAULT_PERSONA: PersonaId = 'general';
export const isPersonaId = (v: unknown): v is PersonaId => PERSONAS.some((p) => p.id === v);

export const LANGS = [
  { id: 'id', label: 'Bahasa Indonesia', name: 'Indonesian (Bahasa Indonesia)' },
  { id: 'en', label: 'English', name: 'English' },
] as const;
export type LangId = (typeof LANGS)[number]['id'];
export const DEFAULT_LANG: LangId = 'id';
export const isLangId = (v: unknown): v is LangId => LANGS.some((l) => l.id === v);

export type Preset = { label: string; persona: PersonaId; idea: string };

export const PRESETS: readonly Preset[] = [
  {
    label: 'Launch Plan SaaS', persona: 'founder',
    idea: 'Buat launch plan 90 hari untuk produk SaaS B2B: positioning, kanal akuisisi, target metrik, dan risiko utama.',
  },
  {
    label: 'Code Refactor & Audit', persona: 'engineer',
    idea: 'Audit dan refactor modul kode legacy: temukan bug, code smell, dan celah keamanan, lalu susun rencana refactor bertahap yang aman.',
  },
  {
    label: 'Copywriting Landing Page', persona: 'marketer',
    idea: 'Tulis copywriting landing page untuk produk digital: headline, subheadline, manfaat utama, bukti sosial, dan CTA.',
  },
  {
    label: 'Riset Kompetitor Pasar', persona: 'analyst',
    idea: 'Riset kompetitor pasar: bandingkan 5 pesaing utama dari harga, fitur, positioning, kekuatan, dan celah peluang.',
  },
];
