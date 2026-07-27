import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      // `ws` is required: the change feed is a websocket under the same prefix,
      // and without it the upgrade is answered with the index page instead.
      '/api': {
        target: process.env.OPTIMIST_API_URL ?? 'http://127.0.0.1:3000',
        ws: true,
        changeOrigin: true,
      },
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    include: ['src/**/*.test.ts'],
    setupFiles: ['./src/test/setup.ts'],
  },
})
