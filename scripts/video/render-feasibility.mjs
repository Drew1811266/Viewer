import { spawn, spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  existsSync,
  mkdirSync,
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
  parsePlaybackTimeUs,
  parseRenderedFrames,
  proveFrameDirection,
  selectLaunchedViewerProcess,
  verifyFixtureHashes,
} from './render-feasibility-assertions.mjs'
import {
  appendCleanupFailure,
  cleanupFeasibilityLaunch,
  renderFeasibilityTestPaths,
} from './render-feasibility-lifecycle.mjs'

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(scriptDirectory, '../..')
const outputRoot = path.join(repoRoot, 'target', 'video-render-feasibility')
const logsRoot = path.join(outputRoot, 'logs')
const screenshotsRoot = path.join(outputRoot, 'screenshots')
const runtimeRoot = path.join(
  repoRoot,
  'target',
  'viewer-video-runtime',
  'aarch64-apple-darwin',
  'ViewerVideoRuntime',
)
const appPath = path.join(repoRoot, 'target', 'debug', 'bundle', 'macos', 'Viewer.app')
const appExecutable = path.join(appPath, 'Contents', 'MacOS', 'viewer-desktop')
const bundledMpv = path.join(
  appPath,
  'Contents',
  'Resources',
  'ViewerVideoRuntime',
  'lib',
  'libmpv.2.dylib',
)
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
]
const fixtures = [
  ['h264-1080p', 'h264-1080p.mp4'],
  ['hevc-portrait', 'hevc-portrait.mp4'],
  ['vfr-step', 'vfr-step.mp4'],
]
const firstFrameExpectations = {
  'h264-1080p': { left: 152, right: 872, minColorRatio: 0.35 },
  'hevc-portrait': { left: 152, right: 872, minColorRatio: 0.25 },
  'vfr-step': { left: 152, right: 872, minColorRatio: 0.35 },
}
const result = {
  generatedAt: new Date().toISOString(),
  source: {},
  machine: {},
  fixtures: {},
  native: {},
  signing: {},
  counters: { before: '0/0/0', after: null },
  screenshots: {},
  rows: Object.fromEntries(matrixRows.map((row) => [row, false])),
  error: null,
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
    elements.push(
      await waitForElement(client, { role: 'AXGroup', name: fragment }, () => true, timeoutMs),
    )
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
    maxBuffer: 4 * 1024 * 1024,
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
  )
  run('codesign', ['--force', '--sign', '-', '--timestamp=none', bundledMpv], `sign-mpv-${fixtureId}.log`)
  run(
    'codesign',
    ['--force', '--deep', '--sign', '-', '--timestamp=none', appPath],
    `sign-app-${fixtureId}.log`,
  )
  run(
    'codesign',
    ['--verify', '--strict', '--verbose=4', bundledMpv],
    `verify-mpv-${fixtureId}.log`,
  )
  run(
    'codesign',
    ['--verify', '--deep', '--strict', '--verbose=4', appPath],
    `verify-app-${fixtureId}.log`,
  )
  result.signing[fixtureId] = {
    nestedRuntimeVerified: true,
    appBundleDeepStrictVerified: true,
    nestedLog: path.join(logsRoot, `verify-mpv-${fixtureId}.log`),
    appLog: path.join(logsRoot, `verify-app-${fixtureId}.log`),
  }

  const nativeLogPath = path.join(logsRoot, `native-${fixtureId}.log`)
  const existingPids = new Set(currentProcessTable().map((processInfo) => processInfo.pid))
  const launcher = spawn(
    '/usr/bin/open',
    ['-n', '-W', '-o', nativeLogPath, '--stderr', nativeLogPath, appPath],
    { cwd: repoRoot, stdio: 'ignore' },
  )
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
  await waitForStatus(client, ['首帧：就绪'])
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
  return activeViewer
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
  preflight()

  const workingTreeStatus = run('git', ['status', '--short'], 'source-status.log')
  result.source = {
    commit: run('git', ['rev-parse', 'HEAD'], 'source-commit.log'),
    tree: run('git', ['rev-parse', 'HEAD^{tree}'], 'source-tree.log'),
    clean: workingTreeStatus.length === 0,
    workingTreeStatus,
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
  await stopViewer(viewer)

  viewer = await buildAndLaunch('hevc-portrait', helper.executablePath)
  result.rows['hevc-videotoolbox'] = true
  await stopViewer(viewer)

  viewer = await buildAndLaunch('vfr-step', helper.executablePath)
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
  await waitForStatus(viewer.client, ['生命周期：30/30'], 10_000)
  const released = await waitForElement(viewer.client, { name: '0/0/0' })
  result.counters.after = released.name
  await capture(viewer.client, 'lifecycle-30-baseline')
  result.rows['30-mount-unmount-baseline'] = true
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
  writeFileSync(path.join(outputRoot, 'matrix-result.json'), `${JSON.stringify(result, null, 2)}\n`)
  for (const row of matrixRows) console.log(`${result.rows[row] ? 'PASS' : 'FAIL'} ${row}`)
  if (result.error) console.error(result.error)
  if (matrixExitCode(result.rows) !== 0) process.exitCode = 1
}
