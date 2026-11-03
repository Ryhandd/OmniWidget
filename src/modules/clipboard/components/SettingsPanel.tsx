import type { ClipSettings, ClipSettingsPatch } from '../types';

type Props = { settings: ClipSettings; onChange: (p: ClipSettingsPatch) => void };

export function SettingsPanel({ settings, onChange }: Props) {
  return (
    <section class="clip__panel" aria-label="Pengaturan clipboard">
      <label class="clip__opt">
        <input type="checkbox" checked={settings.persist} onChange={(e) => onChange({ persist: e.currentTarget.checked })} />
        <span>
          <b>Simpan riwayat terenkripsi</b>
          <small class="muted">Bertahan setelah restart (ChaCha20-Poly1305, kunci di keychain OS). Bawaan: mati, riwayat hanya di memori.</small>
        </span>
      </label>
      <label class="clip__opt">
        <input type="checkbox" checked={settings.clearOnExit} onChange={(e) => onChange({ clearOnExit: e.currentTarget.checked })} />
        <span>
          <b>Bersihkan saat keluar</b>
          <small class="muted">Hapus riwayat (kecuali yang disematkan) ketika OmniWidget ditutup.</small>
        </span>
      </label>
      <p class="clip__fine muted">Konten yang ditandai sensitif oleh aplikasi sumber (mis. pengelola kata sandi) otomatis tidak direkam.</p>
    </section>
  );
}
