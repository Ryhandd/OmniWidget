// Cache thumbnail sisi UI: LRU dengan anggaran byte, murni (tanpa DOM/Tauri) agar teruji.
// Anggaran mengikuti TDD 5.5 (thumbnail <= 400 KB). Thumbnail berupa data URI (CSP: img-src 'self' data:),
// jadi tidak ada object URL yang perlu di-revoke; melepas referensi string sudah cukup bagi GC.
//
// Siklus hidup:
//  - L1 (tab ditinggalkan)  : modul di-unmount -> `clear()` dipanggil hook -> seluruh cache lepas.
//  - item dihapus/tergusur  : `retain(idsAktif)` membuang entri yatim setiap snapshot baru.
//  - permintaan bersamaan   : `get()` men-dedup request yang sedang berjalan per id.

export const DEFAULT_BUDGET_BYTES = 400 * 1024;

type Loader = (id: string) => Promise<string | null>;

export class ThumbCache {
  private map = new Map<string, string>(); // urutan penyisipan = urutan LRU (tertua di depan)
  private pending = new Map<string, Promise<string | null>>();
  private bytes = 0;
  /** Naik setiap `clear()`: hasil request yang berangkat sebelum clear dibuang, bukan di-cache. */
  private epoch = 0;

  constructor(private load: Loader, private budget = DEFAULT_BUDGET_BYTES) {}

  get size() { return this.map.size; }
  get usedBytes() { return this.bytes; }

  /** Baca dari cache tanpa memicu pemuatan; menandai entri sebagai baru dipakai. */
  peek(id: string): string | null {
    const v = this.map.get(id);
    if (v === undefined) return null;
    this.map.delete(id);
    this.map.set(id, v);
    return v;
  }

  async get(id: string): Promise<string | null> {
    const hit = this.peek(id);
    if (hit !== null) return hit;
    const inflight = this.pending.get(id);
    if (inflight) return inflight;

    const epoch = this.epoch;
    const p = this.load(id)
      .then((uri) => {
        if (uri && epoch === this.epoch) this.put(id, uri);
        return uri;
      })
      .finally(() => {
        if (this.pending.get(id) === p) this.pending.delete(id);
      });
    this.pending.set(id, p);
    return p;
  }

  private put(id: string, uri: string) {
    const old = this.map.get(id);
    if (old !== undefined) { this.bytes -= old.length; this.map.delete(id); }
    // Satu entri lebih besar dari seluruh anggaran: tidak di-cache (tetap dikembalikan ke pemanggil).
    if (uri.length > this.budget) return;
    this.map.set(id, uri);
    this.bytes += uri.length;
    while (this.bytes > this.budget) {
      const oldest = this.map.keys().next().value as string | undefined;
      if (oldest === undefined) break;
      this.drop(oldest);
    }
  }

  drop(id: string) {
    const v = this.map.get(id);
    if (v === undefined) return;
    this.bytes -= v.length;
    this.map.delete(id);
  }

  /** Pertahankan hanya id yang masih ada di riwayat (item terhapus/tergusur ring). */
  retain(ids: Iterable<string>) {
    const keep = new Set(ids);
    for (const id of [...this.map.keys()]) if (!keep.has(id)) this.drop(id);
  }

  clear() {
    this.epoch++;
    this.map.clear();
    this.pending.clear();
    this.bytes = 0;
  }
}
