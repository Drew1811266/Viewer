import { spawn, spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  existsSync,
  closeSync,
  mkdirSync,
  openSync,
  readFileSync,
  writeFileSync,
} from 'node:fs'
import { once } from 'node:events'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  NativeAcceptanceClient,
  buildNativeHelper,
  discoverNativeWindows,
  parseProcessTable,
  readRgbaPng,
  selectExactViewer,
  waitFor,
  waitForStable,
} from '../viewer-native-acceptance.mjs'
import {
  analyzeReactOverlay,
  matrixExitCode,
  nativeLaunchSpec,
  parseNamedCounter,
  parsePlaybackTimeUs,
  parseRenderedFrames,
  proveFrameDirection,
  selectLaunchedViewerProcess,
  validatePerformanceRemount,
  verifyFixtureHashes,
} from './render-feasibility-assertions.mjs'
import {
  appendCleanupFailure,
  cleanupFeasibilityLaunch,
  renderFeasibilityTestPaths,
} from './render-feasibility-lifecycle.mjs'
import {
  createBuildAttestation,
  loadVerifiedAuditArtifact,
  loadVerifiedBuildAttestation,
  performanceResetPlan,
  prepareAcceptanceBundle,
  writeBuildAttestation,
} from './build-attestation.mjs'
import { collectSourceIdentity } from './source-identity.mjs'
import {
  loadVerifiedNetworkLaunchArtifact,
  writeNetworkLaunchArtifact,
} from './network-launch-artifact.mjs'

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDirectory, '../..')
const outputRoot = path.join(repoRoot, 'target', 'video-render-feasibility')
const matrixResultPath = path.join(outputRoot, 'matrix-result.json')
const generationDiagnosticPath = path.join(outputRoot, 'task14-generation-diagnostic.json')
const buildAttestationPath = path.join(outputRoot, 'build-attestation.json')
const bundleAuditPath = path.join(outputRoot, 'bundle-audit.json')
const networkLaunchPath = path.join(outputRoot, 'network-launch.json')
const logsRoot = path.join(outputRoot, 'logs')
const screenshotsRoot = path.join(outputRoot, 'screenshots')
const runtimeRoot = path.join(
  repoRoot,
  'target',
  'task6-review-resources',
  'ViewerVideoRuntime',
)
const appPath = process.env.VIEWER_VIDEO_ACCEPTANCE_APP
  ? path.resolve(process.env.VIEWER_VIDEO_ACCEPTANCE_APP)
  : path.join(repoRoot, 'target', 'debug', 'bundle', 'macos', 'Viewer.app')
const appExecutable = path.join(appPath, 'Contents', 'MacOS', 'viewer-desktop')
const bundledMpv = path.join(
  appPath,
  'Contents',
  'Resources',
  'ViewerVideoRuntime',
  'lib',
  'libmpv.2.dylib',
)
const bundleIdentity = Object.freeze({
  identifier: 'com.viewer.desktop',
  productName: 'Viewer',
  executableRelativePath: 'Contents/MacOS/viewer-desktop',
})
const only4k = process.env.VIEWER_VIDEO_ACCEPTANCE_ONLY_4K === '1'
const matrixRows = [
  'surface-at-dom-rect',
  'retina-resize',
  'react-overlay-z-order',
  'first-frame-ready',
  'frame-step-forward',
  'frame-step-backward',
  'h264-videotoolbox',
  'hevc-videotoolbox',
  'no-ipc-frame-buffer',
  '30-mount-unmount-baseline',
  'timeline-preview',
  'h264-1080p60-performance',
  'hevc-4k30-performance',
]
const fixtures = [
  ['h264-1080p', 'h264-1080p.mp4'],
  ['hevc-portrait', 'hevc-portrait.mp4'],
  ['vfr-step', 'vfr-step.mp4'],
]
const firstFrameExpectations = {
  'h264-1080p': { left: 152, right: 872, minColorRatio: 0.35 },
  'hevc-portrait': { left: 212, right: 812, minColorRatio: 0.2 },
  'vfr-step': { left: 212, right: 812, minColorRatio: 0.35 },
  'h264-1080p60': { left: 212, right: 812, minColorRatio: 0.25 },
  'hevc-4k30': { left: 212, right: 812, minColorRatio: 0.25 },
}
const result = {
  generatedAt: new Date().toISOString(),
  source: {},
  machine: {},
  fixtures: {},
  native: {},
  signing: {},
  counters: { before: null, after: null },
  lifecycle: null,
  generationSequence: [],
  binding: null,
  screenshots: {},
  rows: Object.fromEntries(matrixRows.map((row) => [row, false])),
  error: null,
  performance: [],
  rowRunIds: {},
}

if (only4k) {
  if (process.env.VIEWER_VIDEO_ACCEPTANCE_SKIP_BUILD !== '1') {
    throw new Error('only-4k resume requires VIEWER_VIDEO_ACCEPTANCE_SKIP_BUILD=1')
  }
  const previous = JSON.parse(readFileSync(matrixResultPath, 'utf8'))
  const inheritedRows = matrixRows.filter((row) => row !== 'hevc-4k30-performance')
  for (const row of inheritedRows) {
    if (previous.rows?.[row] !== true) {
      throw new Error(`only-4k resume cannot inherit a failed row: ${row}`)
    }
  }
  if (!previous.performance?.some((sample) => sample.id === 'h264-1080p60')) {
    throw new Error('only-4k resume requires passed h264-1080p60 evidence')
  }
  const resumeSourceRunId = previous.generatedAt
  Object.assign(result, {
    fixtures: structuredClone(previous.fixtures),
    native: structuredClone(previous.native),
    signing: structuredClone(previous.signing),
    counters: structuredClone(previous.counters),
    screenshots: structuredClone(previous.screenshots),
    rows: { ...previous.rows, 'hevc-4k30-performance': false },
    performance: previous.performance.filter((sample) => sample.id !== 'hevc-4k30'),
    resumeSourceRunId,
    rowRunIds: Object.fromEntries(
      matrixRows.map((row) => [row, previous.rowRunIds?.[row] ?? resumeSourceRunId]),
    ),
  })
  result.rowRunIds['hevc-4k30-performance'] = null
}

