import { SEGMENTS, type Fields, type Segment } from './segments';

export type OutputFormat = 'plain' | 'xml' | 'markdown';

// Urutan tombol di UI: Teks, XML, MD.
export const FORMATS: readonly { id: OutputFormat; label: string; title: string }[] = [
  { id: 'plain', label: 'Teks', title: 'Teks biasa dengan label (Role:, Task:, ...)' },
  { id: 'xml', label: 'XML', title: 'Tag XML (<role>…</role>), nyaman dibaca model' },
  { id: 'markdown', label: 'MD', title: 'Markdown (### ROLE, ### TASK, ...)' },
];

export const DEFAULT_FORMAT: OutputFormat = 'xml';

export const isOutputFormat = (v: unknown): v is OutputFormat => FORMATS.some((f) => f.id === v);

export const normalize = (s: string) => s.replace(/\r\n?/g, '\n').trim();

// Isi pengguna tidak boleh menutup tag segmennya sendiri (mis. mengetik "</task>" di dalam Task).
const neutralizeClosing = (body: string, tag: string) =>
  body.replace(new RegExp(`</${tag}>`, 'gi'), `<\\/${tag}>`);

/** Format satu blok. `body` sudah dinormalkan dan tidak kosong. */
function render(seg: Segment, body: string, format: OutputFormat): string {
  switch (format) {
    case 'xml':
      return `<${seg.tag}>\n${neutralizeClosing(body, seg.tag)}\n</${seg.tag}>`;
    case 'plain':
      return `${seg.label}:\n${body}`;
    case 'markdown':
    default:
      return `### ${seg.label.toUpperCase()}\n${body}`;
  }
}

/** Satu blok dalam format terpilih; string kosong bila blok kosong. Dipakai tombol "Salin Bagian". */
export function composeSegment(seg: Segment, fields: Fields, format: OutputFormat): string {
  const body = normalize(fields[seg.id] ?? '');
  return body ? render(seg, body, format) : '';
}

/** Rakit prompt dari blok yang terisi (kosong dilewati). Murni: tanpa efek samping. */
export function compose(fields: Fields, format: OutputFormat): string {
  return SEGMENTS.map((seg) => composeSegment(seg, fields, format))
    .filter(Boolean)
    .join('\n\n');
}

export const filledCount = (fields: Fields): number =>
  SEGMENTS.filter((s) => normalize(fields[s.id] ?? '') !== '').length;

/** Perkiraan kasar (≈4 karakter per token); cukup untuk gambaran ukuran, bukan penagihan. */
export const estimateTokens = (text: string): number => Math.ceil(text.length / 4);
