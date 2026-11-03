// Cermin tipe serde di Rust (`plugins/clipboard`). Respons camelCase (`#[serde(rename_all = "camelCase")]`).
// >>> Kontrak ini DIASUMSIKAN dari TDD 4.5; cocokkan dengan backend sungguhan di sini dan di api.ts saja. <<<

export type ItemKind = 'text' | 'image';

/** Metadata ringan satu entri riwayat. Isi penuh & thumbnail TIDAK ikut: diminta terpisah saat dibutuhkan. */
export type ClipItem = {
  /** Hash konten sebagai STRING: u64 Rust melebihi 2^53 dan rusak bila dikirim sebagai angka JSON. */
  id: string;
  kind: ItemKind;
  /** Teks: potongan awal (maks ~200 karakter). Gambar: deskripsi, mis. "Gambar 1920×1080". */
  preview: string;
  /** Teks dipotong di batas 64 KB (TDD 4.5). */
  truncated: boolean;
  /** Epoch milidetik UTC. */
  ts: number;
  pinned: boolean;
  /** Hanya gambar. */
  width: number | null;
  height: number | null;
  bytes: number | null;
};

export type ClipSettings = {
  /** Watcher berhenti merekam (item yang sudah ada tetap). */
  paused: boolean;
  /** Persisten terenkripsi (ChaCha20-Poly1305, kunci di keychain). Default mati. */
  persist: boolean;
  /** Bersihkan riwayat (kecuali pin) saat aplikasi keluar. */
  clearOnExit: boolean;
};

export type ClipSnapshot = {
  items: ClipItem[];
  settings: ClipSettings;
  /** Kapasitas ring untuk item tidak disematkan (20). */
  capacity: number;
};

/** Perubahan pengaturan parsial; field yang tidak ada dibiarkan. */
export type ClipSettingsPatch = Partial<ClipSettings>;