let activeViewer

function run(command, argumentsList, logName, { env } = {}) {
  const completed = spawnSync(command, argumentsList, {
    cwd: repoRoot,
    encoding: 'utf8',
    env: env ?? process.env,
    maxBuffer: 64 * 1024 * 1024,
  })
  const transcript = [
    `$ ${[command, ...argumentsList].join(' ')}`,
    completed.stdout ?? '',
    completed.stderr ?? '',
  ].join('\n')
  writeFileSync(path.join(logsRoot, logName), transcript)
  if (completed.error) throw completed.error
  if (completed.status !== 0) {
    throw new Error(`${command} failed with status ${completed.status}; see ${logName}`)
  }
  return completed.stdout.trim()
}

function sha256(filePath) {
  return createHash('sha256').update(readFileSync(filePath)).digest('hex')
}

function ensureEvidenceBinding(verifiedAudit) {
  const buildAttestation = verifiedAudit.attestation
  const binding = {
    schemaVersion: 1,
    runId: result.generatedAt,
    sourceIdentity: buildAttestation.attestation.sourceIdentity,
    machine: result.machine,
    fixtures: Object.entries(result.fixtures).map(([id, fixture]) => ({
      id,
      sha256: fixture.sha256,
    })),
    appRuntime: buildAttestation.attestation.appRuntime,
    bundleIdentity: buildAttestation.attestation.bundleIdentity,
    buildConfig: buildAttestation.attestation.buildConfig,
    buildAttestationSha256: buildAttestation.sha256,
    bundleAuditSha256: verifiedAudit.sha256,
  }
  if (result.binding !== null && JSON.stringify(result.binding) !== JSON.stringify(binding)) {
    throw new Error('app/runtime evidence binding changed during the native run')
  }
  result.binding = binding
}

function refreshBundledRuntimeInventory() {
  const runtime = path.join(appPath, 'Contents', 'Resources', 'ViewerVideoRuntime')
  const inventoryPath = path.join(runtime, 'runtime.inventory.sha256')
  const entries = readFileSync(inventoryPath, 'utf8')
    .trim()
    .split('\n')
    .map((line) => line.slice(66))
  const inventory = entries
    .map((relative) => `${sha256(path.join(runtime, relative))}  ${relative}`)
    .join('\n')
  writeFileSync(inventoryPath, `${inventory}\n`)
}

function tauriConfig(fixtureId) {
  return JSON.stringify({
    build: {
      beforeBuildCommand: '',
      frontendDist: '../target/viewer-visual-acceptance/site',
    },
    app: {
      windows: [
        {
          title: 'Viewer Video Feasibility',
          url: `visual-acceptance.html?id=VIDEO-FEASIBILITY&viewport=1024x720#${fixtureId}`,
          width: 1024,
          height: 720,
          minWidth: 500,
          resizable: true,
        },
      ],
    },
    bundle: { resources: { [runtimeRoot]: 'ViewerVideoRuntime' } },
  })
}

async function queryOne(client, target) {
  const response = await client.request('query', { target })
  return response.elements[0]
}

async function waitForElement(client, target, predicate = () => true, timeoutMs = 10_000) {
  return waitFor(
    async () => {
      try {
        const element = await queryOne(client, target)
        return predicate(element) ? element : false
      } catch (error) {
        if (error?.code === 'STATE_TARGET_NOT_FOUND') return false
        throw error
      }
    },
    { timeoutMs, intervalMs: 50 },
  )
}

async function waitForStatus(client, requiredFragments, timeoutMs = 10_000) {
  const elements = []
  for (const fragment of requiredFragments) {
    let remainingMs = timeoutMs
    while (true) {
      const windowMs = Math.min(remainingMs, 10_000)
      try {
        elements.push(
          await waitForElement(client, { role: 'AXGroup', name: fragment }, () => true, windowMs),
        )
        break
      } catch (error) {
        remainingMs -= windowMs
        if (error?.code !== 'PRECONDITION_WAIT_TIMEOUT' || remainingMs <= 0) throw error
      }
    }
  }
  return elements
}

async function waitForPlaybackTime(client, predicate = () => true) {
  return waitFor(
    async () => {
      try {
        const element = await queryOne(client, { role: 'AXGroup', namePrefix: '时间：' })
        const timeUs = parsePlaybackTimeUs(element.name)
        return predicate(timeUs) ? timeUs : false
      } catch (error) {
        if (error?.code === 'STATE_TARGET_NOT_FOUND' || /not numeric/.test(String(error))) {
          return false
        }
        throw error
      }
    },
    { timeoutMs: 10_000, intervalMs: 50 },
  )
}

async function currentPlaybackTime(client) {
  try {
    const element = await queryOne(client, { role: 'AXGroup', namePrefix: '时间：' })
    return parsePlaybackTimeUs(element.name)
  } catch (error) {
    if (error?.code === 'STATE_TARGET_NOT_FOUND' || /not numeric/.test(String(error))) return null
    throw error
  }
}

async function waitForRenderedFrames(client, predicate = () => true) {
  return waitFor(
    async () => {
      try {
        const element = await queryOne(client, { role: 'AXGroup', namePrefix: '已绘制：' })
        const frames = parseRenderedFrames(element.name)
        return predicate(frames) ? frames : false
      } catch (error) {
        if (error?.code === 'STATE_TARGET_NOT_FOUND' || /not numeric/.test(String(error))) {
          return false
        }
        throw error
      }
    },
    { timeoutMs: 10_000, intervalMs: 50 },
  )
}

