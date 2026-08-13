import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { collectSourceIdentity, sha256 } from './source-identity.mjs'

const [matrixPath, diagnosticPath, smokePath, outputPath, sourceRootArgument] = process.argv.slice(2)
if (!matrixPath || !diagnosticPath || !smokePath || !outputPath) {
  throw new Error(
    'usage: collect-development-evidence.mjs <matrix-result.json> <generation-diagnostic.json> <smoke-evidence.json> <output.json> [source-root]',
  )
}

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url))
const sourceRoot = path.resolve(sourceRootArgument ?? path.join(scriptDirectory, '../..'))
const matrixBytes = readFileSync(matrixPath)
const diagnosticBytes = readFileSync(diagnosticPath)
const matrix = JSON.parse(matrixBytes)
const diagnostic = JSON.parse(diagnosticBytes)
const smoke = JSON.parse(readFileSync(smokePath))
const binding = validateBindings(matrix, diagnostic, smoke, sourceRoot)
const requiredRows = [
  'first-frame-ready',
  'frame-step-forward',
  'frame-step-backward',
  'h264-videotoolbox',
  'hevc-videotoolbox',
  '30-mount-unmount-baseline',
  'timeline-preview',
  'h264-1080p60-performance',
]

for (const row of requiredRows) {
  if (matrix.rows?.[row] !== true) {
    throw new Error(`required native acceptance row did not pass: ${row}`)
  }
  if (matrix.rowRunIds?.[row] !== binding.runId) {
    throw new Error(`required native acceptance row is not bound to the current run: ${row}`)
  }
}

const lifecycle = validateLifecycle(matrix.lifecycle)
const fourKNative = matrix.native?.['hevc-4k30']
const fourKStages = new Map(diagnostic.sequence?.map((entry) => [entry.stage, entry]) ?? [])
const expectedFourKFixture = binding.fixtures.find((fixture) => fixture.id === 'hevc-4k30')
if (expectedFourKFixture === undefined) {
  throw new Error('fixture binding is missing hevc-4k30')
}
for (const stage of ['hevc-4k30-first-passive', 'hevc-4k30-reopen-passive']) {
  const entry = fourKStages.get(stage)
  const generation = entry?.generation
  if (
    !same(entry?.fixture, expectedFourKFixture) ||
    entry?.ready !== true ||
    generation?.mountReturned !== 1 ||
    !(generation.frameUpdates > 0) ||
    !(generation.pictureFrames > 0) ||
    generation.reveals !== 1 ||
    !(generation.eventEmits > 0)
  ) {
    throw new Error(`4K native readiness evidence is incomplete: ${stage}`)
  }
}
if (
  diagnostic.error !== null ||
  fourKNative?.hwdec !== 'videotoolbox' ||
  fourKNative?.videoOutput !== 'libmpv' ||
  typeof fourKNative?.firstRevealedFrame?.screenshot !== 'string'
) {
  throw new Error('4K native readiness evidence is incomplete: matrix binding')
}

const h264Sample = matrix.performance?.find((sample) => sample.id === 'h264-1080p60')
validatePassedPerformance(h264Sample, {
  id: 'h264-1080p60',
  codec: 'h264',
  width: 1920,
  height: 1080,
  framesPerSecond: 60,
})

const fourKRow = matrix.rows?.['hevc-4k30-performance']
const fourKSample = matrix.performance?.find((sample) => sample.id === 'hevc-4k30')
let fourKPerformance
if (fourKRow === true) {
  validatePassedPerformance(fourKSample, {
    id: 'hevc-4k30',
    codec: 'hevc',
    width: 3840,
    height: 2160,
    framesPerSecond: 30,
  })
  if (matrix.rowRunIds?.['hevc-4k30-performance'] !== binding.runId) {
    throw new Error('passed 4K performance is not bound to the current run')
  }
  fourKPerformance = passedPerformance(fourKSample, binding.runId)
} else if (fourKRow === false) {
  if (fourKSample !== undefined) {
    throw new Error('unverified 4K performance must not contain a performance measurement')
  }
  if (matrix.rowRunIds?.['hevc-4k30-performance'] !== null) {
    throw new Error('unverified 4K performance must retain a null source run ID')
  }
  if (!String(matrix.error).includes('Unable to activate Viewer')) {
    throw new Error('4K performance is not eligible for the activation-race unverified status')
  }
  fourKPerformance = {
    id: 'hevc-4k30',
    status: 'unverified',
    reason:
      'The 4K dropped-frame window was not collected because the acceptance controller failed to activate Viewer before media selection; native 4K initial and reopen readiness are evidenced separately.',
    sourceRunId: null,
    codec: 'hevc',
    width: 3840,
    height: 2160,
    framesPerSecond: 30,
    bitDepth: null,
    hwdec: null,
    videoOutput: null,
    averageCommandLatencyMs: null,
    postWarmupRenderedFrames: null,
    postWarmupDroppedFrames: null,
  }
} else {
  throw new Error('4K performance row is missing; only an explicit false row may be unverified')
}

