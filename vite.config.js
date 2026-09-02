import { defineConfig } from 'vite';

export default defineConfig({
  base: './',
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: 'esnext',
    minify: true,
    sourcemap: false,
  },
});