async function currentNamedCounter(client, label) {
  const element = await queryOne(client, { role: 'AXGroup', namePrefix: `${label}：` })
  return parseNamedCounter(element.name, label)
}

async function waitForNamedCounter(client, label, predicate) {
  return waitFor(
    async () => {
      try {
        const value = await currentNamedCounter(client, label)
        return predicate(value) ? value : false
      } catch (error) {
        if (error?.code === 'STATE_TARGET_NOT_FOUND' || /not numeric/.test(String(error))) {
          return false
        }
        throw error
      }
    },
    { timeoutMs: 10_000, intervalMs: 20 },
  )
}

async function activate(client, name, timeoutMs = 5_000) {
  await client.request('activate', { target: { role: 'AXButton', name } }, { timeoutMs })
}

async function clickWithPointer(client, window, name) {
  const element = await waitForElement(client, { role: 'AXButton', name })
  const point = {
    x: Math.round(element.frame.x - window.x + element.frame.width / 2),
    y: Math.round(element.frame.y - window.y + element.frame.height / 2),
  }
  await client.request('pointer', { kind: 'click', point })
}

async function capture(client, name) {
  const capturePath = path.join(screenshotsRoot, `${name}.png`)
  await client.request('capture', { path: capturePath }, { timeoutMs: 10_000 })
  result.screenshots[name] = { path: capturePath, sha256: sha256(capturePath) }
  return capturePath
}

async function stopViewer(viewer) {
  if (!viewer) return
  try {
    await cleanupFeasibilityLaunch(viewer, {
      currentProcessTable,
      stopLauncher: stopViewerLauncher,
      stopProcess: stopViewerProcess,
    })
  } finally {
    if (activeViewer === viewer) activeViewer = undefined
  }
}

function processIsRunning(pid) {
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}

async function waitForProcessExit(pid, timeoutMs) {
  const deadline = Date.now() + timeoutMs
  while (processIsRunning(pid)) {
    if (Date.now() >= deadline) return false
    await new Promise((resolve) => setTimeout(resolve, 50))
  }
  return true
}

async function stopViewerProcess(pid) {
  if (!processIsRunning(pid)) return
  process.kill(pid, 'SIGTERM')
  if (await waitForProcessExit(pid, 3_000)) return
  if (processIsRunning(pid)) process.kill(pid, 'SIGKILL')
  if (!(await waitForProcessExit(pid, 3_000))) {
    throw new Error(`Viewer process ${pid} did not exit`)
  }
}

async function waitForChildExit(child, timeoutMs) {
  if (child.exitCode !== null || child.signalCode !== null) return true
  return Promise.race([
    once(child, 'exit').then(() => true),
    new Promise((resolve) => setTimeout(() => resolve(false), timeoutMs)),
  ])
}

async function stopViewerLauncher(launcher) {
  if (launcher.exitCode !== null || launcher.signalCode !== null) return
  if (await waitForChildExit(launcher, 3_000)) return
  launcher.kill('SIGTERM')
  if (await waitForChildExit(launcher, 3_000)) return
  if (launcher.exitCode === null && launcher.signalCode === null) launcher.kill('SIGKILL')
  if (!(await waitForChildExit(launcher, 3_000))) {
    throw new Error('Viewer LaunchServices waiter did not exit')
  }
}

function currentProcessTable() {
  const completed = spawnSync('ps', ['-axo', 'pid=,ppid=,pgid=,command='], {
    cwd: repoRoot,
    encoding: 'utf8',
    killSignal: 'SIGKILL',
    maxBuffer: 4 * 1024 * 1024,
    timeout: 1_000,
  })
  if (completed.error) throw completed.error
  if (completed.status !== 0) {
    throw new Error(`ps failed with status ${completed.status}`)
  }
  return parseProcessTable(completed.stdout)
}

async function waitForLaunchedViewer(launchState, fixtureId, existingPids) {
  return waitFor(
    async () => {
      const { launcher } = launchState
      if (launchState.error) throw launchState.error
      if (launcher.exitCode !== null || launcher.signalCode !== null) {
        throw new Error(`Viewer launcher exited before ${fixtureId} became ready`)
      }
      const processes = currentProcessTable()
      const launchedViewer = selectLaunchedViewerProcess(processes, appExecutable, existingPids)
      if (launchedViewer === null) return false
      launchState.pid = launchedViewer.pid
      return selectExactViewer(processes, {
        executablePath: appExecutable,
        controllerPid: process.pid,
      })
    },
    { timeoutMs: 10_000, intervalMs: 50 },
  )
}

