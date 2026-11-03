import { useState } from 'preact/hooks';
import { fmtTime, progressNow } from '../state';
import type { Player } from '../types';
import { useCover } from '../useSpotify';

type Props = { player: Player; receivedAt: number; now: number; onSeek: (ms: number) => void };

export function NowPlaying({ player, receivedAt, now, onSeek }: Props) {
  const item = player.item;
  const cover = useCover(item?.imageUrl);
  const [drag, setDrag] = useState<number | null>(null);

  if (!item) {
    return (
      <div class="spo__now">
        <div class="spo__cover" aria-hidden="true" />
        <div class="spo__meta">
          <strong class="spo__title">Iklan atau sesi privat</strong>
          <span class="muted spo__sub">Info lagu tidak tersedia dari Spotify.</span>
        </div>
      </div>
    );
  }

  const dur = item.durationMs;
  const pos = drag ?? progressNow(player, receivedAt, now);
  const sub = [item.artists, item.album].filter(Boolean).join(' · ');

  return (
    <div class="spo__now">
      {cover ? <img class="spo__cover" src={cover} alt="" /> : <div class="spo__cover" aria-hidden="true" />}
      <div class="spo__meta">
        <strong class="spo__title" title={item.name}>{item.name}</strong>
        <span class="muted spo__sub" title={sub}>{sub}</span>
        <div class="spo__seek">
          <span class="spo__time">{fmtTime(pos)}</span>
          <input
            type="range"
            class="spo__range"
            min={0}
            max={Math.max(dur, 1)}
            step={1000}
            value={Math.min(pos, Math.max(dur, 1))}
            disabled={dur <= 0}
            aria-label="Posisi lagu"
            onInput={(e) => setDrag(Number((e.target as HTMLInputElement).value))}
            onChange={(e) => { setDrag(null); onSeek(Number((e.target as HTMLInputElement).value)); }}
          />
          <span class="spo__time">{fmtTime(dur)}</span>
        </div>
      </div>
    </div>
  );
}
