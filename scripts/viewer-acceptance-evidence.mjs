import { createHash } from 'node:crypto'
import { readFile, writeFile } from 'node:fs/promises'
import { deflateSync, inflateSync } from 'node:zlib'

const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])

export class AcceptanceEvidenceError extends Error {
  constructor(code, message, details = {}) {
    super(message)
    this.name = 'AcceptanceEvidenceError'
    this.code = code
    this.details = details
  }
}

export function sha256Bytes(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}

export async function sha256File(filePath) {
  return sha256Bytes(await readFile(filePath))
}

export async function readRgbaPng(filePath) {
  const png = await readFile(filePath)
  if (!png.subarray(0, 8).equals(PNG_SIGNATURE)) {
    throw new AcceptanceEvidenceError('CAPTURE_PNG', 'Evidence is not a PNG image', {
      filePath,
    })
  }
  let offset = 8
  let header
  const compressed = []
  while (offset < png.length) {
    const length = png.readUInt32BE(offset)
    const type = png.toString('ascii', offset + 4, offset + 8)
    const data = png.subarray(offset + 8, offset + 8 + length)
    if (type === 'IHDR') header = data
    else if (type === 'IDAT') compressed.push(data)
    else if (type === 'IEND') break
    offset += 12 + length
  }
  if (!header || compressed.length === 0 || header.length !== 13) {
    throw new AcceptanceEvidenceError('CAPTURE_PNG', 'PNG evidence is incomplete', {
      filePath,
    })
  }
  const width = header.readUInt32BE(0)
  const height = header.readUInt32BE(4)
  const bitDepth = header[8]
  const colorType = header[9]
  if (
    bitDepth !== 8 ||
    ![2, 6].includes(colorType) ||
    header[10] !== 0 ||
    header[11] !== 0 ||
    header[12] !== 0
  ) {
    throw new AcceptanceEvidenceError(
      'CAPTURE_PNG',
      'PNG evidence must be non-interlaced 8-bit RGB or RGBA',
      { bitDepth, colorType },
    )
  }
  const bytesPerPixel = colorType === 6 ? 4 : 3
  const rowBytes = width * bytesPerPixel
  const filtered = inflateSync(Buffer.concat(compressed))
  if (filtered.length !== height * (rowBytes + 1)) {
    throw new AcceptanceEvidenceError('CAPTURE_PNG', 'PNG scanline size is invalid')
  }
  const decoded = Buffer.alloc(height * rowBytes)
  for (let y = 0; y < height; y += 1) {
    const filter = filtered[y * (rowBytes + 1)]
    const sourceOffset = y * (rowBytes + 1) + 1
    const targetOffset = y * rowBytes
    for (let x = 0; x < rowBytes; x += 1) {
      const encoded = filtered[sourceOffset + x]
      const left = x >= bytesPerPixel ? decoded[targetOffset + x - bytesPerPixel] : 0
      const above = y > 0 ? decoded[targetOffset + x - rowBytes] : 0
      const upperLeft =
        y > 0 && x >= bytesPerPixel
          ? decoded[targetOffset + x - rowBytes - bytesPerPixel]
          : 0
      let predictor
      if (filter === 0) predictor = 0
      else if (filter === 1) predictor = left
      else if (filter === 2) predictor = above
      else if (filter === 3) predictor = Math.floor((left + above) / 2)
      else if (filter === 4) predictor = paethPredictor(left, above, upperLeft)
      else {
        throw new AcceptanceEvidenceError('CAPTURE_PNG', 'PNG uses an unknown filter', {
          filter,
        })
      }
      decoded[targetOffset + x] = (encoded + predictor) & 0xff
    }
  }
  if (colorType === 6) return { width, height, data: decoded }
  const rgba = Buffer.alloc(width * height * 4)
  for (let source = 0, target = 0; source < decoded.length; source += 3, target += 4) {
    rgba[target] = decoded[source]
    rgba[target + 1] = decoded[source + 1]
    rgba[target + 2] = decoded[source + 2]
    rgba[target + 3] = 255
  }
  return { width, height, data: rgba }
}

export async function writeRgbaPng(filePath, { width, height, data }) {
  if (
    !Number.isInteger(width) ||
    width <= 0 ||
    !Number.isInteger(height) ||
    height <= 0 ||
    !Buffer.isBuffer(data) ||
    data.length !== width * height * 4
  ) {
    throw new AcceptanceEvidenceError('CAPTURE_PNG', 'Invalid RGBA evidence image')
  }
  const header = Buffer.alloc(13)
  header.writeUInt32BE(width, 0)
  header.writeUInt32BE(height, 4)
  header[8] = 8
  header[9] = 6
  const scanlines = Buffer.alloc(height * (width * 4 + 1))
  for (let y = 0; y < height; y += 1) {
    const scanlineOffset = y * (width * 4 + 1)
    scanlines[scanlineOffset] = 0
    data.copy(scanlines, scanlineOffset + 1, y * width * 4, (y + 1) * width * 4)
  }
  await writeFile(
    filePath,
    Buffer.concat([
      PNG_SIGNATURE,
      pngChunk('IHDR', header),
      pngChunk('IDAT', deflateSync(scanlines, { level: 9 })),
      pngChunk('IEND', Buffer.alloc(0)),
    ]),
  )
}

export async function combinePngEvidence({ reference, native, output }) {
  const [referenceImage, nativeImage] = await Promise.all([
    readRgbaPng(reference),
    readRgbaPng(native),
  ])
  if (referenceImage.height !== nativeImage.height) {
    throw new AcceptanceEvidenceError(
      'CAPTURE_COMPARISON_SIZE',
      'Reference and native evidence must have the same height',
      {
        reference: { width: referenceImage.width, height: referenceImage.height },
        native: { width: nativeImage.width, height: nativeImage.height },
      },
    )
  }
  const width = referenceImage.width + nativeImage.width
  const height = referenceImage.height
  const data = Buffer.alloc(width * height * 4)
  for (let y = 0; y < height; y += 1) {
    const targetOffset = y * width * 4
    referenceImage.data.copy(
      data,
      targetOffset,
      y * referenceImage.width * 4,
      (y + 1) * referenceImage.width * 4,
    )
    nativeImage.data.copy(
      data,
      targetOffset + referenceImage.width * 4,
      y * nativeImage.width * 4,
      (y + 1) * nativeImage.width * 4,
    )
  }
  await writeRgbaPng(output, { width, height, data })
  return { width, height }
}

function crc32(buffer) {
  let crc = 0xffffffff
  for (const byte of buffer) {
    crc ^= byte
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1))
    }
  }
  return (crc ^ 0xffffffff) >>> 0
}

function pngChunk(type, data) {
  const typeBytes = Buffer.from(type, 'ascii')
  const chunk = Buffer.alloc(12 + data.length)
  chunk.writeUInt32BE(data.length, 0)
  typeBytes.copy(chunk, 4)
  data.copy(chunk, 8)
  chunk.writeUInt32BE(crc32(Buffer.concat([typeBytes, data])), 8 + data.length)
  return chunk
}

function paethPredictor(left, above, upperLeft) {
  const prediction = left + above - upperLeft
  const leftDistance = Math.abs(prediction - left)
  const aboveDistance = Math.abs(prediction - above)
  const upperLeftDistance = Math.abs(prediction - upperLeft)
  if (leftDistance <= aboveDistance && leftDistance <= upperLeftDistance) return left
  if (aboveDistance <= upperLeftDistance) return above
  return upperLeft
}
