import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'

import { nativeLaunchSpec } from './render-feasibility-assertions.mjs'

test('clean-machine smoke rejects missing app bundles', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'viewer-smoke-contract-'))
  assert.throws(
    () => execFileSync('scripts/video/run-clean-machine-smoke.sh', ['--mode', 'development', path.join(root, 'Viewer.app')]),
    /Viewer app is missing/,
  )
})

test('future signed mode fails closed when the bundle cannot pass the strict runtime audit', () => {
  const root = mkdtempSync(path.join(tmpdir(), 'viewer-signed-contract-'))
  const app = path.join(root, 'Viewer.app')
  const runtime = path.join(app, 'Contents', 'Resources', 'ViewerVideoRuntime')
  mkdirSync(path.join(app, 'Contents', 'MacOS'), { recursive: true })
  mkdirSync(path.join(runtime, 'bin'), { recursive: true })
  mkdirSync(path.join(runtime, 'lib'), { recursive: true })
  for (const relative of ['Contents/MacOS/viewer-desktop', 'Contents/Resources/ViewerVideoRuntime/bin/ffmpeg', 'Contents/Resources/ViewerVideoRuntime/bin/ffprobe']) {
    const file = path.join(app, relative)
    writeFileSync(file, '#!/bin/sh\nexit 0\n', { mode: 0o755 })
  }
  for (const relative of ['lib/libmpv.2.dylib', 'runtime.lock.json', 'runtime.inventory.sha256']) {
    writeFileSync(path.join(runtime, relative), '')
  }
  assert.throws(
    () => execFileSync('scripts/video/run-clean-machine-smoke.sh', ['--mode', 'signed', app], { stdio: 'pipe' }),
    /staged runtime lock differs from the reviewed lock/,
  )
})

test('development mode uses a deny-network sandbox and restricted PATH', () => {
  const launch = nativeLaunchSpec({
    appExecutable: '/fixture/Viewer.app/Contents/MacOS/viewer-desktop',
    appPath: '/fixture/Viewer.app',
    nativeLogPath: '/fixture/native.log',
    networkDisabled: true,
    env: { HOME: '/fixture/home', PATH: '/malicious/bin' },
  })
  assert.equal(launch.command, '/usr/bin/sandbox-exec')
  assert.deepEqual(launch.argumentsList, [
    '-p',
    '(version 1)(allow default)(deny network*)',
    '/fixture/Viewer.app/Contents/MacOS/viewer-desktop',
  ])
  assert.equal(launch.env.PATH, '/usr/bin:/bin:/usr/sbin:/sbin')
})
