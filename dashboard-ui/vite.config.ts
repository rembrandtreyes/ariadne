import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// The app is served by the ariadne binary at /next/ (see src/dashboard/mod.rs).
// build.rs sets VITE_OUT_DIR to Cargo's OUT_DIR so the bundle never lands in
// the source tree; plain `bun run build` (no env) writes to dist/ for dev.
export default defineConfig({
  base: '/next/',
  plugins: [svelte()],
  build: {
    outDir: process.env.VITE_OUT_DIR || 'dist',
    emptyOutDir: true,
    target: 'es2022',
  },
  server: {
    // Dev-mode proxy so `bun run dev` talks to a running `ariadne dash`.
    proxy: {
      '/api': 'http://127.0.0.1:1337',
    },
  },
});
