import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'

const manifestPath = new URL('../../tests/fixtures/videos/manifest.json', import.meta.url)

const expected = [
  ['h264-aac', 'mp4', 'h264', 'aac'],
  ['hevc-portrait', 'mov', 'hevc', undefined],
  ['prores', 'mov', 'prores', undefined],
  ['vp9-opus', 'webm', 'vp9', 'opus'],
  ['av1', 'mkv', 'av1', undefined],
  ['vfr', 'mp4', undefined, undefined],
  ['silent', 'mp4', undefined, null],
  ['truncated', 'mp4', undefined, undefined],
  ['unsupported-codec', 'mkv', undefined, undefined],
]

test('the redistribution-safe media matrix records all nine required fixture contracts', async () => {
  const manifest = JSON.parse(await readFile(manifestPath, 'utf8'))

  assert.equal(manifest.schemaVersion, 2)
  assert.deepEqual(
    manifest.fixtures.map(({ id, container, videoCodec, audioCodec }) => [
      id,
      container,
      videoCodec,
      audioCodec,
    ]),
    expected,
  )
  assert.equal(manifest.fixtures.find(({ id }) => id === 'hevc-portrait').rotation, 90)
  assert.equal(manifest.fixtures.find(({ id }) => id === 'vfr').variableFrameRate, true)
  assert.equal(manifest.fixtures.find(({ id }) => id === 'truncated').expectedFailure, 'damaged')
  assert.equal(
    manifest.fixtures.find(({ id }) => id === 'unsupported-codec').expectedFailure,
    'unsupported',
  )

  for (const fixture of manifest.fixtures) {
    assert.match(fixture.file, /^[a-z0-9-]+\.(?:mkv|mov|mp4|webm)$/)
    assert.match(fixture.sha256, /^[a-f0-9]{64}$/)
    assert.ok(
      ['reviewed-runtime-generated', 'repository-embedded-reviewed-seed'].includes(
        fixture.redistribution,
      ),
    )
    assert.equal(fixture.redistributable, true)
  }
})
