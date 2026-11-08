// Ikon garis kecil untuk pemutar musik (tanpa dependensi ikon).
import type { ComponentChildren } from 'preact';

type P = { class?: string };

const svg = (children: ComponentChildren, extra = '') => (
  <svg class={`mico${extra}`} viewBox="0 0 24 24" aria-hidden="true">
    {children}
  </svg>
);

export const PlayIcon = (_: P) => svg(<path d="M8 5.5v13l10-6.5z" fill="currentColor" stroke="none" />);
export const PauseIcon = (_: P) => svg(<path d="M8.5 5.5v13M15.5 5.5v13" />);
export const PrevIcon = (_: P) =>
  svg(<><path d="M18.5 6v12L10 12z" fill="currentColor" stroke="none" /><path d="M6 6v12" /></>);
export const NextIcon = (_: P) =>
  svg(<><path d="M5.5 6v12L14 12z" fill="currentColor" stroke="none" /><path d="M18 6v12" /></>);
export const ShuffleIcon = (_: P) =>
  svg(<><path d="M3 7h3.2c1.4 0 2.6.7 3.4 1.9l4.8 6.2c.8 1.2 2 1.9 3.4 1.9H21" /><path d="M18 4l3 3-3 3" /><path d="M3 17h3.2c1.4 0 2.6-.7 3.4-1.9" /><path d="M13.4 8.9C14.2 7.7 15.4 7 16.8 7H21" /><path d="M18 20l3-3-3-3" /></>);
export const RepeatIcon = (_: P) =>
  svg(<><path d="M17 3l3 3-3 3" /><path d="M20 6H8a4 4 0 0 0-4 4v1" /><path d="M7 21l-3-3 3-3" /><path d="M4 18h12a4 4 0 0 0 4-4v-1" /></>);
export const VolumeIcon = (_: P) =>
  svg(<><path d="M4 9.5h3L12 5v14l-5-4.5H4z" /><path d="M15.5 9a4 4 0 0 1 0 6" /><path d="M18 6.5a7.5 7.5 0 0 1 0 11" /></>);
export const MuteIcon = (_: P) =>
  svg(<><path d="M4 9.5h3L12 5v14l-5-4.5H4z" /><path d="M16 9.5l5 5M21 9.5l-5 5" /></>);
export const ListIcon = (_: P) =>
  svg(<><path d="M8 6h12M8 12h12M8 18h12" /><path d="M4 6h.01M4 12h.01M4 18h.01" /></>);
export const PlusIcon = (_: P) => svg(<><path d="M12 5v14M5 12h14" /></>);
export const TrashIcon = (_: P) =>
  svg(<><path d="M4 7h16" /><path d="M9 7V4h6v3" /><path d="M6 7l1 13h10l1-13" /><path d="M10 11v5M14 11v5" /></>);
export const ExternalIcon = (_: P) =>
  svg(<><path d="M14 4h6v6" /><path d="M20 4l-8 8" /><path d="M19 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h5" /></>);
export const NoteIcon = (_: P) =>
  svg(<><path d="M9 18V6l10-2v12" /><circle cx="6.5" cy="18" r="2.5" /><circle cx="16.5" cy="16" r="2.5" /></>);
