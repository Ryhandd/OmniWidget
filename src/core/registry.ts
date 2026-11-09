import type { ComponentType } from 'preact';

export type ModuleDef = {
  id: string;
  title: string;
  icon: string;
  load: () => Promise<{ default: ComponentType<any> }>;
};

// Tambah modul baru cukup dengan satu entri; modul hanya di-import saat tab pertama dibuka.
export const MODULES: ModuleDef[] = [
  { id: 'anatomy', title: 'Anatomy Prompt', icon: '✦', load: () => import('../modules/anatomy') },
  { id: 'todo', title: 'To-do & Pengingat', icon: '☑', load: () => import('../modules/todo') },
  { id: 'pomodoro', title: 'Pomodoro', icon: '◷', load: () => import('../modules/pomodoro') },
  { id: 'clipboard', title: 'Clipboard', icon: '⧉', load: () => import('../modules/clipboard') },
  { id: 'converter', title: 'Converter', icon: '⇄', load: () => import('../modules/converter') },
  { id: 'music', title: 'Musik', icon: '♪', load: () => import('../modules/music') },
  { id: 'brightness', title: 'Kecerahan', icon: '☀', load: () => import('../modules/brightness') },
  { id: 'capture', title: 'Eyedropper & Penggaris', icon: '◉', load: () => import('../modules/capture') },
  { id: 'qr', title: 'QR Code', icon: '▦', load: () => import('../modules/qr') },
  { id: 'sysmon', title: 'Monitor Sistem', icon: '∿', load: () => import('../modules/sysmon') },
  { id: 'settings', title: 'Pengaturan', icon: '⚙', load: () => import('../modules/settings') },
];
