import type { Config } from 'tailwindcss';
import plugin from 'tailwindcss/plugin';

// Semua warna/radius/bayangan berasal dari design token (CSS variables) milik tema aktif.
// Komponen tidak boleh memakai nilai literal: ganti tema = ganti variabel, bukan ganti class.
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  corePlugins: { preflight: false }, // reset sudah ada di base.css; jangan timpa gaya modul lain
  theme: {
    extend: {
      colors: {
        bg: 'var(--bg)',
        surface: 'var(--surface)',
        'surface-2': 'var(--surface-2)',
        ink: 'var(--text)',
        dim: 'var(--text-dim)',
        accent: 'var(--accent)',
        'accent-ink': 'var(--accent-text)',
        'accent-2': 'var(--accent-2)',
        danger: 'var(--danger)',
        line: 'var(--border-c)',
      },
      borderRadius: { token: 'var(--radius)', 'token-sm': 'calc(var(--radius) / 2)' },
      borderWidth: { token: 'var(--border-w)' },
      boxShadow: { token: 'var(--shadow)' },
      fontFamily: { ui: 'var(--font-ui)' },
    },
  },
  plugins: [
    plugin(({ addUtilities }) => {
      // --motion berisi "durasi easing" (mis. "120ms steps(4)"), jadi dipakai langsung di shorthand transition.
      addUtilities({
        '.motion-colors': { transition: 'background var(--motion), color var(--motion), border-color var(--motion)' },
      });
    }),
  ],
} satisfies Config;
