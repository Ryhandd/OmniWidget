// "Tahan tetap terbuka": selama ada tahanan, island tidak auto-ciut walau kursor keluar jendela.
// Dipakai untuk dialog native OS (kursor pasti meninggalkan widget) dan drag file dari OS.
let holds = 0;
const subs = new Set<() => void>();

export const islandHold = {
  acquire(): () => void {
    holds++;
    let done = false;
    return () => {
      if (done) return;
      done = true;
      holds = Math.max(0, holds - 1);
      if (holds === 0) subs.forEach((f) => f());
    };
  },
  active: () => holds > 0,
  /** Dipanggil saat tahanan terakhir dilepas. */
  onRelease(fn: () => void) {
    subs.add(fn);
    return () => subs.delete(fn);
  },
};

export async function withHold<T>(run: () => Promise<T>): Promise<T> {
  const release = islandHold.acquire();
  try {
    return await run();
  } finally {
    release();
  }
}
