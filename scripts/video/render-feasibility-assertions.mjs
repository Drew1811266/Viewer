export function parsePlaybackTimeUs(name) {
  const match = /^时间：(\d+) µs$/.exec(name)
  if (!match) throw new Error(`Playback time status is not numeric: ${name}`)
  return Number.parseInt(match[1], 10)
}

export function parseRenderedFrames(name) {
  const match = /^已绘制：(\d+)$/.exec(name)
  if (!match) throw new Error(`Rendered-frame status is not numeric: ${name}`)
  return Number.parseInt(match[1], 10)
}

export function proveFrameDirection(initialUs, forwardUs, backwardUs) {
  if (!(forwardUs > initialUs)) {
    throw new Error(`Playback time did not move forward: ${initialUs} -> ${forwardUs}`)
  }
  if (!(backwardUs < forwardUs)) {
    throw new Error(`Playback time did not move backward: ${forwardUs} -> ${backwardUs}`)
  }
  return { initialUs, forwardUs, backwardUs }
}

export function matrixExitCode(rows) {
  return Object.values(rows).every(Boolean) ? 0 : 1
}

const REQUIRED_FIXTURE_HASHES = Object.freeze({
  'h264-1080p': '972aff59c7183940dbdfae2d906421a26604a10b6bd24432ccf969a028674296',
  'hevc-portrait': '4e7abaf98918f862ea07c5b529a143cd7f208c3bba7b807dab7995ac8103d0ac',
  'vfr-step': 'ee9ede284fabb52570fdd51428bf5408fad35de9ef8a4aeaefbcbc577cfcdde9',
})

export function verifyFixtureHashes(fixtures) {
  const requiredIds = Object.keys(REQUIRED_FIXTURE_HASHES).sort()
  const actualIds = Object.keys(fixtures).sort()
  if (actualIds.length !== requiredIds.length || actualIds.some((id, index) => id !== requiredIds[index])) {
    throw new Error(`Native matrix requires the exact fixture allowlist: ${requiredIds.join(', ')}`)
  }
  for (const id of requiredIds) {
    const actual = fixtures[id]?.sha256
    const expected = REQUIRED_FIXTURE_HASHES[id]
    if (actual !== expected) {
      throw new Error(`Fixture hash mismatch for ${id}: expected ${expected}, got ${actual}`)
    }
  }
  return true
}

export function analyzeReactOverlay(image) {
  const scaleX = image.width / 1024
  const scaleY = image.height / 720
  const left = Math.round(404 * scaleX)
  const right = Math.round(620 * scaleX)
  const top = Math.round(488 * scaleY)
  const bottom = Math.round(540 * scaleY)
  let neutral = 0
  let brightNeutral = 0
  let sampled = 0

  for (let y = top; y < bottom; y += 1) {
    for (let x = left; x < right; x += 1) {
      const offset = (y * image.width + x) * 4
      const red = image.data[offset]
      const green = image.data[offset + 1]
      const blue = image.data[offset + 2]
      const high = Math.max(red, green, blue)
      const low = Math.min(red, green, blue)
      const isNeutral = high - low <= 35
      sampled += 1
      if (isNeutral) neutral += 1
      if (isNeutral && low >= 180) brightNeutral += 1
    }
  }

  const evidence = {
    region: { left: 404, right: 620, top: 488, bottom: 540 },
    neutralRatio: neutral / sampled,
    brightNeutralRatio: brightNeutral / sampled,
  }
  if (evidence.neutralRatio < 0.5 || evidence.brightNeutralRatio < 0.18) {
    throw new Error(
      `React overlay pixels are not visible above video: neutral=${evidence.neutralRatio.toFixed(3)} bright=${evidence.brightNeutralRatio.toFixed(3)}`,
    )
  }
  return evidence
}
