import assert from 'node:assert/strict'
import test from 'node:test'

import {
  analyzeReactOverlay,
  matrixExitCode,
  parseRenderedFrames,
  parseNamedCounter,
  parsePlaybackTimeUs,
  proveFrameDirection,
  selectLaunchedViewerProcess,
  validatePerformanceRemount,
  verifyFixtureHashes,
} from './render-feasibility-assertions.mjs'

const requiredFixtures = {
  'h264-1080p': {
    sha256: '972aff59c7183940dbdfae2d906421a26604a10b6bd24432ccf969a028674296',
  },
  'hevc-portrait': {
    sha256: '4e7abaf98918f862ea07c5b529a143cd7f208c3bba7b807dab7995ac8103d0ac',
  },
  'vfr-step': {
    sha256: 'ee9ede284fabb52570fdd51428bf5408fad35de9ef8a4aeaefbcbc577cfcdde9',
  },
}

test('requires the exact three approved fixture hashes before native work', () => {
  assert.equal(verifyFixtureHashes(requiredFixtures), true)
  assert.throws(
    () =>
      verifyFixtureHashes({
        ...requiredFixtures,
        'h264-1080p': { sha256: '0'.repeat(64) },
      }),
    /Fixture hash mismatch/,
  )
  assert.throws(
    () => verifyFixtureHashes({ ...requiredFixtures, extra: { sha256: '0'.repeat(64) } }),
    /exact fixture allowlist/,
  )
  const { 'vfr-step': _missing, ...missingFixture } = requiredFixtures
  assert.throws(() => verifyFixtureHashes(missingFixture), /exact fixture allowlist/)

})

test('parses typed playback time and proves forward then backward movement', () => {
  assert.equal(parseRenderedFrames('已绘制：4'), 4)
  assert.equal(parseNamedCounter('误时帧：3', '误时帧'), 3)
  assert.equal(parseNamedCounter('命令：12', '命令'), 12)
  assert.throws(() => parseNamedCounter('命令：—', '命令'), /not numeric/)
  assert.throws(() => parseRenderedFrames('4 帧'), /not numeric/)
  assert.equal(parsePlaybackTimeUs('时间：33333 µs'), 33_333)
  assert.deepEqual(proveFrameDirection(0, 33_333, 16_667), {
    initialUs: 0,
    forwardUs: 33_333,
    backwardUs: 16_667,
  })
  assert.throws(() => proveFrameDirection(0, 0, 0), /did not move forward/)
  assert.throws(() => proveFrameDirection(0, 33_333, 33_333), /did not move backward/)
})

test('performance remount evidence is generation-coherent and backend-bound', () => {
  const observed = {
    generationDiagnostics: {
      generation: 8,
      mountReturned: true,
      updateCallbacks: 2,
      drawEntries: 3,
      frameUpdates: 1,
      pictureFrames: 1,
      reveals: 1,
      eventEmits: 2,
    },
    firstFrameReady: true,
    decodedPictureType: 'I',
    resources: '1/1/1',
    renderedFrames: 4,
    hwdec: 'videotoolbox',
    videoOutput: 'libmpv',
  }
  assert.equal(validatePerformanceRemount(7, 8, observed), observed)
  assert.throws(
    () =>
      validatePerformanceRemount(7, 8, {
        ...observed,
        generationDiagnostics: { ...observed.generationDiagnostics, generation: 7 },
      }),
    /expected native generation/,
  )
  assert.throws(
    () => validatePerformanceRemount(7, 8, { ...observed, hwdec: 'no' }),
    /required native backend/,
  )
})

test('requires every matrix row before returning a successful exit code', () => {
  assert.equal(matrixExitCode({ first: true, second: true }), 0)
  assert.equal(matrixExitCode({ first: true, second: false }), 1)
})

test('attributes the new exact app process before stricter Viewer validation', () => {
  const executablePath = '/worktree/target/debug/bundle/macos/Viewer.app/Contents/MacOS/viewer-desktop'
  const processes = [
    { pid: 41, command: '/other-worktree/target/debug/viewer-desktop' },
    { pid: 52, command: `${executablePath} --acceptance` },
  ]

  assert.deepEqual(selectLaunchedViewerProcess(processes, executablePath, new Set([41])), {
    pid: 52,
    command: `${executablePath} --acceptance`,
  })
  assert.equal(selectLaunchedViewerProcess(processes.slice(0, 1), executablePath, new Set([41])), null)
  assert.throws(
    () =>
      selectLaunchedViewerProcess(
        [...processes, { pid: 53, command: executablePath }],
        executablePath,
        new Set([41]),
      ),
    /exactly one new exact Viewer process/,
  )
})

test('recognizes a neutral React control overlay above saturated video pixels', () => {
  const image = rgbaImage(1024, 720, [255, 0, 0, 255])
  fill(image, 404, 488, 620, 540, [20, 22, 26, 255])
  for (const [left, right] of [
    [414, 448],
    [454, 488],
    [494, 554],
    [560, 610],
  ]) {
    fill(image, left, 498, right, 530, [238, 238, 238, 255])
  }

  const evidence = analyzeReactOverlay(image)
  assert.ok(evidence.neutralRatio > 0.7)
  assert.ok(evidence.brightNeutralRatio > 0.2)

  const noOverlay = rgbaImage(1024, 720, [255, 0, 0, 255])
  assert.throws(() => analyzeReactOverlay(noOverlay), /React overlay pixels/)
})

function rgbaImage(width, height, color) {
  const data = new Uint8Array(width * height * 4)
  const image = { width, height, data }
  fill(image, 0, 0, width, height, color)
  return image
}

function fill(image, left, top, right, bottom, color) {
  for (let y = top; y < bottom; y += 1) {
    for (let x = left; x < right; x += 1) {
      image.data.set(color, (y * image.width + x) * 4)
    }
  }
}
