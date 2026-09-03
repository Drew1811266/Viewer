import assert from 'node:assert/strict'
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'

import { prepareDevelopmentVideoRuntime } from './development-video-runtime.mjs'

async function runtimeFixture() {
  const root = await mkdtemp(path.join(os.tmpdir(), 'viewer-development-runtime-'))
  const sourcePath = path.join(root, 'source', 'ViewerVideoRuntime')
  const destinationPath = path.join(root, 'target', 'debug', 'ViewerVideoRuntime')
  const verifierPath = path.join(root, 'verify-runtime.sh')
  const verifierLogPath = path.join(root, 'verifier.log')

  await mkdir(path.join(sourcePath, 'bin'), { recursive: true })
  await writeFile(path.join(sourcePath, 'bin', 'ffmpeg'), 'reviewed runtime\n', {
    mode: 0o755,
  })
  await writeFile(path.join(sourcePath, 'runtime.inventory.sha256'), 'fixture inventory\n')
  await writeFile(path.join(sourcePath, 'runtime.lock.json'), '{"fixture":true}\n')
  await writeFile(
    verifierPath,
    [
      '#!/bin/sh',
      'set -eu',
      'test -x "$VIEWER_VIDEO_STAGE_DIR/bin/ffmpeg"',
      'test -f "$VIEWER_VIDEO_STAGE_DIR/runtime.inventory.sha256"',
      'printf \'%s\\n\' "$VIEWER_VIDEO_STAGE_DIR" >> "$VIEWER_TEST_VERIFIER_LOG"',
      '',
    ].join('\n'),
    { mode: 0o755 },
  )

  return {
    root,
    sourcePath,
    destinationPath,
    verifierPath,
    env: {
      ...process.env,
      VIEWER_TEST_VERIFIER_LOG: verifierLogPath,
    },
    verifierLogPath,
  }
}

test('reuses an unchanged verified development runtime', async () => {
  const fixture = await runtimeFixture()

  try {
    await mkdir(path.dirname(fixture.destinationPath), { recursive: true })
    await cp(fixture.sourcePath, fixture.destinationPath, { recursive: true })

    const result = await prepareDevelopmentVideoRuntime(fixture)

    assert.equal(result, 'reused')
    assert.equal(
      await readFile(path.join(fixture.destinationPath, 'bin', 'ffmpeg'), 'utf8'),
      'reviewed runtime\n',
    )
    assert.deepEqual(
      (await readFile(fixture.verifierLogPath, 'utf8')).trim().split('\n'),
      [fixture.sourcePath, fixture.destinationPath],
    )
  } finally {
    await rm(fixture.root, { recursive: true, force: true })
  }
})

test('replaces an incomplete development runtime with the verified source', async () => {
  const fixture = await runtimeFixture()
  const staleFile = path.join(fixture.destinationPath, 'stale.txt')

  try {
    await mkdir(path.dirname(fixture.destinationPath), { recursive: true })
    await cp(fixture.sourcePath, fixture.destinationPath, { recursive: true })
    await rm(path.join(fixture.destinationPath, 'runtime.inventory.sha256'))
    await writeFile(staleFile, 'must not survive replacement\n')

    const result = await prepareDevelopmentVideoRuntime(fixture)

    assert.equal(result, 'installed')
    assert.equal(
      await readFile(path.join(fixture.destinationPath, 'runtime.inventory.sha256'), 'utf8'),
      'fixture inventory\n',
    )
    await assert.rejects(readFile(staleFile, 'utf8'), { code: 'ENOENT' })
  } finally {
    await rm(fixture.root, { recursive: true, force: true })
  }
})

test('replaces a verified development runtime when its identity is stale', async () => {
  const fixture = await runtimeFixture()

  try {
    await mkdir(path.dirname(fixture.destinationPath), { recursive: true })
    await cp(fixture.sourcePath, fixture.destinationPath, { recursive: true })
    await writeFile(
      path.join(fixture.destinationPath, 'runtime.inventory.sha256'),
      'older inventory\n',
    )

    const result = await prepareDevelopmentVideoRuntime(fixture)

    assert.equal(result, 'installed')
    assert.equal(
      await readFile(path.join(fixture.destinationPath, 'runtime.inventory.sha256'), 'utf8'),
      'fixture inventory\n',
    )
  } finally {
    await rm(fixture.root, { recursive: true, force: true })
  }
})