validateSmoke(smoke, binding, matrixBytes, diagnosticBytes)
const evidence = {
  schemaVersion: 3,
  mode: 'development',
  binding,
  developmentDecision: {
    status: 'passed',
    reason:
      'The provenance-bound functional and measured lifecycle rows plus H.264 1080p60 performance passed; 4K dropped-frame performance remains explicitly statused.',
  },
  machine: {
    model: binding.machine.model,
    chip: binding.machine.chip,
    macos: binding.machine.macOS,
  },
  native: {
    h264FirstFrame: matrix.rows['h264-videotoolbox'],
    hevcFirstFrame: matrix.rows['hevc-videotoolbox'],
    hevc4kFirstFrame: true,
    frameStepForward: matrix.rows['frame-step-forward'],
    frameStepBackward: matrix.rows['frame-step-backward'],
    timelinePreview: matrix.rows['timeline-preview'],
    closeToIdle: matrix.rows['30-mount-unmount-baseline'],
  },
  lifecycle,
  performance: [passedPerformance(h264Sample, binding.runId), fourKPerformance],
  thumbnailConcurrency: 1,
  offline: {
    status: 'passed',
    reason: null,
    networkPolicy: smoke.launch.networkPolicy,
    networkDisabled: smoke.networkDisabled,
    bundledRuntimeOnly: smoke.bundledRuntimeOnly,
    hostDependenciesAbsent: smoke.hostDependenciesAbsent,
  },
  sourceRuns: {
    base: binding.runId,
    rows: matrix.rowRunIds,
    h264Performance: binding.runId,
    hevc4kFunctional: binding.runId,
    hevc4kPerformance: matrix.rowRunIds['hevc-4k30-performance'],
  },
}

