import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'

import {
  createBuildAttestation,
  loadVerifiedAuditArtifact,
  loadVerifiedBuildAttestation,
  prepareAcceptanceBundle,
  performanceResetPlan,
  writeAuditArtifact,
  writeBuildAttestation,
} from './build-attestation.mjs'

function fixture() {
  const root = mkdtempSync(path.join(tmpdir(), 'viewer-build-attestation-'))
  const sourceRoot = path.join(root, 'source')
  const appPath = path.join(root, 'Viewer.app')
  const runtime = path.join(appPath, 'Contents/Resources/ViewerVideoRuntime')
  execFileSync('git', ['init', '--quiet', sourceRoot])
  execFileSync('git', ['config', 'user.email', 'viewer-test@example.invalid'], { cwd: sourceRoot })
  execFileSync('git', ['config', 'user.name', 'Viewer Test'], { cwd: sourceRoot })
  writeFileSync(path.join(sourceRoot, 'source.rs'), 'fn fixture() {}\n')
  execFileSync('git', ['add', 'source.rs'], { cwd: sourceRoot })
  execFileSync('git', ['commit', '--quiet', '-m', 'fixture'], { cwd: sourceRoot })
  writeFileSync(path.join(sourceRoot, 'source.rs'), 'fn dirty_fixture() {}\n')
  mkdirSync(path.join(appPath, 'Contents/MacOS'), { recursive: true })
  mkdirSync(path.join(runtime, 'bin'), { recursive: true })
  mkdirSync(path.join(runtime, 'lib'))
  const executable = path.join(appPath, 'Contents/MacOS/viewer-desktop')
  writeFileSync(path.join(appPath, 'Contents/Info.plist'), '<plist>fixture identity</plist>\n')
  writeFileSync(executable, '#!/bin/sh\nexit 0\n')
  chmodSync(executable, 0o755)
  for (const relative of ['bin/ffmpeg', 'bin/ffprobe', 'lib/libmpv.2.dylib']) {
    writeFileSync(path.join(runtime, relative), `${relative} bytes\n`)
  }
  writeFileSync(path.join(runtime, 'runtime.lock.json'), '{"runtime":"fixture"}\n')
  const crypto = awaitImportCrypto()
  const inventory = ['bin/ffmpeg', 'bin/ffprobe', 'lib/libmpv.2.dylib']
    .map((relative) => `${crypto(readFileSync(path.join(runtime, relative)))}  ${relative}`)
    .join('\n')
  writeFileSync(path.join(runtime, 'runtime.inventory.sha256'), `${inventory}\n`)
  return {
    root,
    sourceRoot,
    appPath,
    runtime,
    executable,
    attestationPath: path.join(root, 'build-attestation.json'),
    auditPath: path.join(root, 'bundle-audit.json'),
    bundleIdentity: {
      identifier: 'com.viewer.desktop',
      productName: 'Viewer',
      executableRelativePath: 'Contents/MacOS/viewer-desktop',
    },
    buildConfig: { profile: 'debug', features: ['video-feasibility'], bundle: 'app' },
  }
}

function awaitImportCrypto() {
  return (bytes) => execFileSync('shasum', ['-a', '256'], { input: bytes }).toString().split(' ')[0]
}

function attest(f) {
  const value = createBuildAttestation(f)
  writeBuildAttestation(f.attestationPath, value)
  return value
}

test('skip-build accepts only the exact source and final app/runtime attested together', () => {
  const f = fixture()
  const expected = attest(f)
  const loaded = loadVerifiedBuildAttestation(f)
  assert.deepEqual(loaded.attestation, expected)
  assert.match(loaded.sha256, /^[a-f0-9]{64}$/)
})

test('skip-build rejects a missing attestation before native work', () => {
  const f = fixture()
  assert.throws(() => loadVerifiedBuildAttestation(f), /ENOENT/)
})

for (const [name, mutate, pattern] of [
  ['dirty source bytes', (f) => writeFileSync(path.join(f.sourceRoot, 'source.rs'), 'changed\n'), /source identity/i],
  ['app executable', (f) => writeFileSync(f.executable, 'changed app\n'), /app.runtime identity/i],
  ['bundle metadata', (f) => writeFileSync(path.join(f.appPath, 'Contents/Info.plist'), 'changed plist\n'), /app.runtime identity/i],
  ['runtime bytes', (f) => writeFileSync(path.join(f.runtime, 'bin/ffprobe'), 'changed runtime\n'), /app.runtime identity/i],
]) {
  test(`skip-build rejects stale attestation after ${name} change`, () => {
    const f = fixture()
    attest(f)
    mutate(f)
    assert.throws(() => loadVerifiedBuildAttestation(f), pattern)
  })
}

test('bundle audit artifact is bound to the attestation and rejects post-audit mutation', () => {
  const f = fixture()
  attest(f)
  writeAuditArtifact({
    appPath: f.appPath,
    attestationPath: f.attestationPath,
    outputPath: f.auditPath,
    mode: 'development',
  })
  assert.equal(loadVerifiedAuditArtifact({ ...f, auditPath: f.auditPath }).artifact.mode, 'development')
  writeFileSync(f.executable, 'post audit mutation\n')
  assert.throws(
    () => loadVerifiedAuditArtifact({ ...f, auditPath: f.auditPath }),
    /app.runtime identity/i,
  )
})

test('post-audit addition of an unlisted runtime file invalidates the artifact', () => {
  const f = fixture()
  attest(f)
  writeAuditArtifact({
    appPath: f.appPath,
    attestationPath: f.attestationPath,
    outputPath: f.auditPath,
    mode: 'development',
  })
  writeFileSync(path.join(f.runtime, 'lib/injected.dylib'), 'post audit bytes\n')
  assert.throws(
    () => loadVerifiedAuditArtifact({ ...f, auditPath: f.auditPath }),
    /app.runtime identity/i,
  )
})

test('default and skip-build preparation audit the final bundle before launch', async () => {
  for (const skipBuild of [false, true]) {
    const events = []
    await prepareAcceptanceBundle({
      skipBuild,
      build: async () => events.push('build'),
      signRuntime: async () => events.push('sign-runtime'),
      refreshInventory: async () => events.push('inventory'),
      signApp: async () => events.push('sign-app'),
      createOrVerifyAttestation: async (isSkip) => events.push(isSkip ? 'verify-attestation' : 'write-attestation'),
      audit: async () => events.push('audit'),
      verifyAudit: async () => events.push('verify-audit'),
    })
    events.push('launch')
    assert.deepEqual(
      events,
      skipBuild
        ? ['verify-attestation', 'audit', 'verify-audit', 'launch']
        : ['build', 'sign-runtime', 'inventory', 'sign-app', 'write-attestation', 'audit', 'verify-audit', 'launch'],
    )
  }
})

test('performance reset remounts the exact sample without an intermediary fixture', () => {
  assert.deepEqual(performanceResetPlan('h264-1080p60'), {
    fixtureId: 'h264-1080p60',
    action: '重新挂载当前样本',
  })
  assert.deepEqual(performanceResetPlan('hevc-4k30'), {
    fixtureId: 'hevc-4k30',
    action: '重新挂载当前样本',
  })
})
