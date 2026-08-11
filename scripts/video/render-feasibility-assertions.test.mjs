import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

import {
  analyzeReactOverlay,
  matrixExitCode,
  parsePlaybackTimeUs,
  proveFrameDirection,
} from './render-feasibility-assertions.mjs'

test('parses typed playback time and proves forward then backward movement', () => {
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
