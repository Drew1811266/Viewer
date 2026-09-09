import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  // Tauri's devUrl is intentionally fixed to the canonical loopback port.
  // Refuse to auto-increment when a stale/orphaned Vite process owns it;
  // otherwise Tauri could connect to an older frontend while the new Vite
  // process quietly listens on another port.
  server: {
    host: 'localhost',
    port: 5173,
    strictPort: true,
  },
  test: {
    css: true,
    environment: 'jsdom',
    setupFiles: './src/setupTests.ts',
    coverage: {
      provider: 'v8',
      reportsDirectory: '../target/coverage/ui',
      reporter: ['text', 'json-summary'],
      include: ['src/**/*.{ts,tsx}'],
      exclude: [
        'src/**/*.test.{ts,tsx}',
        'src/setupTests.ts',
        'src/main.tsx',
        // Retained only as migration fixtures; the native renderer is the
        // sole production path and these adapters are never imported by it.
        'src/rendering/reactWebImageRendererAdapter.ts',
        'src/rendering/webImageRenderer.ts',
      ],
    },
  },
})
