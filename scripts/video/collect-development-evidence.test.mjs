import assert from 'node:assert/strict'
import { execFileSync, spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { chmodSync, mkdtempSync, readFileSync, statSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'

const collector = path.resolve('scripts/video/collect-development-evidence.mjs')

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}

function git(root, args) {
  return execFileSync('git', args, { cwd: root })
}

function sourceIdentity(root) {
  const commit = git(root, ['rev-parse', 'HEAD']).toString().trim()
  const trackedDiffSha256 = sha256(git(root, ['diff', '--binary', '--no-ext-diff', 'HEAD', '--']))
  const untracked = git(root, ['ls-files', '--others', '--exclude-standard', '-z'])
    .toString()
    .split('\0')
    .filter(Boolean)
    .sort()
    .map((relativePath) => {
      const file = path.join(root, relativePath)
      return {
        path: relativePath,
        mode: statSync(file).mode & 0o777,
        sha256: sha256(readFileSync(file)),
      }
    })
  const manifest = { commit, trackedDiffSha256, untracked }
  return { ...manifest, manifestSha256: sha256(JSON.stringify(manifest)) }
}

function inputs() {
  const root = mkdtempSync(path.join(tmpdir(), 'viewer-development-evidence-'))
  const sourceRoot = path.join(root, 'source')
  execFileSync('git', ['init', '--quiet', sourceRoot])
  execFileSync('git', ['config', 'user.email', 'viewer-test@example.invalid'], { cwd: sourceRoot })
  execFileSync('git', ['config', 'user.name', 'Viewer Test'], { cwd: sourceRoot })
  writeFileSync(path.join(sourceRoot, 'tracked-source.rs'), 'fn reviewed() {}\n')
  git(sourceRoot, ['add', 'tracked-source.rs'])
  git(sourceRoot, ['commit', '--quiet', '-m', 'fixture'])
  writeFileSync(path.join(sourceRoot, 'tracked-source.rs'), 'fn current_dirty_bytes() {}\n')
  writeFileSync(path.join(sourceRoot, 'task14-untracked.mjs'), 'export const task14 = true\n')
  chmodSync(path.join(sourceRoot, 'task14-untracked.mjs'), 0o755)

  const matrixPath = path.join(root, 'matrix.json')
  const diagnosticPath = path.join(root, 'diagnostic.json')
  const smokePath = path.join(root, 'smoke.json')
  const outputPath = path.join(root, 'development-evidence.json')
  const runId = 'task14-run-41'
  const machine = {
    model: 'Mac16,12',
    chip: 'Apple M4',
    macOS: 'macOS 26.5.2',
    uname: 'Darwin fixture arm64',
  }
  const fixtures = [
    { id: 'h264-1080p', sha256: '1'.repeat(64) },
    { id: 'hevc-portrait', sha256: '2'.repeat(64) },
    { id: 'vfr-step', sha256: '3'.repeat(64) },
    { id: 'h264-1080p60', sha256: '4'.repeat(64) },
    { id: 'hevc-4k30', sha256: '5'.repeat(64) },
  ]
  const binding = {
    schemaVersion: 1,
    runId,
    sourceIdentity: sourceIdentity(sourceRoot),
    machine,
    fixtures,
    appRuntime: {
      appExecutableSha256: '6'.repeat(64),
      runtimeInventorySha256: '7'.repeat(64),
      runtimeLockSha256: '8'.repeat(64),
      runtimeFiles: [{ path: 'bin/ffprobe', sha256: '9'.repeat(64) }],
    },
    bundleIdentity: {
      identifier: 'com.viewer.desktop',
      productName: 'Viewer',
      executableRelativePath: 'Contents/MacOS/viewer-desktop',
    },
    buildConfig: { profile: 'debug', features: ['video-feasibility'], bundle: 'app' },
    buildAttestationSha256: 'a'.repeat(64),
    bundleAuditSha256: 'b'.repeat(64),
    networkLaunchSha256: 'c'.repeat(64),
  }
  const matrix = {
    binding: structuredClone(binding),
    generatedAt: '2026-08-13T04:29:50.603Z',
    machine,
    rows: {
      'first-frame-ready': true,
      'frame-step-forward': true,
      'frame-step-backward': true,
      'h264-videotoolbox': true,
      'hevc-videotoolbox': true,
      '30-mount-unmount-baseline': true,
      'timeline-preview': true,
      'h264-1080p60-performance': true,
      'hevc-4k30-performance': false,
    },
    rowRunIds: {
      'first-frame-ready': runId,
      'frame-step-forward': runId,
      'frame-step-backward': runId,
      'h264-videotoolbox': runId,
      'hevc-videotoolbox': runId,
      '30-mount-unmount-baseline': runId,
      'timeline-preview': runId,
      'h264-1080p60-performance': runId,
      'hevc-4k30-performance': null,
    },
    lifecycle: {
      cycles: 30,
      before: { clients: 2, renderContexts: 3, surfaces: 4 },
      after: { clients: 2, renderContexts: 3, surfaces: 4 },
      fixtureSequence: Array.from({ length: 30 }, (_, index) =>
        index % 2 === 0 ? 'h264-1080p' : 'hevc-portrait',
      ),
      measuredResources: ['clients', 'renderContexts', 'surfaces'],
    },
    native: {
      'hevc-4k30': {
        hwdec: 'videotoolbox',
        videoOutput: 'libmpv',
        firstRevealedFrame: { screenshot: '/auditable/hevc-4k30-first-revealed.png' },
      },
    },
    error: 'AcceptanceError: Unable to activate Viewer before media selection',
    performance: [
      {
        id: 'h264-1080p60',
        codec: 'h264',
        width: 1920,
        height: 1080,
        framesPerSecond: 60,
        bitDepth: 8,
        hwdec: 'videotoolbox',
        videoOutput: 'libmpv',
        averageCommandLatencyMs: 0.01965,
        postWarmupRenderedFrames: 76,
        postWarmupDroppedFrames: 0,
      },
    ],
  }
  const diagnostic = {
    binding: structuredClone(binding),
    generatedAt: '2026-08-13T04:19:38.222Z',
    sequence: [
      {
        stage: 'hevc-4k30-first-passive',
        fixture: fixtures[4],
        ready: true,
        generation: {
          mountReturned: 1,
          frameUpdates: 2,
          pictureFrames: 2,
          reveals: 1,
          eventEmits: 3,
        },
      },
      {
        stage: 'hevc-4k30-reopen-passive',
        fixture: fixtures[4],
        ready: true,
        generation: {
          mountReturned: 1,
          frameUpdates: 2,
          pictureFrames: 2,
          reveals: 1,
          eventEmits: 3,
        },
      },
    ],
    error: null,
  }
  writeFileSync(matrixPath, `${JSON.stringify(matrix)}\n`)
  writeFileSync(diagnosticPath, `${JSON.stringify(diagnostic)}\n`)
  const smoke = {
    schemaVersion: 1,
    status: 'passed',
    binding: structuredClone(binding),
    matrixSha256: sha256(readFileSync(matrixPath)),
    diagnosticSha256: sha256(readFileSync(diagnosticPath)),
    audit: {
      mode: 'development',
      artifactSha256: binding.bundleAuditSha256,
      attestationSha256: binding.buildAttestationSha256,
      appRuntime: structuredClone(binding.appRuntime),
      bundleIdentity: structuredClone(binding.bundleIdentity),
      checks: {
        inventoryAndHashes: true,
        architectureMatch: true,
        loaderContainment: true,
        appleAbsoluteDependenciesOnly: true,
        bundledRuntimeOfflineProbe: true,
      },
    },
    launch: {
      artifactSha256: binding.networkLaunchSha256,
      networkPolicy: 'deny-all',
      executable: '/usr/bin/sandbox-exec',
      arguments: [
        '-p',
        '(version 1)(allow default)(deny network*)',
        '/fixture/Viewer.app/Contents/MacOS/viewer-desktop',
      ],
      sandboxProfileSha256: sha256('(version 1)(allow default)(deny network*)'),
      launcherPid: 501,
      targetPid: 502,
      windowIdentity: {
        windowId: 81,
        title: 'Viewer Video Feasibility',
        width: 1024,
        height: 720,
      },
    },
    networkDisabled: true,
    bundledRuntimeOnly: true,
    hostDependenciesAbsent: true,
  }
  writeFileSync(smokePath, `${JSON.stringify(smoke)}\n`)
  return {
    root,
    sourceRoot,
    matrixPath,
    diagnosticPath,
    smokePath,
    outputPath,
    matrix,
    diagnostic,
    smoke,
  }
}

function persist(fixture) {
  writeFileSync(fixture.matrixPath, `${JSON.stringify(fixture.matrix)}\n`)
  writeFileSync(fixture.diagnosticPath, `${JSON.stringify(fixture.diagnostic)}\n`)
  fixture.smoke.matrixSha256 = sha256(readFileSync(fixture.matrixPath))
  fixture.smoke.diagnosticSha256 = sha256(readFileSync(fixture.diagnosticPath))
  writeFileSync(fixture.smokePath, `${JSON.stringify(fixture.smoke)}\n`)
}

function run(fixture) {
  persist(fixture)
  return execFileSync(
    process.execPath,
    [
      collector,
      fixture.matrixPath,
      fixture.diagnosticPath,
      fixture.smokePath,
      fixture.outputPath,
      fixture.sourceRoot,
    ],
    { encoding: 'utf8', stdio: 'pipe' },
  )
}

function reject(fixture, pattern) {
  persist(fixture)
  const result = spawnSync(
    process.execPath,
    [
      collector,
      fixture.matrixPath,
      fixture.diagnosticPath,
      fixture.smokePath,
      fixture.outputPath,
      fixture.sourceRoot,
    ],
    { encoding: 'utf8' },
  )
  assert.notEqual(result.status, 0, result.stdout)
  assert.match(result.stderr, pattern)
}

test('exact source, run, machine, fixture, app/runtime, and smoke bindings produce evidence', () => {
  const fixture = inputs()
  run(fixture)
  const evidence = JSON.parse(readFileSync(fixture.outputPath, 'utf8'))

  assert.equal(evidence.schemaVersion, 3)
  assert.equal(evidence.developmentDecision.status, 'passed')
  assert.deepEqual(evidence.binding, fixture.matrix.binding)
  assert.deepEqual(evidence.lifecycle, fixture.matrix.lifecycle)
  assert.equal(evidence.offline.status, 'passed')
})

test('an old diagnostic without an evidence binding is rejected without timestamp backfill', () => {
  const fixture = inputs()
  delete fixture.diagnostic.binding
  reject(fixture, /diagnostic.*binding/i)
})

test('current dirty bytes must exactly match the source identity captured by the native run', () => {
  const fixture = inputs()
  writeFileSync(path.join(fixture.sourceRoot, 'tracked-source.rs'), 'fn changed_after_native_run() {}\n')
  reject(fixture, /current source identity/i)
})

test('mismatched fixture, machine, and run bindings are each rejected', () => {
  for (const [field, mutate] of [
    ['fixture', (fixture) => (fixture.diagnostic.binding.fixtures[4].sha256 = '9'.repeat(64))],
    ['machine', (fixture) => (fixture.diagnostic.binding.machine.model = 'OtherMac')],
    ['run', (fixture) => (fixture.diagnostic.binding.runId = 'other-run')],
  ]) {
    const fixture = inputs()
    mutate(fixture)
    reject(fixture, new RegExp(`${field}.*binding|binding.*${field}`, 'i'))
  }
})

test('lifecycle evidence is read from the runner and rejects sequence, count, or baseline tampering', () => {
  for (const [label, mutate] of [
    ['cycles', (fixture) => (fixture.matrix.lifecycle.cycles = 29)],
    [
      'navigation',
      (fixture) => (fixture.matrix.lifecycle.fixtureSequence = Array(30).fill('h264-1080p')),
    ],
    ['baseline', (fixture) => (fixture.matrix.lifecycle.after.clients += 1)],
  ]) {
    const fixture = inputs()
    mutate(fixture)
    reject(fixture, new RegExp(`lifecycle.*${label}|${label}.*lifecycle`, 'i'))
  }
})

test('lifecycle rejects a 29-plus-1 grouped path that visits two fixtures without alternating', () => {
  const fixture = inputs()
  fixture.matrix.lifecycle.fixtureSequence = [
    ...Array.from({ length: 29 }, () => 'h264-1080p'),
    'hevc-portrait',
  ]
  reject(fixture, /alternate/i)
})

test('a cryptographically stale smoke artifact cannot be promoted by an environment bit', () => {
  const fixture = inputs()
  fixture.smoke.matrixSha256 = '0'.repeat(64)
  persist(fixture)
  fixture.smoke.matrixSha256 = '0'.repeat(64)
  writeFileSync(fixture.smokePath, `${JSON.stringify(fixture.smoke)}\n`)
  const result = spawnSync(
    process.execPath,
    [
      collector,
      fixture.matrixPath,
      fixture.diagnosticPath,
      fixture.smokePath,
      fixture.outputPath,
      fixture.sourceRoot,
    ],
    { encoding: 'utf8', env: { ...process.env, VIEWER_VIDEO_DEVELOPER_SMOKE: '1' } },
  )
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /smoke.*matrix.*hash/i)
})

test('an unrestricted launch cannot satisfy offline evidence even with a legacy true bit', () => {
  const fixture = inputs()
  fixture.smoke.launch.networkPolicy = 'unrestricted'
  fixture.smoke.networkDisabled = true
  reject(fixture, /deny-all|launch/i)
})

test('unverified remains allowlisted for 4K performance only', () => {
  const fixture = inputs()
  fixture.matrix.rows['h264-1080p60-performance'] = false
  fixture.matrix.performance = []
  reject(fixture, /required native acceptance row did not pass: h264-1080p60-performance/)
})

test('4K performance cannot be unverified without native initial and reopen readiness evidence', () => {
  const fixture = inputs()
  fixture.diagnostic.sequence = fixture.diagnostic.sequence.slice(0, 1)
  reject(fixture, /4K native readiness evidence is incomplete/)
})
