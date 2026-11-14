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
  // Buang console/debugger dan sourcemap: bundel lebih kecil, parse JS lebih ringan di WebView.
  esbuild: { drop: ['console', 'debugger'], legalComments: 'none' },
  build: {
    target: 'es2021',
    sourcemap: false,
    cssMinify: true,
    reportCompressedSize: false,
    modulePreload: { polyfill: false },
    // Dua halaman: widget utama + overlay capture (eyedropper/ruler/QR) yang dibuat on-demand.
    rollupOptions: { input: { main: 'index.html', overlay: 'overlay.html' } },
  },
  server: {
    port: 1420,
    strictPort: true,
  },
});