import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  analyzeReactOverlay,
  matrixExitCode,
  parseRenderedFrames,
  parsePlaybackTimeUs,
  proveFrameDirection,
  selectLaunchedViewerProcess,
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

  const runner = readFileSync(new URL('./render-feasibility.mjs', import.meta.url), 'utf8')
  const verify = runner.indexOf('verifyFixtureHashes(result.fixtures)')
  const focusedTests = runner.indexOf("'test-render-feasibility-assertions.log'")
  assert.ok(verify >= 0 && focusedTests > verify)
})

test('parses typed playback time and proves forward then backward movement', () => {
  assert.equal(parseRenderedFrames('已绘制：4'), 4)
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

test('the signed gate verifies both the nested runtime and final app bundle', () => {
  const runner = readFileSync(new URL('./render-feasibility.mjs', import.meta.url), 'utf8')
  assert.match(runner, /verify-mpv-/)
  assert.match(runner, /verify-app-/)
  assert.match(runner, /'--verify', '--deep', '--strict'/)
})

test('window discovery must remain stable before the native helper binds its id', () => {
  const runner = readFileSync(new URL('./render-feasibility.mjs', import.meta.url), 'utf8')
  assert.match(runner, /waitForStable\(/)
  assert.match(runner, /stableMs: 300/)
})

test('the first ready surface is captured and visually checked before backend polling', () => {
  const runner = readFileSync(new URL('./render-feasibility.mjs', import.meta.url), 'utf8')
  const ready = runner.indexOf("await waitForStatus(client, ['首帧：就绪'])")
  const capture = runner.indexOf("capture(client, `${fixtureId}-first-revealed`)", ready)
  const pixels = runner.indexOf('const firstRevealedPixels = await assertColorBars', capture)
  const decodedSignal = runner.indexOf("waitForStatus(client, ['解码帧：I'])", pixels)
  const backend = runner.indexOf("name: 'videotoolbox'", ready)

  assert.ok(
    ready >= 0 &&
      capture > ready &&
      pixels > capture &&
      decodedSignal > pixels &&
      backend > decodedSignal,
  )
  assert.match(
    runner,
    /waitForRenderedFrames\(\s*viewer\.client,\s*\(frames\) => frames > initialRenderedFrames/s,
  )
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