async function buildAndLaunch(fixtureId, helperPath) {
  const skipBuild = process.env.VIEWER_VIDEO_ACCEPTANCE_SKIP_BUILD === '1'
  const buildConfig = {
    profile: 'debug',
    features: ['video-feasibility'],
    bundle: 'app',
    tauriConfig: JSON.parse(tauriConfig(fixtureId)),
  }
  const attestationOptions = {
    sourceRoot: repoRoot,
    appPath,
    attestationPath: buildAttestationPath,
    bundleIdentity,
    buildConfig,
  }
  const verifiedAudit = await prepareAcceptanceBundle({
    skipBuild,
    build: () =>
      run(
        'pnpm',
        [
          'tauri',
          'build',
          '--debug',
          '--features',
          'video-feasibility',
          '--bundles',
          'app',
          '--config',
          tauriConfig(fixtureId),
        ],
        `build-${fixtureId}.log`,
      ),
    signRuntime: () =>
      run(
        'codesign',
        ['--force', '--sign', '-', '--timestamp=none', bundledMpv],
        `sign-mpv-${fixtureId}.log`,
      ),
    refreshInventory: () => refreshBundledRuntimeInventory(),
    signApp: () =>
      run(
        'codesign',
        ['--force', '--deep', '--sign', '-', '--timestamp=none', appPath],
        `sign-app-${fixtureId}.log`,
      ),
    createOrVerifyAttestation: (isSkip) => {
      if (!isSkip) {
        writeBuildAttestation(buildAttestationPath, createBuildAttestation(attestationOptions))
      }
      return loadVerifiedBuildAttestation(attestationOptions)
    },
    audit: () =>
      run(
        '/bin/bash',
        [
          path.join(scriptDirectory, 'verify-app-bundle.sh'),
          '--mode',
          'development',
          '--attestation',
          buildAttestationPath,
          '--artifact',
          bundleAuditPath,
          appPath,
        ],
        `bundle-audit-${fixtureId}.log`,
      ),
    verifyAudit: () =>
      loadVerifiedAuditArtifact({ ...attestationOptions, auditPath: bundleAuditPath }),
  })
  ensureEvidenceBinding(verifiedAudit)
  result.signing[fixtureId] = {
    bundleAuditMode: verifiedAudit.artifact.mode,
    bundleAuditSha256: verifiedAudit.sha256,
    buildAttestationSha256: verifiedAudit.attestation.sha256,
    auditLog: path.join(logsRoot, `bundle-audit-${fixtureId}.log`),
  }

  const nativeLogPath = path.join(logsRoot, `native-${fixtureId}.log`)
  const existingPids = new Set(currentProcessTable().map((processInfo) => processInfo.pid))
  const networkDisabled = process.env.VIEWER_VIDEO_ACCEPTANCE_NETWORK_DISABLED === '1'
  const launch = nativeLaunchSpec({
    appExecutable,
    appPath,
    nativeLogPath,
    networkDisabled,
    env: process.env,
  })
  const nativeLogDescriptor = networkDisabled ? openSync(nativeLogPath, 'a') : undefined
  const launcher = spawn(launch.command, launch.argumentsList, {
    cwd: repoRoot,
    env: launch.env,
    stdio: networkDisabled
      ? ['ignore', nativeLogDescriptor, nativeLogDescriptor]
      : 'ignore',
  })
  if (nativeLogDescriptor !== undefined) closeSync(nativeLogDescriptor)
  activeViewer = {
    baselinePids: existingPids,
    client: undefined,
    error: undefined,
    executablePath: appExecutable,
    launcher,
    pid: undefined,
  }
  launcher.once('error', (error) => {
    if (activeViewer?.launcher === launcher) activeViewer.error = error
  })
  const viewer = await waitForLaunchedViewer(activeViewer, fixtureId, existingPids)

  let candidateWindowId
  const window = await waitForStable(
    async () => {
      if (launcher.exitCode !== null || launcher.signalCode !== null) {
        throw new Error(`Viewer exited before ${fixtureId} became ready`)
      }
      try {
        const windows = await discoverNativeWindows({ helperPath, pid: viewer.pid })
        const candidate =
          windows.find(
            (window) =>
              window.title === 'Viewer Video Feasibility' &&
              window.width === 1024 &&
              window.height === 720,
          ) ?? false
        if (!candidate) return false
        if (candidate.windowId !== candidateWindowId) {
          candidateWindowId = candidate.windowId
          return false
        }
        return candidate
      } catch {
        return false
      }
    },
    { timeoutMs: 10_000, intervalMs: 100, stableMs: 300 },
  )
  if (launch.networkPolicy === 'deny-all') {
    writeNetworkLaunchArtifact({
      outputPath: networkLaunchPath,
      appPath,
      binding: result.binding,
      launch,
      launcherPid: launcher.pid,
      targetPid: viewer.pid,
      windowIdentity: {
        windowId: window.windowId,
        title: window.title,
        width: window.width,
        height: window.height,
      },
    })
    const verifiedLaunch = loadVerifiedNetworkLaunchArtifact({
      artifactPath: networkLaunchPath,
      appPath,
      binding: result.binding,
    })
    result.binding.networkLaunchSha256 = verifiedLaunch.sha256
  }
  const client = new NativeAcceptanceClient({
    executablePath: helperPath,
    pid: viewer.pid,
    window,
    defaultTimeoutMs: 10_000,
  })
  activeViewer.client = client
  activeViewer.window = window
  await client.start()
  await client.request('focus', { target: { role: 'AXWindow' } })
  await waitForStatus(client, ['首帧：就绪'], 20_000)
  const firstRevealedCapture = await capture(client, `${fixtureId}-first-revealed`)
  const firstRevealedPixels = await assertColorBars(
    firstRevealedCapture,
    firstFrameExpectations[fixtureId].left,
    firstFrameExpectations[fixtureId].right,
    firstFrameExpectations[fixtureId].minColorRatio,
  )
  const [decodedPictureType] = await waitForStatus(client, ['解码帧：I'])
  const hwdec = await waitForElement(client, { name: 'videotoolbox' })
  const videoOutput = await waitForElement(client, { name: 'libmpv' })
  const resources = await waitForElement(client, { name: '1/1/1' })
  const renderedFrames = await waitForRenderedFrames(client, (frames) => frames >= 1)
  const playbackTimeUs = await currentPlaybackTime(client)

  result.native[fixtureId] = {
    pid: viewer.pid,
    hwdec: hwdec.name,
    videoOutput: videoOutput.name,
    resources: resources.name,
    renderedFrames,
    playbackTimeUs,
    decodedPictureType: decodedPictureType.name.replace('解码帧：', ''),
    firstRevealedFrame: {
      screenshot: firstRevealedCapture,
      pixels: firstRevealedPixels,
      capturedBeforeBackendPolling: true,
    },
    log: nativeLogPath,
  }
  await recordGeneration(client, fixtureId)
  return activeViewer
}

