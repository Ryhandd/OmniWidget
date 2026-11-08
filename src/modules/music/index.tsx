import { useState } from 'preact/hooks';
import { music, useMusicTick, useNowPlaying, useVolumeOnce } from '../../core/nowplaying';
import {
  appLabel,
  fmtTime,
  interpolatePosition,
  progressPct,
  repeatLabel,
  songSearchUri,
  type RepeatMode,
} from './logic';
import {
  ExternalIcon,
  ListIcon,
  MuteIcon,
  NextIcon,
  NoteIcon,
  PauseIcon,
  PlayIcon,
  PlusIcon,
  PrevIcon,
  RepeatIcon,
  ShuffleIcon,
  TrashIcon,
  VolumeIcon,
} from './icons';
import './styles.css';

type Tab = 'history' | 'playlists';

export default function MusicModule() {
  const s = useNowPlaying();
  const [tab, setTab] = useState<Tab>('history');
  const [drag, setDrag] = useState<number | null>(null);
  const snap = s.snap;
  useVolumeOnce();
  useMusicTick(!!snap?.playing);

  if (!snap?.available || !snap.title) {
    return (
      <div class="mus mus--empty">
        <div class="mus__empty">
          <span class="mus__emptyico" aria-hidden="true"><NoteIcon /></span>
          <h3>Belum ada musik</h3>
          <p>
            Buka Spotify dan putar satu lagu. OmniWidget membaca sesi media Windows — tanpa login,
            tanpa API key, dan tetap bekerja di akun Free.
          </p>
          <div class="mus__emptyact">
            <button type="button" class="btn" onClick={() => void music.openUri('spotify:')}>
              Buka Spotify
            </button>
            <button type="button" class="btn btn--ghost" onClick={() => void music.refresh()}>
              Coba lagi
            </button>
          </div>
          {s.error && <p class="err mus__fine">{s.error}</p>}
        </div>
      </div>
    );
  }

  const pos = drag ?? interpolatePosition(snap.positionMs, snap.playing, s.fetchedAt, Date.now(), snap.durationMs);
  const pct = progressPct(pos, snap.durationMs);
  const vol = s.volume;
  const repeat: RepeatMode = snap.repeat;
  const canSeek = snap.canSeek && snap.durationMs > 0;
  const commitSeek = () => {
    if (drag === null) return;
    void music.seek(drag);
    setDrag(null);
  };

  return (
    <div class="mus">
      {s.error && (
        <button type="button" class="mus__notice" onClick={() => music.dismissError()}>
          {s.error}
          <span aria-hidden="true">×</span>
        </button>
      )}

      <section class="mus__hero">
        <div class={`mus__art${snap.playing ? ' is-playing' : ''}`}>
          {s.art ? <img src={s.art} alt={`Sampul ${snap.album || snap.title}`} draggable={false} /> : <span aria-hidden="true">♪</span>}
        </div>
        <div class="mus__meta">
          <div class="mus__kicker">
            <i class={`mus__led${snap.playing ? ' is-on' : ''}`} aria-hidden="true" />
            {appLabel(snap.app)} · {snap.playing ? 'Diputar' : 'Dijeda'}
          </div>
          <h2 class="mus__title" title={snap.title}>{snap.title}</h2>
          <div class="mus__artist" title={snap.artist}>{snap.artist || 'Artis tidak diketahui'}</div>
          {snap.album && <div class="mus__album" title={snap.album}>{snap.album}</div>}
        </div>
      </section>

      <div class="mus__seek">
        <span class="mus__time">{fmtTime(pos)}</span>
        <input
          class="mus__range"
          type="range"
          min={0}
          max={Math.max(1, snap.durationMs)}
          step={500}
          value={Math.min(pos, Math.max(1, snap.durationMs))}
          disabled={!canSeek}
          aria-label="Posisi lagu"
          style={`--pct:${pct}%`}
          onInput={(e) => setDrag(Number((e.currentTarget as HTMLInputElement).value))}
          onPointerUp={commitSeek}
          onPointerCancel={() => setDrag(null)}
          onKeyUp={commitSeek}
        />
        <span class="mus__time">{snap.durationMs ? fmtTime(snap.durationMs) : '--:--'}</span>
      </div>

      <div class="mus__transport">
        <button
          type="button"
          class="mus__btn"
          data-on={snap.shuffle ? 'true' : undefined}
          title={snap.shuffle ? 'Acak aktif' : 'Acak mati'}
          aria-label="Acak"
          aria-pressed={snap.shuffle}
          disabled={!snap.canShuffle}
          onClick={() => void music.toggleShuffle()}
        >
          <ShuffleIcon />
        </button>
        <button type="button" class="mus__btn" title="Sebelumnya" aria-label="Sebelumnya" disabled={!snap.canPrev} onClick={() => void music.control('prev')}>
          <PrevIcon />
        </button>
        <button
          type="button"
          class="mus__btn mus__btn--main"
          title={snap.playing ? 'Jeda' : 'Putar'}
          aria-label={snap.playing ? 'Jeda' : 'Putar'}
          disabled={snap.playing ? !snap.canPause : !snap.canPlay}
          onClick={() => void music.control('toggle')}
        >
          {snap.playing ? <PauseIcon /> : <PlayIcon />}
        </button>
        <button type="button" class="mus__btn" title="Berikutnya" aria-label="Berikutnya" disabled={!snap.canNext} onClick={() => void music.control('next')}>
          <NextIcon />
        </button>
        <button
          type="button"
          class="mus__btn"
          data-on={repeat !== 'off' ? 'true' : undefined}
          title={repeatLabel(repeat)}
          aria-label={repeatLabel(repeat)}
          disabled={!snap.canRepeat}
          onClick={() => void music.cycleRepeat()}
        >
          <RepeatIcon />
          {repeat === 'track' && <span class="mus__badge">1</span>}
        </button>
      </div>

      <div class="mus__volume">
        <button
          type="button"
          class="mus__volico"
          title={vol === 0 ? 'Bunyikan' : 'Bisukan'}
          aria-label={vol === 0 ? 'Bunyikan' : 'Bisukan'}
          onClick={() => music.setVolume(vol && vol > 0 ? 0 : 0.6)}
        >
          {vol === 0 ? <MuteIcon /> : <VolumeIcon />}
        </button>
        <input
          class="mus__range mus__range--vol"
          type="range"
          min={0}
          max={100}
          step={1}
          value={vol === null ? 100 : Math.round(vol * 100)}
          disabled={vol === null}
          aria-label="Volume"
          style={`--pct:${vol === null ? 100 : Math.round(vol * 100)}%`}
          onInput={(e) => music.setVolume(Number((e.currentTarget as HTMLInputElement).value) / 100)}
        />
        <span class="mus__volpct">{vol === null ? '—' : `${Math.round(vol * 100)}%`}</span>
      </div>

      <div class="mus__tabs" role="tablist" aria-label="Daftar musik">
        <button
          type="button"
          role="tab"
          aria-selected={tab === 'history'}
          class="mus__tab"
          onClick={() => setTab('history')}
        >
          <ListIcon /> Riwayat
          {s.history.length > 0 && <span class="mus__count">{s.history.length}</span>}
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === 'playlists'}
          class="mus__tab"
          onClick={() => setTab('playlists')}
        >
          <span aria-hidden="true" class="mus__pl">≡</span> Playlist
          {s.playlists.length > 0 && <span class="mus__count">{s.playlists.length}</span>}
        </button>
      </div>

      {tab === 'history' ? (
        <History
          items={s.history}
          playingKey={snap.artKey}
          onOpen={(q) => void music.openUri(q)}
          onClear={() => music.clearHistory()}
        />
      ) : (
        <Playlists />
      )}
    </div>
  );
}

