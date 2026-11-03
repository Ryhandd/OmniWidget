// Hanya CSS tema aktif yang dimuat: ganti tema = ganti href <link> + atribut data-theme.
type ThemeDef = { name: string; base: 'light' | 'dark'; load?: () => Promise<{ default: string }> };

export const THEMES: Record<string, ThemeDef> = {
  default: { name: 'Midnight', base: 'dark' },
  'neo-brutalism': { name: 'Neo-Brutalism', base: 'light', load: () => import('./neo-brutalism.css?url') },
  retro: { name: 'Retro', base: 'dark', load: () => import('./retro.css?url') },
  'glass-dark': { name: 'Glass Dark', base: 'dark', load: () => import('./glass-dark.css?url') },
};

const KEY = 'omniwidget.theme';

export async function applyTheme(id: string) {
  const def = THEMES[id] ?? THEMES.default;
  const key = THEMES[id] ? id : 'default';
  const link = document.getElementById('theme-css') as HTMLLinkElement;
  if (def.load) {
    const url = (await def.load()).default;
    await new Promise<void>((res) => {
      link.onload = () => res();
      link.onerror = () => res();
      link.href = url;
    });
  } else {
    link.removeAttribute('href');
  }
  document.documentElement.dataset.theme = key;
  document.documentElement.style.colorScheme = def.base;
  // Sementara memakai localStorage; pindahkan ke storage Rust (kv) saat modul SQLite siap.
  try { localStorage.setItem(KEY, key); } catch { /* storage bisa kosong/dimatikan */ }
}

export async function loadSavedTheme() {
  let id = 'default';
  try { id = localStorage.getItem(KEY) ?? 'default'; } catch { /* abaikan */ }
  await applyTheme(id);
}