async function switchNativeFixture(viewer, fixtureId) {
  await activate(viewer.client, `切换样本 ${fixtureId}`)
  await waitForStatus(viewer.client, [`样本：${fixtureId}`, '首帧：就绪'])
  const firstRevealedCapture = await capture(viewer.client, `${fixtureId}-first-revealed`)
  const firstRevealedPixels = await assertColorBars(
    firstRevealedCapture,
    firstFrameExpectations[fixtureId].left,
    firstFrameExpectations[fixtureId].right,
    firstFrameExpectations[fixtureId].minColorRatio,
  )
  const [decodedPictureType] = await waitForStatus(viewer.client, ['解码帧：I'])
  const hwdec = await waitForElement(viewer.client, { name: 'videotoolbox' })
  const videoOutput = await waitForElement(viewer.client, { name: 'libmpv' })
  const resources = await waitForElement(viewer.client, { name: '1/1/1' })
  const renderedFrames = await waitForRenderedFrames(viewer.client, (frames) => frames >= 1)
  const playbackTimeUs = await currentPlaybackTime(viewer.client)
  result.native[fixtureId] = {
    pid: viewer.pid,
    hwdec: hwdec.name,
    videoOutput: videoOutput.name,
    resources: resources.name,
    renderedFrames,
    playbackTimeUs,
    decodedPictureType: decodedPictureType.name.replace('解码帧：', ''),
    firstRevealedFrame: {
      screenshot: firstRevealedCapture,
      pixels: firstRevealedPixels,
      capturedBeforeBackendPolling: true,
    },
    log: result.native['h264-1080p'].log,
  }
  await recordGeneration(viewer.client, fixtureId)
}

async function remountNativeFixture(viewer, fixtureId) {
  const currentGeneration = await readGenerationDiagnostics(viewer.client)
  const reset = performanceResetPlan(fixtureId)
  await activate(viewer.client, reset.action)
  const mountedGeneration = await waitForGenerationDiagnostics(
    viewer.client,
    (next) => next.generation > currentGeneration.generation,
  )
  await waitForStatus(viewer.client, [`样本：${reset.fixtureId}`, '首帧：就绪', '解码帧：I'])
  const hwdec = await waitForElement(viewer.client, { name: 'videotoolbox' })
  const videoOutput = await waitForElement(viewer.client, { name: 'libmpv' })
  const resources = await waitForElement(viewer.client, { name: '1/1/1' })
  const renderedFrames = await waitForRenderedFrames(viewer.client, (frames) => frames >= 1)
  const confirmedGeneration = await readGenerationDiagnostics(viewer.client)
  const measured = validatePerformanceRemount(
    currentGeneration.generation,
    mountedGeneration.generation,
    {
      generationDiagnostics: confirmedGeneration,
      firstFrameReady: true,
      decodedPictureType: 'I',
      resources: resources.name,
      renderedFrames,
      hwdec: hwdec.name,
      videoOutput: videoOutput.name,
    },
  )
  await recordGeneration(viewer.client, fixtureId)
  return measured
}

async function readGenerationDiagnostics(client) {
  const element = await queryOne(client, { role: 'AXGroup', namePrefix: '诊断代：' })
  const match = /^诊断代：(\d+) 挂载：(\d+) 回调：(\d+) 绘制入口：(\d+) FRAME：(\d+) 图像：(\d+) 显示：(\d+) 事件：(\d+)$/.exec(
    element.name,
  )
  if (!match) throw new Error(`generation diagnostics are not numeric: ${element.name}`)
  return {
    generation: Number.parseInt(match[1], 10),
    mountReturned: Number.parseInt(match[2], 10),
    updateCallbacks: Number.parseInt(match[3], 10),
    drawEntries: Number.parseInt(match[4], 10),
    frameUpdates: Number.parseInt(match[5], 10),
    pictureFrames: Number.parseInt(match[6], 10),
    reveals: Number.parseInt(match[7], 10),
    eventEmits: Number.parseInt(match[8], 10),
  }
}

async function waitForGenerationDiagnostics(client, predicate) {
  return waitFor(
    async () => {
      try {
        const diagnostics = await readGenerationDiagnostics(client)
        return predicate(diagnostics) ? diagnostics : false
      } catch (error) {
        if (error?.code === 'STATE_TARGET_NOT_FOUND' || /not numeric/.test(String(error))) {
          return false
        }
        throw error
      }
    },
    { timeoutMs: 10_000, intervalMs: 50 },
  )
}

async function recordGeneration(client, fixtureId) {
  const diagnostics = await readGenerationDiagnostics(client)
  const fixture = result.binding?.fixtures.find((entry) => entry.id === fixtureId)
  if (fixture === undefined) throw new Error(`generation fixture is not evidence-bound: ${fixtureId}`)
  const visits = result.generationSequence.filter((entry) => entry.fixture.id === fixtureId).length
  result.generationSequence.push({
    stage:
      fixtureId === 'hevc-4k30'
        ? visits === 0
          ? 'hevc-4k30-first-passive'
          : 'hevc-4k30-reopen-passive'
        : `${fixtureId}-passive-${visits + 1}`,
    fixture,
    ready: true,
    generation: diagnostics,
  })
}

function parseResourceCounts(name, label) {
  const match = new RegExp(`^${label}：(\\d+)\\/(\\d+)\\/(\\d+)$`).exec(name)
  if (!match) throw new Error(`${label} resource counts are not numeric: ${name}`)
  return {
    clients: Number.parseInt(match[1], 10),
    renderContexts: Number.parseInt(match[2], 10),
    surfaces: Number.parseInt(match[3], 10),
  }
}

