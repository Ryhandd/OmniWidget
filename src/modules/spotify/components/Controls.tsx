import { useState } from 'preact/hooks';
import { nextRepeat } from '../state';
import type { Control, Player } from '../types';
import { IconNext, IconPause, IconPlay, IconPrev, IconRepeat, IconShuffle, IconVolume } from './icons';

type Props = { player: Player; onControl: (c: Control) => void };

const REPEAT_LABEL = { off: 'Ulangi: mati', context: 'Ulangi: daftar putar', track: 'Ulangi: satu lagu' } as const;

export function Controls({ player, onControl }: Props) {
  const [vol, setVol] = useState<number | null>(null);
  const dev = player.device;
  const canVolume = !!dev && dev.supportsVolume;
  const volume = vol ?? dev?.volumePercent ?? 0;

  return (
    <div class="spo__controls">
      <div class="spo__transport">
        <button type="button" class="spo__btn" data-on={player.shuffle} aria-pressed={player.shuffle} aria-label="Acak" title="Acak"
          onClick={() => onControl({ action: 'shuffle', on: !player.shuffle })}>
          <IconShuffle />
        </button>
        <button type="button" class="spo__btn" aria-label="Sebelumnya" title="Sebelumnya" onClick={() => onControl({ action: 'previous' })}>
          <IconPrev />
        </button>
        <button type="button" class="spo__btn spo__btn--main" aria-label={player.isPlaying ? 'Jeda' : 'Putar'} title={player.isPlaying ? 'Jeda' : 'Putar'}
          onClick={() => onControl({ action: player.isPlaying ? 'pause' : 'play' })}>
          {player.isPlaying ? <IconPause /> : <IconPlay />}
        </button>
        <button type="button" class="spo__btn" aria-label="Berikutnya" title="Berikutnya" onClick={() => onControl({ action: 'next' })}>
          <IconNext />
        </button>
        <button type="button" class="spo__btn" data-on={player.repeat !== 'off'} aria-label={REPEAT_LABEL[player.repeat]} title={REPEAT_LABEL[player.repeat]}
          onClick={() => onControl({ action: 'repeat', mode: nextRepeat(player.repeat) })}>
          <IconRepeat />
          {player.repeat === 'track' && <span class="spo__badge" aria-hidden="true">1</span>}
        </button>
      </div>

      <label class="spo__vol" title={canVolume ? 'Volume' : 'Perangkat ini tidak mendukung kontrol volume'}>
        <IconVolume />
        <input
          type="range"
          class="spo__range"
          min={0}
          max={100}
          step={1}
          value={volume}
          disabled={!canVolume}
          aria-label="Volume"
          onInput={(e) => setVol(Number((e.target as HTMLInputElement).value))}
          onChange={(e) => { const v = Number((e.target as HTMLInputElement).value); setVol(null); onControl({ action: 'volume', percent: v }); }}
        />
        <span class="spo__time">{volume}</span>
      </label>
    </div>
  );
}
