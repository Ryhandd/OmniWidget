import { bridge } from './bridge';

/**
 * Salin teks ke clipboard sistem lewat Rust (andal di WebView desktop). Di luar Tauri (vite di browser)
 * invoke gagal, lalu jatuh ke Clipboard API browser. Mengembalikan false bila keduanya gagal.
 */
export async function copyText(text: string): Promise<boolean> {
  try {
    await bridge.invoke('clipboard_write', { text });
    return true;
  } catch { /* lanjut ke cadangan */ }
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