async function samplePerformance(viewer, fixtureId, sample) {
  await switchNativeFixture(viewer, fixtureId)
  const latencies = []
  let serial = await currentNamedCounter(viewer.client, '命令')
  for (let index = 0; index < 10; index += 1) {
    for (const action of ['播放性能样本', '暂停性能样本']) {
      await activate(viewer.client, action)
      serial = await waitForNamedCounter(viewer.client, '命令', (value) => value > serial)
      latencies.push((await currentNamedCounter(viewer.client, '引擎延迟')) / 1_000)
    }
  }
  const averageCommandLatencyMs = latencies.reduce((sum, value) => sum + value, 0) / latencies.length

  // Accessibility-driven command sampling can consume most of a short sample
  // even though the native engine acknowledgements are fast. Remount the same
  // performance fixture before establishing the playback warmup baseline.
  const measuredSession = await remountNativeFixture(viewer, fixtureId)
  const resetTimeUs = await currentPlaybackTime(viewer.client)
  if (resetTimeUs !== null && resetTimeUs > 100_000) {
    throw new Error(`${fixtureId} did not reset before performance playback: ${resetTimeUs}us`)
  }
  serial = await currentNamedCounter(viewer.client, '命令')
  if (serial !== 0) {
    throw new Error(`${fixtureId} command serial did not reset: ${serial}`)
  }
  const resetRendered = await waitForRenderedFrames(viewer.client)
  await activate(viewer.client, '播放性能样本')
  serial = await waitForNamedCounter(viewer.client, '命令', (value) => value > serial)
  await new Promise((resolve) => setTimeout(resolve, 1_000))
  const beforeRendered = await waitForRenderedFrames(
    viewer.client,
    (frames) => frames > resetRendered,
  )
  const beforeMistimed = await currentNamedCounter(viewer.client, '误时帧')
  const beforeDecoderDropped = await currentNamedCounter(viewer.client, '解码丢帧')
  await new Promise((resolve) => setTimeout(resolve, 3_000))
  await activate(viewer.client, '暂停性能样本')
  await waitForNamedCounter(viewer.client, '命令', (value) => value > serial)
  const afterRendered = await waitForRenderedFrames(viewer.client)
  const afterMistimed = await currentNamedCounter(viewer.client, '误时帧')
  const afterDecoderDropped = await currentNamedCounter(viewer.client, '解码丢帧')
  const postWarmupRenderedFrames = afterRendered - beforeRendered
  const postWarmupDroppedFrames =
    afterMistimed - beforeMistimed + (afterDecoderDropped - beforeDecoderDropped)
  const droppedPercent =
    (postWarmupDroppedFrames * 100) / (postWarmupRenderedFrames + postWarmupDroppedFrames)
  if (!(averageCommandLatencyMs < 100)) {
    throw new Error(`${fixtureId} average command latency ${averageCommandLatencyMs}ms exceeded 100ms`)
  }
  if (!(postWarmupRenderedFrames > 0) || !(droppedPercent < 1)) {
    throw new Error(`${fixtureId} dropped ${droppedPercent}% after warmup`)
  }
  const evidence = {
    id: fixtureId,
    ...sample,
    hwdec: measuredSession.hwdec,
    videoOutput: measuredSession.videoOutput,
    averageCommandLatencyMs,
    postWarmupRenderedFrames,
    postWarmupDroppedFrames,
  }
  result.performance.push(evidence)
  result.rows[`${fixtureId}-performance`] = true
  result.rowRunIds[`${fixtureId}-performance`] = result.generatedAt
}

async function assertColorBars(filePath, expectedLeft, expectedRight, minColorRatio = 0.35) {
  const image = await readRgbaPng(filePath)
  const scaleX = image.width / 1024
  const scaleY = image.height / 720
  let colorful = 0
  let sampled = 0
  let minX = image.width
  let maxX = -1
  const startY = Math.round(170 * scaleY)
  const endY = Math.round(300 * scaleY)
  for (let y = startY; y < endY; y += 1) {
    for (let x = Math.round(100 * scaleX); x < Math.round(924 * scaleX); x += 1) {
      const offset = (y * image.width + x) * 4
      const red = image.data[offset]
      const green = image.data[offset + 1]
      const blue = image.data[offset + 2]
      const bright = Math.max(red, green, blue)
      const saturated = bright - Math.min(red, green, blue) >= 90 && bright >= 170
      sampled += 1
      if (saturated) {
        colorful += 1
        minX = Math.min(minX, x)
        maxX = Math.max(maxX, x)
      }
    }
  }
  const ratio = colorful / sampled
  const logicalMin = minX / scaleX
  const logicalMax = (maxX + 1) / scaleX
  if (
    ratio < minColorRatio ||
    Math.abs(logicalMin - expectedLeft) > 8 ||
    Math.abs(logicalMax - expectedRight) > 8
  ) {
    throw new Error(
      `Native color frame geometry mismatch in ${filePath}: ratio=${ratio.toFixed(3)} extent=${logicalMin.toFixed(1)}..${logicalMax.toFixed(1)}`,
    )
  }
  return { ratio, left: logicalMin, right: logicalMax }
}

function assertNoIpcFrameBuffer() {
  const files = [
    path.join(repoRoot, 'src-tauri', 'src', 'video_feasibility.rs'),
    path.join(repoRoot, 'ui', 'src', 'acceptance', 'scenes', 'videoFeasibilityScene.tsx'),
  ]
  const forbidden = /frame[_-]?buffer|ArrayBuffer|Uint8Array|ImageData|data:image/i
  for (const filePath of files) {
    if (forbidden.test(readFileSync(filePath, 'utf8'))) {
      throw new Error(`IPC frame-buffer token found in ${filePath}`)
    }
  }
}

