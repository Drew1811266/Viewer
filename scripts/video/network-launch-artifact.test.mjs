import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { once } from 'node:events'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'

import { collectAppRuntimeIdentity } from './build-attestation.mjs'
import {
  DENY_NETWORK_SANDBOX_PROFILE,
  loadVerifiedNetworkLaunchArtifact,
  writeNetworkLaunchArtifact,
} from './network-launch-artifact.mjs'
import { nativeLaunchSpec } from './render-feasibility-assertions.mjs'

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')

function fixture(networkDisabled = true) {
  const root = mkdtempSync(path.join(tmpdir(), 'viewer-network-launch-'))
  const appPath = path.join(root, 'Viewer.app')
  const runtime = path.join(appPath, 'Contents/Resources/ViewerVideoRuntime')
  const tools = path.join(root, 'tools')
  const sandbox = path.join(tools, 'sandbox-exec')
  const open = path.join(tools, 'open')
  const invocation = path.join(root, 'launch-args.json')
  mkdirSync(path.join(appPath, 'Contents/MacOS'), { recursive: true })
  mkdirSync(path.join(runtime, 'bin'), { recursive: true })
  mkdirSync(tools)
  const appExecutable = path.join(appPath, 'Contents/MacOS/viewer-desktop')
  writeFileSync(path.join(appPath, 'Contents/Info.plist'), '<plist>fixture</plist>\n')
  writeFileSync(appExecutable, '#!/bin/sh\nexit 0\n')
  chmodSync(appExecutable, 0o755)
  writeFileSync(path.join(runtime, 'bin/ffprobe'), 'runtime\n')
  writeFileSync(path.join(runtime, 'runtime.lock.json'), '{}\n')
  writeFileSync(
    path.join(runtime, 'runtime.inventory.sha256'),
    `${sha256(readFileSync(path.join(runtime, 'bin/ffprobe')))}  bin/ffprobe\n`,
  )
  writeFileSync(
    sandbox,
    `#!/bin/sh
printf '["%s","%s","%s"]\n' "$1" "$2" "$3" > "${invocation}"
test "$1" = -p
test "$2" = '${DENY_NETWORK_SANDBOX_PROFILE}'
exec "$3"
`,
  )
  writeFileSync(open, `#!/bin/sh\nprintf '["open"]\n' > "${invocation}"\nexit 0\n`)
  chmodSync(sandbox, 0o755)
  chmodSync(open, 0o755)
  const binding = {
    schemaVersion: 1,
    runId: 'network-run-17',
    sourceIdentity: { manifestSha256: '1'.repeat(64) },
    machine: { model: 'Mac16,12', chip: 'Apple M4', macOS: 'macOS 26', uname: 'Darwin arm64' },
    appRuntime: collectAppRuntimeIdentity(appPath),
    bundleIdentity: {
      identifier: 'com.viewer.desktop',
      productName: 'Viewer',
      executableRelativePath: 'Contents/MacOS/viewer-desktop',
    },
    buildAttestationSha256: 'a'.repeat(64),
    bundleAuditSha256: 'b'.repeat(64),
  }
  const launch = nativeLaunchSpec({
    appExecutable,
    appPath,
    nativeLogPath: path.join(root, 'native.log'),
    networkDisabled,
    env: { PATH: '/malicious' },
    sandboxExecutable: sandbox,
    openExecutable: open,
  })
  return {
    root,
    appPath,
    appExecutable,
    binding,
    launch,
    invocation,
    artifactPath: path.join(root, 'network-launch.json'),
    sandbox,
  }
}

async function spawnFixture(f) {
  const child = spawn(f.launch.command, f.launch.argumentsList, { env: f.launch.env })
  await once(child, 'spawn')
  await once(child, 'exit')
  return child.pid
}

function writeValid(f, overrides = {}) {
  return writeNetworkLaunchArtifact({
    outputPath: f.artifactPath,
    appPath: f.appPath,
    binding: f.binding,
    launch: f.launch,
    launcherPid: 410,
    targetPid: 411,
    windowIdentity: { windowId: 73, title: 'Viewer Video Feasibility', width: 1024, height: 720 },
    ...overrides,
  })
}

function verify(f, overrides = {}) {
  return loadVerifiedNetworkLaunchArtifact({
    artifactPath: f.artifactPath,
    appPath: f.appPath,
    binding: f.binding,
    expectedSandboxExecutable: f.sandbox,
    ...overrides,
  })
}

test('network-disabled launch executes the exact app through the canonical deny profile', async () => {
  const f = fixture(true)
  await spawnFixture(f)
  assert.deepEqual(JSON.parse(readFileSync(f.invocation)), [
    '-p',
    DENY_NETWORK_SANDBOX_PROFILE,
    f.appExecutable,
  ])
  writeValid(f)
  const verified = verify(f)
  assert.equal(verified.artifact.networkPolicy, 'deny-all')
  assert.equal(verified.artifact.launch.executable, f.sandbox)
  assert.equal(verified.artifact.targetPid, 411)
})

test('unrestricted open launch cannot produce offline launch evidence', async () => {
  const f = fixture(false)
  await spawnFixture(f)
  assert.throws(() => writeValid(f), /deny-all|sandbox/i)
})

test('missing and stale launch artifacts reject', () => {
  const missing = fixture(true)
  assert.throws(() => verify(missing), /ENOENT/)

  for (const [field, mutate, pattern] of [
    ['run', (artifact) => (artifact.runId = 'stale-run'), /run/i],
    ['source', (artifact) => (artifact.sourceIdentitySha256 = 'f'.repeat(64)), /source/i],
    ['build', (artifact) => (artifact.buildAttestationSha256 = 'f'.repeat(64)), /build/i],
    ['audit', (artifact) => (artifact.bundleAuditSha256 = 'f'.repeat(64)), /audit/i],
    ['profile', (artifact) => (artifact.launch.sandboxProfile = '(version 1)(allow default)'), /profile/i],
  ]) {
    const f = fixture(true)
    writeValid(f)
    const artifact = JSON.parse(readFileSync(f.artifactPath))
    mutate(artifact)
    writeFileSync(f.artifactPath, `${JSON.stringify(artifact)}\n`)
    assert.throws(() => verify(f), pattern, field)
  }
})

test('current app/runtime mutation invalidates launch evidence', () => {
  const f = fixture(true)
  writeValid(f)
  writeFileSync(f.appExecutable, 'mutated after launch\n')
  assert.throws(() => verify(f), /app.runtime/i)
})

test('current runtime mutation invalidates launch evidence', () => {
  const f = fixture(true)
  writeValid(f)
  writeFileSync(
    path.join(f.appPath, 'Contents/Resources/ViewerVideoRuntime/bin/ffprobe'),
    'mutated runtime after launch\n',
  )
  assert.throws(() => verify(f), /app.runtime|inventory/i)
})
