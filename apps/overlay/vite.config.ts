import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const root = dirname(fileURLToPath(import.meta.url));
const pkg = (p: string) => resolve(root, '../../packages', p, 'src');

// Relative base so the embedded bundle resolves assets no matter where axum mounts it.
export default defineConfig({
  base: './',
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@sb/types': pkg('types'),
      '@sb/shared': pkg('shared'),
      '@sb/ui': pkg('ui'),
    },
  },
  build: { outDir: 'dist', emptyOutDir: true },
});