function preflight() {
  if (process.platform !== 'darwin' || process.arch !== 'arm64') {
    throw new Error('Native render feasibility requires arm64 macOS')
  }
  if (!existsSync(runtimeRoot)) throw new Error(`Bundled runtime is missing: ${runtimeRoot}`)
  const running = spawnSync('pgrep', ['-x', 'viewer-desktop'], { encoding: 'utf8' })
  if (running.status === 0 && running.stdout.trim().length > 0) {
    throw new Error(`Close existing Viewer processes before running the matrix: ${running.stdout.trim()}`)
  }
}

async function main() {
  mkdirSync(logsRoot, { recursive: true })
  mkdirSync(screenshotsRoot, { recursive: true })
  for (const [fixtureId, fileName] of fixtures) {
    const fixturePath = path.join(repoRoot, 'tests', 'fixtures', 'videos', fileName)
    result.fixtures[fixtureId] = { path: fixturePath, sha256: sha256(fixturePath) }
  }
  verifyFixtureHashes(result.fixtures)
  for (const [fixtureId, fileName] of [
    ['h264-1080p60', 'h264-1080p60.mp4'],
    ['hevc-4k30', 'hevc-4k30.mov'],
  ]) {
    const fixturePath = path.join(repoRoot, 'target', 'video-performance', fileName)
    result.fixtures[fixtureId] = { path: fixturePath, sha256: sha256(fixturePath) }
  }
  preflight()

  const workingTreeStatus = run('git', ['status', '--short'], 'source-status.log')
  result.source = {
    commit: run('git', ['rev-parse', 'HEAD'], 'source-commit.log'),
    tree: run('git', ['rev-parse', 'HEAD^{tree}'], 'source-tree.log'),
    clean: workingTreeStatus.length === 0,
    workingTreeStatus,
    identity: collectSourceIdentity(repoRoot),
  }
  result.machine = {
    model: run('sysctl', ['-n', 'hw.model'], 'machine-model.log'),
    chip: run('sysctl', ['-n', 'machdep.cpu.brand_string'], 'machine-chip.log'),
    macOS: run('sw_vers', [], 'machine-macos.log'),
    uname: run('uname', ['-a'], 'machine-uname.log'),
  }
  run(
    'node',
    ['--test', ...renderFeasibilityTestPaths],
    'test-render-feasibility-assertions.log',
  )
  run(
    'cargo',
    ['test', '-p', 'viewer-platform-macos', '--test', 'video_surface_geometry'],
    'test-video-surface-geometry.log',
  )
  run(
    'cargo',
    ['test', '-p', 'viewer-video-mpv', '--test', 'client_contract'],
    'test-mpv-client-contract.log',
  )
  run(
    'cargo',
    ['test', '-p', 'viewer-desktop', '--features', 'video-feasibility', '--lib'],
    'test-video-feasibility-route.log',
  )
  run(
    'cargo',
    [
      'test',
      '-p',
      'viewer-desktop',
      '--test',
      'video_resource_lifecycle',
      'bundled_timeline_preview_produces_a_real_png',
      '--',
      '--ignored',
      '--nocapture',
    ],
    'test-bundled-timeline-preview.log',
    { env: { ...process.env, VIEWER_VIDEO_RUNTIME_DIR: path.join(runtimeRoot, '..') } },
  )
  result.rows['timeline-preview'] = true
  run(
    'pnpm',
    ['--dir', 'ui', 'exec', 'vitest', 'run', 'src/acceptance/scenes/videoFeasibilityScene.test.tsx'],
    'test-video-feasibility-ui.log',
  )
  assertNoIpcFrameBuffer()
  result.rows['no-ipc-frame-buffer'] = true

  run('pnpm', ['build:visual-acceptance'], 'build-visual-acceptance.log')
  const helper = await buildNativeHelper({
    repoRoot,
    sourcePath: path.join(repoRoot, 'scripts', 'viewer-native-acceptance.swift'),
  })

  if (only4k) {
    const viewer = await buildAndLaunch('h264-1080p', helper.executablePath)
    await activate(viewer.client, '调整原生表面尺寸')
    await waitForElement(viewer.client, { name: 'videotoolbox' })
    await samplePerformance(viewer, 'hevc-4k30', {
      codec: 'hevc',
      width: 3840,
      height: 2160,
      framesPerSecond: 30,
      bitDepth: 8,
    })
    await stopViewer(viewer)
    return
  }

  let viewer = await buildAndLaunch('h264-1080p', helper.executablePath)
  const overlay = await waitForElement(viewer.client, { name: 'React 视频控制覆盖层' })
  const h264Capture = result.native['h264-1080p'].firstRevealedFrame.screenshot
  const initialGeometry = result.native['h264-1080p'].firstRevealedFrame.pixels
  const overlayPixels = analyzeReactOverlay(await readRgbaPng(h264Capture))
  await activate(viewer.client, '调整原生表面尺寸')
  await waitForElement(viewer.client, { name: 'videotoolbox' })
  const retinaCapture = await capture(viewer.client, 'h264-retina-resize')
  const resizedGeometry = await assertColorBars(retinaCapture, 212, 812)
  result.native['h264-1080p'].geometry = { initial: initialGeometry, resized: resizedGeometry }
  result.native['h264-1080p'].overlay = { accessibilityName: overlay.name, pixels: overlayPixels }
  result.rows['surface-at-dom-rect'] = true
  result.rows['retina-resize'] = true
  result.rows['react-overlay-z-order'] = true
  result.rows['first-frame-ready'] = true
  result.rows['h264-videotoolbox'] = true
  await switchNativeFixture(viewer, 'hevc-portrait')
  result.rows['hevc-videotoolbox'] = true
  await switchNativeFixture(viewer, 'vfr-step')
  const reportedInitialTimeUs = result.native['vfr-step'].playbackTimeUs
  const initialTimeUs = reportedInitialTimeUs ?? 0
  const initialRenderedFrames = result.native['vfr-step'].renderedFrames
  await activate(viewer.client, '前进一帧')
  await waitForStatus(viewer.client, ['前进：完成'])
  const firstForwardTimeUs = await waitForPlaybackTime(
    viewer.client,
    (timeUs) => timeUs > initialTimeUs,
  )
  const forwardFrames = await waitForRenderedFrames(
    viewer.client,
    (frames) => frames > initialRenderedFrames,
  )
  result.native['vfr-step'].frameStepForwardFrames = forwardFrames
  result.rows['frame-step-forward'] = true
  // Move away from the first-frame boundary before stepping backward. At the
  // boundary mpv intentionally makes time-pos unavailable, which cannot prove
  // whether the command moved backward or merely returned an EOF state.
  await activate(viewer.client, '前进一帧')
  const secondForwardTimeUs = await waitForPlaybackTime(
    viewer.client,
    (timeUs) => timeUs > firstForwardTimeUs,
  )
  const secondForwardFrames = await waitForRenderedFrames(
    viewer.client,
    (frames) => frames > forwardFrames,
  )
  await activate(viewer.client, '后退一帧')
  await waitForStatus(viewer.client, ['前进：完成', '后退：完成'])
  const backwardTimeUs = await waitForPlaybackTime(
    viewer.client,
    (timeUs) => timeUs < secondForwardTimeUs,
  )
  const backwardFrames = await waitForRenderedFrames(
    viewer.client,
    (frames) => frames > secondForwardFrames,
  )
  result.native['vfr-step'].secondFrameStepForwardFrames = secondForwardFrames
  result.native['vfr-step'].frameStepBackwardFrames = backwardFrames
  result.native['vfr-step'].playbackDirection = proveFrameDirection(
    firstForwardTimeUs,
    secondForwardTimeUs,
    backwardTimeUs,
  )
  result.native['vfr-step'].playbackDirection.initialSource =
    reportedInitialTimeUs === null ? 'first-forward-time-pos' : 'mpv-time-pos-after-first-forward'
  result.rows['frame-step-backward'] = true
  await capture(viewer.client, 'vfr-forward-backward')
  await clickWithPointer(viewer.client, viewer.window, '循环挂载与卸载 30 次')
  // Do not traverse the accessibility tree while AppKit is intentionally
  // adding and removing OpenGL views. The isolated diagnostic run completes
  // all 30 cycles in about two seconds; wait three, then enforce the exact
  // terminal UI/counter state within the normal ten-second safety bound.
  await new Promise((resolve) => setTimeout(resolve, 3_000))
  const [cycles] = await waitForStatus(viewer.client, ['生命周期：30/30'], 10_000)
  const baseline = await queryOne(viewer.client, {
    role: 'AXGroup',
    namePrefix: '生命周期基线：',
  })
  const after = await queryOne(viewer.client, {
    role: 'AXGroup',
    namePrefix: '生命周期结束：',
  })
  const sequence = await queryOne(viewer.client, {
    role: 'AXGroup',
    namePrefix: '生命周期路径：',
  })
  const cycleMatch = /^生命周期：(\d+)\/30$/.exec(cycles.name)
  if (!cycleMatch) throw new Error(`lifecycle cycle count is not numeric: ${cycles.name}`)
  const beforeCounts = parseResourceCounts(baseline.name, '生命周期基线')
  const afterCounts = parseResourceCounts(after.name, '生命周期结束')
  result.lifecycle = {
    cycles: Number.parseInt(cycleMatch[1], 10),
    before: beforeCounts,
    after: afterCounts,
    fixtureSequence: sequence.name.replace(/^生命周期路径：/, '').split(',').filter(Boolean),
    measuredResources: ['clients', 'renderContexts', 'surfaces'],
  }
  result.counters.before = Object.values(beforeCounts).join('/')
  result.counters.after = Object.values(afterCounts).join('/')
  await capture(viewer.client, 'lifecycle-30-baseline')
  result.rows['30-mount-unmount-baseline'] = true
  await samplePerformance(viewer, 'h264-1080p60', {
    codec: 'h264',
    width: 1920,
    height: 1080,
    framesPerSecond: 60,
    bitDepth: 8,
  })
  await samplePerformance(viewer, 'hevc-4k30', {
    codec: 'hevc',
    width: 3840,
    height: 2160,
    framesPerSecond: 30,
    bitDepth: 8,
  })
  await stopViewer(viewer)
}

try {
  await main()
} catch (error) {
  result.error = error?.stack ?? String(error)
  process.exitCode = 1
} finally {
  try {
    await stopViewer(activeViewer)
  } catch (error) {
    result.error = appendCleanupFailure(result.error, error)
    process.exitCode = 1
  }
  mkdirSync(outputRoot, { recursive: true })
  if (!only4k && result.binding !== null) {
    for (const row of matrixRows) {
      if (result.rows[row] === true && result.rowRunIds[row] === undefined) {
        result.rowRunIds[row] = result.binding.runId
      }
    }
  }
  writeFileSync(matrixResultPath, `${JSON.stringify(result, null, 2)}\n`)
  writeFileSync(
    generationDiagnosticPath,
    `${JSON.stringify(
      {
        binding: result.binding,
        generatedAt: result.generatedAt,
        sequence: result.generationSequence,
        error: result.error,
      },
      null,
      2,
    )}\n`,
  )
  for (const row of matrixRows) console.log(`${result.rows[row] ? 'PASS' : 'FAIL'} ${row}`)
  if (result.error) console.error(result.error)
  if (matrixExitCode(result.rows) !== 0) process.exitCode = 1
}
