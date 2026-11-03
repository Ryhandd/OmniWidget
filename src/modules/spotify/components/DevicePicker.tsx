import type { Device } from '../types';

type Props = {
  current: Device | null;
  devices: Device[];
  onOpen: () => void;
  onPick: (d: Device) => void;
};

/** Pemilih perangkat. Daftar dimuat saat dibuka (bukan saat polling) agar tidak membebani API. */
export function DevicePicker({ current, devices, onOpen, onPick }: Props) {
  const usable = devices.filter((d) => d.id);
  const options = usable.length > 0 ? usable : current?.id ? [current] : [];
  return (
    <label class="spo__dev">
      <span class="muted">Perangkat</span>
      <select
        class="field"
        value={current?.id ?? ''}
        onFocus={onOpen}
        onChange={(e) => {
          const id = (e.target as HTMLSelectElement).value;
          const d = options.find((x) => x.id === id);
          if (d) onPick(d);
        }}
      >
        {!current?.id && <option value="">Pilih perangkat…</option>}
        {options.map((d) => <option key={d.id} value={d.id ?? ''}>{d.name} · {d.kind}</option>)}
      </select>
    </label>
  );
}
