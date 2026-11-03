import { bridge } from './bridge';

// Pintu tunggal UI ke backend LLM di Rust. API key tidak pernah kembali ke WebView: hanya `hasKey`.
export type LlmProvider = 'openai' | 'anthropic';

export type LlmStatus = {
  provider: LlmProvider;
  baseUrl: string;
  model: string;
  hasKey: boolean;
  /** Model terisi dan (key tersedia atau server lokal): siap dipanggil. */
  configured: boolean;
};

export type LlmConfigInput = {
  provider: LlmProvider;
  baseUrl: string;
  model: string;
  /** undefined = biarkan key lama; '' = hapus key; selain itu = simpan key baru. */
  apiKey?: string;
};

export const llm = {
  status: () => bridge.invoke<LlmStatus>('llm_get_config'),
  save: (c: LlmConfigInput) =>
    bridge.invoke<LlmStatus>('llm_set_config', {
      provider: c.provider,
      baseUrl: c.baseUrl,
      model: c.model,
      apiKey: c.apiKey ?? null,
    }),
  /** `system` boleh kosong. Melempar string pesan error berbahasa Indonesia dari Rust. */
  complete: (system: string, user: string, maxTokens?: number) =>
    bridge.invoke<string>('llm_complete', { system, user, maxTokens: maxTokens ?? null }),
};

/** Error dari Rust tiba sebagai string; di browser biasa (tanpa Tauri) invoke melempar TypeError. */
export function errText(e: unknown): string {
  if (typeof e === 'string') return e;
  const msg = e instanceof Error ? e.message : '';
  if (/invoke|__TAURI/i.test(msg)) return 'Fitur AI hanya berjalan di aplikasi OmniWidget (bukan di browser biasa).';
  return msg || 'Terjadi kesalahan tak terduga';
}
