import { defineConfig } from 'vite';
import preact from '@preact/preset-vite';

export default defineConfig({
  plugins: [preact()],
  resolve: {
    alias: {
      react: 'preact/compat',
      'react-dom': 'preact/compat',
    },
  },
  build: {
    // Dua halaman: widget utama + overlay capture (eyedropper/ruler/QR) yang dibuat on-demand.
    rollupOptions: { input: { main: 'index.html', overlay: 'overlay.html' } },
  },
  server: {
    port: 1420,
    strictPort: true,
  },
});