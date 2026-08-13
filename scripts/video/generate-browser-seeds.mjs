import { writeFile } from 'node:fs/promises'
import { chromium } from 'playwright'

const [output] = process.argv.slice(2)
if (!output) throw new Error('output path required')

const browser = await chromium.launch({
  executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  headless: true,
})
try {
  const page = await browser.newPage()
  const base64 = await page.evaluate(async () => {
    const canvas = document.createElement('canvas')
    canvas.width = 320
    canvas.height = 180
    const context = canvas.getContext('2d')
    const stream = canvas.captureStream(24)
    const mimeType = 'video/webm;codecs=vp9'
    if (!MediaRecorder.isTypeSupported(mimeType)) {
      throw new Error('Chrome does not expose a VP9 MediaRecorder')
    }
    const chunks = []
    const recorder = new MediaRecorder(stream, { mimeType, videoBitsPerSecond: 300_000 })
    recorder.ondataavailable = ({ data }) => {
      if (data.size > 0) chunks.push(data)
    }
    const stopped = new Promise((resolve) => recorder.addEventListener('stop', resolve, { once: true }))
    recorder.start(100)
    const started = performance.now()
    while (performance.now() - started < 1200) {
      const elapsed = performance.now() - started
      context.fillStyle = `hsl(${Math.floor(elapsed / 4) % 360} 80% 48%)`
      context.fillRect(0, 0, canvas.width, canvas.height)
      context.fillStyle = 'white'
      context.fillRect(Math.floor(elapsed / 8) % 280, 70, 40, 40)
      await new Promise((resolve) => requestAnimationFrame(resolve))
    }
    recorder.stop()
    await stopped
    const bytes = new Uint8Array(await new Blob(chunks, { type: mimeType }).arrayBuffer())
    let binary = ''
    for (const byte of bytes) binary += String.fromCharCode(byte)
    return btoa(binary)
  })
  await writeFile(output, Buffer.from(base64, 'base64'))
} finally {
  await browser.close()
}
