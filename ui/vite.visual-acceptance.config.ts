import { fileURLToPath } from 'node:url'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const acceptanceEntry = fileURLToPath(new URL('./visual-acceptance.html', import.meta.url))
const acceptanceOutput = fileURLToPath(
  new URL('../target/viewer-visual-acceptance/site', import.meta.url),
)

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: acceptanceOutput,
    emptyOutDir: true,
    rollupOptions: {
      input: acceptanceEntry,
    },
  },
})
