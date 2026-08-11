export function parsePlaybackTimeUs(name) {
  const match = /^时间：(\d+) µs$/.exec(name)
  if (!match) throw new Error(`Playback time status is not numeric: ${name}`)
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
