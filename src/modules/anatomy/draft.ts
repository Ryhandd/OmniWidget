import { DEFAULT_FORMAT, isOutputFormat, type OutputFormat } from './compose';
import {
  DEFAULT_LANG, DEFAULT_PERSONA, MAX_FIELD_CHARS, MAX_IDEA_CHARS, SEGMENT_IDS,
  emptyFields, isLangId, isPersonaId, type Fields, type LangId, type PersonaId,
} from './segments';

export const DRAFT_KEY = 'anatomy.draft';

export type Draft = {
  v: 2;
  idea: string;
  persona: PersonaId;
  lang: LangId;
  fields: Fields;
  format: OutputFormat;
};

export const emptyDraft = (): Draft => ({
  v: 2,
  idea: '',
  persona: DEFAULT_PERSONA,
  lang: DEFAULT_LANG,
  fields: emptyFields(),
  format: DEFAULT_FORMAT,
});

export const serializeDraft = (d: Draft): string => JSON.stringify(d);

// Karakter kontrol (selain \t \n) membengkak 6 byte saat di-escape JSON; buang agar batas kv tetap terjamin.
// eslint-disable-next-line no-control-regex
const CONTROL = /[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F]/g;
const clean = (s: string, max: number) => s.replace(CONTROL, '').slice(0, max);

/**
 * Baca draft dari penyimpanan secara defensif: data rusak, lama, atau diedit manual
 * tidak boleh membuat modul crash; yang tidak valid diganti nilai kosong/default.
 * Draft v1 (4 segmen: role/context/task/constraints) dimigrasi; "constraints" menjadi "stop_conditions".
 */
export function parseDraft(raw: string | null | undefined): Draft {
  const draft = emptyDraft();
  if (!raw) return draft;
  let data: unknown;
  try {
    data = JSON.parse(raw);
  } catch {
    return draft;
  }
  if (typeof data !== 'object' || data === null) return draft;
  const obj = data as Record<string, unknown>;

  if (typeof obj.fields === 'object' && obj.fields !== null) {
    const src = obj.fields as Record<string, unknown>;
    for (const id of SEGMENT_IDS) {
      const v = src[id];
      if (typeof v === 'string') draft.fields[id] = clean(v, MAX_FIELD_CHARS);
    }
    if (typeof src.constraints === 'string' && draft.fields.stop_conditions === '') {
      draft.fields.stop_conditions = clean(src.constraints, MAX_FIELD_CHARS);
    }
  }
  if (typeof obj.idea === 'string') draft.idea = clean(obj.idea, MAX_IDEA_CHARS);
  if (isPersonaId(obj.persona)) draft.persona = obj.persona;
  if (isLangId(obj.lang)) draft.lang = obj.lang;
  if (isOutputFormat(obj.format)) draft.format = obj.format;
  return draft;
}

export const isBlank = (fields: Fields): boolean => SEGMENT_IDS.every((id) => fields[id].trim() === '');
