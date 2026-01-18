import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  base: '/admin/',
  build: {
    outDir: '../target/admin',
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    proxy: {
      '/apify': {
        target: 'http://localhost:4000',
        changeOrigin: true,
      },
    },
  },
});
