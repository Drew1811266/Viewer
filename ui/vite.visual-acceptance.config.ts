import { fileURLToPath } from 'node:url'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const acceptanceEntry = fileURLToPath(new URL('./visual-acceptance.html', import.meta.url))
const acceptanceOutput = fileURLToPath(
  new URL('../target/viewer-visual-acceptance/site', import.meta.url),
)
const acceptanceImageFixtures = fileURLToPath(
  new URL('./src/acceptance/media/images', import.meta.url),
)

export default defineConfig({
  plugins: [react()],
  publicDir: acceptanceImageFixtures,
  build: {
    outDir: acceptanceOutput,
    emptyOutDir: true,
    rollupOptions: {
      input: acceptanceEntry,
    },
  },
})
