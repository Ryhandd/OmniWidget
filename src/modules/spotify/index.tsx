import { ConnectPanel } from './components/ConnectPanel';
import { Controls } from './components/Controls';
import { DevicePicker } from './components/DevicePicker';
import { NowPlaying } from './components/NowPlaying';
import { useNow, useSpotify } from './useSpotify';
import './styles.css';

export default function SpotifyModule() {
  const { state: s, actions } = useSpotify();
  const now = useNow(!!s.player?.isPlaying);

  if (s.phase === 'loading') {
    return (
      <div class="spo">
        <div class={s.error ? 'spo__notice spo__notice--err' : 'muted'} role="status">{s.error ? s.error.message : 'Memuat…'}</div>
      </div>
    );
  }

  if (s.phase === 'setup' || s.phase === 'connect') {
    return (
      <ConnectPanel
        phase={s.phase}
        status={s.status}
        logging={s.logging}
        error={s.error}
        onSaveClientId={actions.saveClientId}
        onLogin={actions.login}
        onCancel={actions.cancelLogin}
      />
    );
  }

  const p = s.player;
  return (
    <div class="spo">
      {p ? (
        <>
          <NowPlaying player={p} receivedAt={s.receivedAt} now={now} onSeek={(ms) => actions.control({ action: 'seek', position_ms: ms })} />
          <Controls player={p} onControl={actions.control} />
        </>
      ) : (
        <section class="spo__card">
          <h4>Tidak ada pemutaran aktif</h4>
          <p class="muted spo__fine">Buka Spotify di ponsel atau komputer Anda, lalu pilih perangkat di bawah untuk mulai mengontrolnya dari sini.</p>
        </section>
      )}

      <DevicePicker
        current={p?.device ?? null}
        devices={s.devices}
        onOpen={actions.loadDevices}
        onPick={(d) => d.id && actions.control({ action: 'transfer', device_id: d.id, play: p?.isPlaying ?? true })}
      />

      {s.error && (
        <div class="spo__notice spo__notice--err" role="alert">
          <span>{s.error.message}</span>
          <button type="button" class="spo__link" onClick={actions.dismissError}>tutup</button>
        </div>
      )}

      <div class="spo__foot">
        <button type="button" class="spo__link" onClick={actions.logout}>Putuskan akun Spotify</button>
      </div>
    </div>
  );
}
