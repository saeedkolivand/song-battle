import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const root = dirname(fileURLToPath(import.meta.url));
const pkg = (p: string) => resolve(root, '../../packages', p, 'src');

// Mirrors vite.config.ts aliases so tests resolve the workspace packages from source.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@sb/types': pkg('types'),
      '@sb/shared': pkg('shared'),
      '@sb/ui': pkg('ui'),
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['src/test/setup.ts'],
    css: false,
    include: ['src/**/*.test.{ts,tsx}'],
  },
});
