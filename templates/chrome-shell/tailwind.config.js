import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';

const require = createRequire(import.meta.url);
const uiDist = join(dirname(require.resolve('@almadar/ui')), '..', '**', '*.js');

/** @type {import('tailwindcss').Config} */
export default {
  presets: [require('@almadar/ui/tailwind-preset')],
  content: ['./panel.html', './src/**/*.{ts,tsx}', uiDist],
  plugins: [require('@tailwindcss/container-queries')],
};
