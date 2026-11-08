import { useState } from 'preact/hooks';
import { music, useMusicTick, useNowPlaying, useVolumeOnce } from '../core/nowplaying';
import { appLabel, interpolatePosition, progressPct } from '../modules/music/logic';
import {
  MuteIcon,
  NextIcon,
  PauseIcon,
  PlayIcon,
  PrevIcon,
  VolumeIcon,
} from '../modules/music/icons';

/**
 * Pemutar ringkas yang selalu terlihat di panel saat ada sesi media.
 * Satu klik pada judul/sampul membuka modul Musik penuh.
 */
export function MiniPlayer({ onOpen }: { onOpen: () => void }) {
  const s = useNowPlaying();
  const [volOpen, setVolOpen] = useState(false);
  const snap = s.snap;
  useVolumeOnce();
  useMusicTick(!!snap?.playing);

  if (!snap?.available || !snap.title) return null;

  const pos = interpolatePosition(snap.positionMs, snap.playing, s.fetchedAt, Date.now(), snap.durationMs);
  const pct = progressPct(pos, snap.durationMs);
  const vol = s.volume;

  return (
    <div class="mini" data-playing={snap.playing ? '' : undefined}>
      <button type="button" class="mini__art" onClick={onOpen} title="Buka pemutar Musik" aria-label="Buka pemutar Musik">
        {s.art ? <img src={s.art} alt="" draggable={false} /> : <span class="mini__ph" aria-hidden="true">♪</span>}
      </button>

      <button type="button" class="mini__meta" onClick={onOpen} title={`${snap.title} — ${snap.artist}`}>
        <span class="mini__title">{snap.title}</span>
        <span class="mini__sub">
          {snap.artist || appLabel(snap.app)}
          <i class="mini__dot" aria-hidden="true">·</i>
          <span class="mini__src">{appLabel(snap.app)}</span>
        </span>
      </button>

      <div class="mini__actions">
        <button
          type="button"
          class="mini__btn"
          title="Lagu sebelumnya"
          aria-label="Lagu sebelumnya"
          disabled={!snap.canPrev}
          onClick={() => void music.control('prev')}
        >
          <PrevIcon />
        </button>
        <button
          type="button"
          class="mini__btn mini__btn--main"
          title={snap.playing ? 'Jeda' : 'Putar'}
          aria-label={snap.playing ? 'Jeda' : 'Putar'}
          disabled={snap.playing ? !snap.canPause : !snap.canPlay}
          onClick={() => void music.control('toggle')}
        >
          {snap.playing ? <PauseIcon /> : <PlayIcon />}
        </button>
        <button
          type="button"
          class="mini__btn"
          title="Lagu berikutnya"
          aria-label="Lagu berikutnya"
          disabled={!snap.canNext}
          onClick={() => void music.control('next')}
        >
          <NextIcon />
        </button>
        <button
          type="button"
          class="mini__btn"
          data-on={volOpen ? 'true' : undefined}
          title="Volume"
          aria-label="Atur volume"
          aria-expanded={volOpen}
          onClick={() => setVolOpen((v) => !v)}
        >
          {vol === 0 ? <MuteIcon /> : <VolumeIcon />}
        </button>
      </div>

      {volOpen && (
        <div class="mini__vol" role="group" aria-label="Volume">
          <span class="mini__volt">{vol === null ? '—' : Math.round(vol * 100)}</span>
          <input
            type="range"
            min={0}
            max={100}
            step={1}
            value={vol === null ? 100 : Math.round(vol * 100)}
            disabled={vol === null}
            aria-label="Volume"
            onInput={(e) => music.setVolume(Number((e.currentTarget as HTMLInputElement).value) / 100)}
          />
        </div>
      )}

      <div class="mini__bar" aria-hidden="true">
        <i style={`width:${pct}%`} />
      </div>
    </div>
  );
}
