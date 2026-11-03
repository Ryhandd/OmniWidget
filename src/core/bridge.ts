import { invoke, Channel, type InvokeArgs, type InvokeOptions } from '@tauri-apps/api/core';

export const bridge = {
  // `args` boleh objek biasa (JSON) atau Uint8Array/ArrayBuffer (badan biner mentah; metadata lewat `options.headers`).
  invoke: <T = unknown>(cmd: string, args?: InvokeArgs, options?: InvokeOptions) => invoke<T>(cmd, args, options),
  channel: <T>(onMsg: (m: T) => void) => {
    const c = new Channel<T>();
    c.onmessage = onMsg;
    return c;
  },
};
