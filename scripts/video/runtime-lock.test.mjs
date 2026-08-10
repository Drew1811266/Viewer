import assert from 'node:assert/strict'
import { mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { spawnSync } from 'node:child_process'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const script = fileURLToPath(new URL('./runtime-lock.mjs', import.meta.url))
const lock = fileURLToPath(new URL('./runtime.lock.json', import.meta.url))

test('list emits the complete build order from the validated lock', () => {
  const result = spawnSync(process.execPath, [script, 'list', lock], { encoding: 'utf8' })
  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(
    result.stdout
      .trim()
      .split('\n')
      .map((line) => line.split('\t')[0]),
    [
      'pkgconf',
      'freetype',
      'fribidi',
      'harfbuzz',
      'libass',
      'ffmpeg',
      'fast_float',
      'vulkan-headers',
      'libplacebo',
      'mpv',
    ],
  )
})

test('validate rejects a source whose digest is not SHA-256', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'viewer-video-lock-'))
  const invalidPath = join(directory, 'runtime.lock.json')
  const invalid = (await readFile(lock, 'utf8')).replace(
    '79721badcad1987dead9c3609eb4877ab9b58821c06bdacb824f2c8897c11f2a',
    'missing',
  )
  await writeFile(invalidPath, invalid)

  const result = spawnSync(process.execPath, [script, 'validate', invalidPath], {
    encoding: 'utf8',
  })
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /invalid SHA-256/)
})