mkdirSync(path.dirname(outputPath), { recursive: true })
const temporary = `${outputPath}.tmp-${process.pid}`
writeFileSync(temporary, `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 })
renameSync(temporary, outputPath)
console.log(`PASS development evidence ${outputPath}`)

function validateBindings(matrixInput, diagnosticInput, smokeInput, currentSourceRoot) {
  for (const [label, value] of [
    ['matrix', matrixInput.binding],
    ['diagnostic', diagnosticInput.binding],
    ['smoke', smokeInput.binding],
  ]) {
    if (value === undefined || value?.schemaVersion !== 1) {
      throw new Error(`${label} evidence binding is missing or unsupported`)
    }
  }
  const expected = matrixInput.binding
  for (const field of [
    'runId',
    'sourceIdentity',
    'machine',
    'fixtures',
    'appRuntime',
    'bundleIdentity',
    'buildConfig',
    'buildAttestationSha256',
    'bundleAuditSha256',
    'networkLaunchSha256',
  ]) {
    if (!same(expected[field], diagnosticInput.binding[field])) {
      throw new Error(`${field} binding differs between matrix and diagnostic`)
    }
    if (!same(expected[field], smokeInput.binding[field])) {
      throw new Error(`${field} binding differs between matrix and smoke`)
    }
  }
  if (!same(matrixInput.machine, expected.machine)) {
    throw new Error('machine binding differs from the matrix machine inventory')
  }
  if (typeof expected.runId !== 'string' || expected.runId.length === 0) {
    throw new Error('run binding has no run ID')
  }
  if (!Array.isArray(expected.fixtures) || expected.fixtures.length < 2) {
    throw new Error('fixture binding is incomplete')
  }
  if (
    !isSha256(expected.appRuntime?.appExecutableSha256) ||
    !isSha256(expected.appRuntime?.runtimeInventorySha256) ||
    !isSha256(expected.appRuntime?.runtimeLockSha256)
  ) {
    throw new Error('app/runtime binding contains an invalid hash')
  }
  if (
    !isSha256(expected.buildAttestationSha256) ||
    !isSha256(expected.bundleAuditSha256) ||
    !isSha256(expected.networkLaunchSha256) ||
    expected.bundleIdentity?.identifier !== 'com.viewer.desktop' ||
    expected.bundleIdentity?.executableRelativePath !== 'Contents/MacOS/viewer-desktop'
  ) {
    throw new Error('build/audit attestation binding is incomplete')
  }
  const current = collectSourceIdentity(currentSourceRoot)
  if (!same(current, expected.sourceIdentity)) {
    throw new Error('current source identity does not match the native evidence binding')
  }
  return expected
}

function validateLifecycle(lifecycleInput) {
  if (lifecycleInput?.cycles !== 30) {
    throw new Error('lifecycle cycles must come from an exact 30-cycle runner result')
  }
  if (
    !Array.isArray(lifecycleInput.fixtureSequence) ||
    lifecycleInput.fixtureSequence.length !== lifecycleInput.cycles ||
    new Set(lifecycleInput.fixtureSequence).size < 2 ||
    lifecycleInput.fixtureSequence.some(
      (fixture, index, sequence) => index > 0 && fixture === sequence[index - 1],
    )
  ) {
    throw new Error('lifecycle navigation must alternate at least two fixtures')
  }
  const measuredResources = ['clients', 'renderContexts', 'surfaces']
  if (!same(lifecycleInput.measuredResources, measuredResources)) {
    throw new Error('lifecycle measured-resource claim exceeds the native diagnostics')
  }
  for (const resource of measuredResources) {
    const before = lifecycleInput.before?.[resource]
    const after = lifecycleInput.after?.[resource]
    if (!Number.isInteger(before) || before < 0 || after !== before) {
      throw new Error(`lifecycle baseline mismatch for ${resource}`)
    }
  }
  return structuredClone(lifecycleInput)
}

function validateSmoke(smokeInput, expectedBinding, matrixInput, diagnosticInput) {
  const sandboxProfile = '(version 1)(allow default)(deny network*)'
  if (smokeInput.schemaVersion !== 1 || smokeInput.status !== 'passed') {
    throw new Error('smoke evidence did not pass the strict development audit')
  }
  if (smokeInput.matrixSha256 !== sha256(matrixInput)) {
    throw new Error('smoke matrix hash does not match the collected matrix artifact')
  }
  if (smokeInput.diagnosticSha256 !== sha256(diagnosticInput)) {
    throw new Error('smoke diagnostic hash does not match the collected diagnostic artifact')
  }
  if (
    smokeInput.audit?.mode !== 'development' ||
    smokeInput.audit?.artifactSha256 !== expectedBinding.bundleAuditSha256 ||
    smokeInput.audit?.attestationSha256 !== expectedBinding.buildAttestationSha256 ||
    !same(smokeInput.audit?.appRuntime, expectedBinding.appRuntime) ||
    !same(smokeInput.audit?.bundleIdentity, expectedBinding.bundleIdentity) ||
    Object.values(smokeInput.audit?.checks ?? {}).length !== 5 ||
    !Object.values(smokeInput.audit?.checks ?? {}).every((value) => value === true) ||
    smokeInput.launch?.artifactSha256 !== expectedBinding.networkLaunchSha256 ||
    smokeInput.launch?.networkPolicy !== 'deny-all' ||
    smokeInput.launch?.executable !== '/usr/bin/sandbox-exec' ||
    !same(smokeInput.launch?.arguments?.slice(0, 2), ['-p', sandboxProfile]) ||
    smokeInput.launch?.sandboxProfileSha256 !== sha256(sandboxProfile) ||
    !Number.isInteger(smokeInput.launch?.launcherPid) ||
    smokeInput.launch.launcherPid <= 0 ||
    !Number.isInteger(smokeInput.launch?.targetPid) ||
    smokeInput.launch.targetPid <= 0 ||
    !Number.isInteger(smokeInput.launch?.windowIdentity?.windowId) ||
    smokeInput.launch.windowIdentity.windowId <= 0 ||
    smokeInput.networkDisabled !== true ||
    smokeInput.bundledRuntimeOnly !== true ||
    smokeInput.hostDependenciesAbsent !== true
  ) {
    throw new Error('smoke audit or deny-all launch is not bound to the evidence inputs')
  }
}

function validatePassedPerformance(sample, expected) {
  if (sample === undefined) throw new Error(`missing required performance sample: ${expected.id}`)
  for (const field of ['id', 'codec', 'width', 'height']) {
    if (sample[field] !== expected[field]) {
      throw new Error(`invalid ${expected.id} performance ${field}`)
    }
  }
  if (Math.abs(sample.framesPerSecond - expected.framesPerSecond) >= 0.01) {
    throw new Error(`invalid ${expected.id} performance framesPerSecond`)
  }
  if (![8, 10].includes(sample.bitDepth)) throw new Error(`invalid ${expected.id} bit depth`)
  if (sample.hwdec !== 'videotoolbox' || sample.videoOutput !== 'libmpv') {
    throw new Error(`invalid ${expected.id} native backend`)
  }
  if (!(sample.averageCommandLatencyMs < 100)) {
    throw new Error(`${expected.id} command latency did not pass`)
  }
  if (!(sample.postWarmupRenderedFrames > 0)) {
    throw new Error(`${expected.id} rendered-frame window is empty`)
  }
  const total = sample.postWarmupRenderedFrames + sample.postWarmupDroppedFrames
  if (!(total > 0) || (sample.postWarmupDroppedFrames * 100) / total >= 1) {
    throw new Error(`${expected.id} dropped-frame budget did not pass`)
  }
}

function passedPerformance(sample, sourceRunId) {
  return { ...sample, status: 'passed', reason: null, sourceRunId }
}

function same(left, right) {
  return JSON.stringify(left) === JSON.stringify(right)
}

function isSha256(value) {
  return typeof value === 'string' && /^[a-f0-9]{64}$/.test(value)
}