function History({
  items,
  playingKey,
  onOpen,
  onClear,
}: {
  items: ReturnType<typeof useNowPlaying>['history'];
  playingKey: string;
  onOpen: (uri: string) => void;
  onClear: () => void;
}) {
  if (!items.length) {
    return (
      <div class="mus__list mus__list--empty">
        <p>Riwayat muncul otomatis saat lagu berganti. Tidak ada data yang dikirim keluar.</p>
      </div>
    );
  }
  return (
    <div class="mus__list">
      <div class="mus__listhead">
        <span>Terakhir diputar</span>
        <button type="button" class="mus__link" onClick={onClear}>Bersihkan</button>
      </div>
      <ol class="mus__rows">
        {items.map((h, i) => {
          const current = `${h.app}|${h.title}|${h.artist}|${h.album}` === playingKey;
          return (
            <li key={`${h.title}-${h.artist}-${h.at}`}>
              <button
                type="button"
                class="mus__row"
                data-current={current ? '' : undefined}
                title={`Cari "${h.title}" di Spotify`}
                onClick={() => onOpen(songSearchUri(h))}
              >
                <span class="mus__num" aria-hidden="true">{current ? '▶' : i + 1}</span>
                <span class="mus__rowmeta">
                  <span class="mus__rowtitle">{h.title}</span>
                  <span class="mus__rowsub">{h.artist || appLabel(h.app)}</span>
                </span>
                <span class="mus__rowtime">{new Date(h.at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                <ExternalIcon />
              </button>
            </li>
          );
        })}
      </ol>
    </div>
  );
}

function Playlists() {
  const s = useNowPlaying();
  const [name, setName] = useState('');
  const [link, setLink] = useState('');
  const [bad, setBad] = useState(false);

  const submit = (e: Event) => {
    e.preventDefault();
    const ok = music.addPlaylist(name, link);
    setBad(!ok);
    if (ok) {
      setName('');
      setLink('');
    }
  };

  return (
    <div class="mus__list">
      <form class="mus__add" onSubmit={submit}>
        <input
          class="field"
          type="text"
          placeholder="Nama playlist (mis. Favorit)"
          value={name}
          maxLength={40}
          onInput={(e) => setName((e.currentTarget as HTMLInputElement).value)}
        />
        <div class="mus__addrow">
          <input
            class={`field${bad ? ' mus__field--bad' : ''}`}
            type="text"
            placeholder="Tempel tautan Spotify…"
            value={link}
            onInput={(e) => { setLink((e.currentTarget as HTMLInputElement).value); setBad(false); }}
          />
          <button type="submit" class="mus__addbtn" title="Tambah playlist" aria-label="Tambah playlist">
            <PlusIcon />
          </button>
        </div>
        {bad && <p class="err mus__fine">Tautan tidak dikenali. Tempel tautan open.spotify.com atau spotify:playlist:…</p>}
      </form>

      <div class="mus__listhead">
        <span>Pintasan playlist</span>
        <button type="button" class="mus__link" onClick={() => void music.openUri('spotify:')}>Buka beranda</button>
      </div>

      {s.playlists.length === 0 ? (
        <p class="mus__hint">
          Tambahkan tautan playlist/album/lagu. Klik pintasan untuk membukanya di aplikasi Spotify —
          pemutaran langsung butuh Premium, jadi ini jalan terbaik untuk akun Free.
        </p>
      ) : (
        <ul class="mus__rows">
          {s.playlists.map((p) => (
            <li key={p.id} class="mus__plrow">
              <button
                type="button"
                class="mus__row"
                title={`Buka ${p.name} di Spotify`}
                onClick={() => void music.openUri(p.uri)}
              >
                <span class="mus__num" aria-hidden="true">♫</span>
                <span class="mus__rowmeta">
                  <span class="mus__rowtitle">{p.name}</span>
                  <span class="mus__rowsub">{p.uri.replace(/^spotify:/, '').split(':')[0]}</span>
                </span>
                <ExternalIcon />
              </button>
              <button
                type="button"
                class="mus__del"
                title="Hapus pintasan"
                aria-label={`Hapus ${p.name}`}
                onClick={() => music.removePlaylist(p.id)}
              >
                <TrashIcon />
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
