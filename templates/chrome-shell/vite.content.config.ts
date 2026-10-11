import { defineConfig } from 'vite';

// Content scripts run as classic scripts, so the page script is one self-contained IIFE.
export default defineConfig({
  build: {
    outDir: 'dist',
    emptyOutDir: false,
    lib: { entry: 'src/content.ts', formats: ['iife'], name: 'AlmadarPage', fileName: () => 'content.js' },
  },
});
