import { useState } from 'preact/hooks';
import { copyText } from '../../../core/clipboard';
import { isClientId } from '../state';
import type { SpotifyErr, Status } from '../types';

type Props = {
  phase: 'setup' | 'connect';
  status: Status | null;
  logging: boolean;
  error: SpotifyErr | null;
  onSaveClientId: (id: string) => void;
  onLogin: () => void;
  onCancel: () => void;
};

export function ConnectPanel({ phase, status, logging, error, onSaveClientId, onLogin, onCancel }: Props) {
  const [id, setId] = useState('');
  const [editing, setEditing] = useState(false);
  const [copied, setCopied] = useState(false);
  const redirect = status?.redirectUri ?? '';
  const showSetup = phase === 'setup' || editing;

  const copy = async () => {
    setCopied(await copyText(redirect));
    window.setTimeout(() => setCopied(false), 1500);
  };
  const save = () => { onSaveClientId(id.trim()); setEditing(false); setId(''); };

  return (
    <div class="spo">
      {showSetup ? (
        <section class="spo__card">
          <h4>Siapkan aplikasi Spotify</h4>
          <ol class="spo__steps">
            <li>Buka <code>developer.spotify.com/dashboard</code> lalu <em>Create app</em> (pilih <em>Web API</em>).</li>
            <li>
              Pada <em>Redirect URIs</em>, daftarkan persis:
              <div class="spo__copy">
                <code class="spo__uri">{redirect}</code>
                <button type="button" class="btn btn--ghost" onClick={copy}>{copied ? 'Tersalin' : 'Salin'}</button>
              </div>
            </li>
            <li>Salin <em>Client ID</em> ke kolom di bawah. Tidak perlu Client Secret.</li>
          </ol>
          <div class="spo__row">
            <input class="field" type="text" value={id} spellcheck={false} autocomplete="off" placeholder="Client ID (32 karakter)"
              onInput={(e) => setId((e.target as HTMLInputElement).value)}
              onKeyDown={(e) => { if (e.key === 'Enter' && isClientId(id)) save(); }} />
            <button type="button" class="btn" disabled={!isClientId(id)} onClick={save}>Simpan</button>
            {editing && <button type="button" class="btn btn--ghost" onClick={() => { setEditing(false); setId(''); }}>Batal</button>}
          </div>
          <p class="muted spo__fine">
            Pemutaran hanya bisa dikontrol dengan akun Premium. Mode Development Spotify mensyaratkan pemilik app berlangganan
            Premium dan membatasi maksimal 5 pengguna terdaftar.
          </p>
        </section>
      ) : (
        <section class="spo__card">
          <h4>Hubungkan Spotify</h4>
          <p class="muted spo__fine">
            Client ID: <code>{status?.clientId?.slice(0, 6)}…</code>{' '}
            <button type="button" class="spo__link" disabled={logging} onClick={() => setEditing(true)}>ganti</button>
          </p>
          {logging ? (
            <>
              <p class="spo__wait" role="status">Menunggu persetujuan di browser… Selesaikan izin di tab yang terbuka.</p>
              <button type="button" class="btn btn--ghost" onClick={onCancel}>Batal</button>
            </>
          ) : (
            <button type="button" class="btn" onClick={onLogin}>Hubungkan Spotify</button>
          )}
        </section>
      )}
      {error && <div class="spo__notice spo__notice--err" role="alert">{error.message}</div>}
    </div>
  );
}
