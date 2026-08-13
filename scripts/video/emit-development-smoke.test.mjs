import assert from 'node:assert/strict'
import { execFileSync, spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'

import { collectAppRuntimeIdentity } from './build-attestation.mjs'
import {
  DENY_NETWORK_SANDBOX_PROFILE,
  writeNetworkLaunchArtifact,
} from './network-launch-artifact.mjs'

const emitter = path.resolve('scripts/video/emit-development-smoke.mjs')
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')

function fixture() {
  const root = mkdtempSync(path.join(tmpdir(), 'viewer-smoke-artifact-'))
  const appPath = path.join(root, 'Viewer.app')
  const runtime = path.join(appPath, 'Contents/Resources/ViewerVideoRuntime')
  mkdirSync(path.join(appPath, 'Contents/MacOS'), { recursive: true })
  mkdirSync(path.join(runtime, 'bin'), { recursive: true })
  const executable = path.join(appPath, 'Contents/MacOS/viewer-desktop')
  writeFileSync(path.join(appPath, 'Contents/Info.plist'), '<plist>fixture identity</plist>\n')
  writeFileSync(executable, '#!/bin/sh\nexit 0\n')
  chmodSync(executable, 0o755)
  writeFileSync(path.join(runtime, 'bin/ffprobe'), 'runtime\n')
  writeFileSync(path.join(runtime, 'runtime.lock.json'), '{}\n')
  writeFileSync(
    path.join(runtime, 'runtime.inventory.sha256'),
    `${sha256(readFileSync(path.join(runtime, 'bin/ffprobe')))}  bin/ffprobe\n`,
  )
  const appRuntime = collectAppRuntimeIdentity(appPath)
  const bundleIdentity = {
    identifier: 'com.viewer.desktop',
    productName: 'Viewer',
    executableRelativePath: 'Contents/MacOS/viewer-desktop',
  }
  const audit = {
    schemaVersion: 1,
    mode: 'development',
    attestationSha256: 'a'.repeat(64),
    appRuntime,
    bundleIdentity,
    checks: {
      inventoryAndHashes: true,
      architectureMatch: true,
      loaderContainment: true,
      appleAbsoluteDependenciesOnly: true,
      bundledRuntimeOfflineProbe: true,
    },
  }
  const auditPath = path.join(root, 'audit.json')
  writeFileSync(auditPath, `${JSON.stringify(audit, null, 2)}\n`)
  const binding = {
    schemaVersion: 1,
    runId: 'smoke-run-1',
    sourceIdentity: { manifestSha256: '1'.repeat(64) },
    machine: { model: 'Mac16,12', chip: 'Apple M4', macOS: 'macOS 26', uname: 'Darwin arm64' },
    appRuntime,
    bundleIdentity,
    buildAttestationSha256: audit.attestationSha256,
    bundleAuditSha256: sha256(readFileSync(auditPath)),
  }
  const launchPath = path.join(root, 'network-launch.json')
  writeNetworkLaunchArtifact({
    outputPath: launchPath,
    appPath,
    binding,
    launch: {
      command: '/usr/bin/sandbox-exec',
      argumentsList: ['-p', DENY_NETWORK_SANDBOX_PROFILE, executable],
      networkPolicy: 'deny-all',
      sandboxProfile: DENY_NETWORK_SANDBOX_PROFILE,
    },
    launcherPid: 501,
    targetPid: 502,
    windowIdentity: {
      windowId: 81,
      title: 'Viewer Video Feasibility',
      width: 1024,
      height: 720,
    },
  })
  binding.networkLaunchSha256 = sha256(readFileSync(launchPath))
  const matrixPath = path.join(root, 'matrix.json')
  const diagnosticPath = path.join(root, 'diagnostic.json')
  writeFileSync(matrixPath, `${JSON.stringify({ binding })}\n`)
  writeFileSync(diagnosticPath, `${JSON.stringify({ binding })}\n`)
  return {
    root,
    appPath,
    audit,
    auditPath,
    launchPath,
    matrixPath,
    diagnosticPath,
    outputPath: path.join(root, 'smoke.json'),
  }
}

function run(f) {
  return execFileSync(
    process.execPath,
    [
      emitter,
      f.appPath,
      f.matrixPath,
      f.diagnosticPath,
      f.auditPath,
      f.launchPath,
      f.outputPath,
    ],
    { encoding: 'utf8' },
  )
}

test('smoke copies the verified audit and launch artifacts instead of minting booleans', () => {
  const f = fixture()
  run(f)
  const smoke = JSON.parse(readFileSync(f.outputPath))
  assert.deepEqual(smoke.audit.checks, f.audit.checks)
  assert.equal(smoke.audit.artifactSha256, sha256(readFileSync(f.auditPath)))
  assert.equal(smoke.launch.artifactSha256, sha256(readFileSync(f.launchPath)))
  assert.equal(smoke.launch.networkPolicy, 'deny-all')
  assert.equal(smoke.networkDisabled, true)
})

test('direct emitter invocation without a launch artifact rejects', () => {
  const f = fixture()
  const result = spawnSync(
    process.execPath,
    [emitter, f.appPath, f.matrixPath, f.diagnosticPath, f.auditPath, f.outputPath],
    { encoding: 'utf8' },
  )
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /usage|launch/i)
})

test('smoke rejects a missing or stale verifier artifact', () => {
  const missing = fixture()
  const missingResult = spawnSync(
    process.execPath,
    [
      emitter,
      missing.appPath,
      missing.matrixPath,
      missing.diagnosticPath,
      path.join(missing.root, 'missing.json'),
      missing.launchPath,
      missing.outputPath,
    ],
    { encoding: 'utf8' },
  )
  assert.notEqual(missingResult.status, 0)

  const stale = fixture()
  stale.audit.attestationSha256 = 'f'.repeat(64)
  writeFileSync(stale.auditPath, `${JSON.stringify(stale.audit)}\n`)
  const staleResult = spawnSync(
    process.execPath,
    [
      emitter,
      stale.appPath,
      stale.matrixPath,
      stale.diagnosticPath,
      stale.auditPath,
      stale.launchPath,
      stale.outputPath,
    ],
    { encoding: 'utf8' },
  )
  assert.notEqual(staleResult.status, 0)
  assert.match(staleResult.stderr, /audit artifact/i)
})
