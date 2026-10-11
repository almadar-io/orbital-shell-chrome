import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// onnxruntime-web's embedded-factory build: an MV3 service worker forbids the dynamic import() of the default build.
const onnxDist = dirname(createRequire(import.meta.url).resolve('onnxruntime-web'));

// The side panel page and the service worker (an ES module worker).
export default defineConfig({
  plugins: [react()],
  resolve: { alias: { 'onnxruntime-web': resolve(onnxDist, 'ort.bundle.min.mjs') } },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    rollupOptions: {
      input: { panel: resolve(__dirname, 'panel.html'), worker: resolve(__dirname, 'src/worker.ts') },
      output: { entryFileNames: (chunk) => (chunk.name === 'worker' ? 'worker.js' : 'assets/[name]-[hash].js') },
    },
  },
});
