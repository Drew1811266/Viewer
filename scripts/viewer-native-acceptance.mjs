import path from 'node:path'
import { execFile, spawn } from 'node:child_process'
import { createHash, randomUUID } from 'node:crypto'
import { readFileSync } from 'node:fs'
import {
  access,
  cp,
  lstat,
  mkdir,
  readFile,
  readdir,
  realpath,
  rename,
  rm,
  writeFile,
} from 'node:fs/promises'
import readline from 'node:readline'
import { homedir } from 'node:os'
import { fileURLToPath } from 'node:url'
import { promisify } from 'node:util'
import { deflateSync, inflateSync } from 'node:zlib'

const execFileAsync = promisify(execFile)

export const PROTOCOL_VERSION = 1
export const ALLOWED_COMMANDS = new Set([
  'inspect',
  'query',
  'activate',
  'focus',
  'setValue',
  'key',
  'pointer',
  'drag',
  'capture',
  'shutdown',
])

const ALLOWED_KEYS = new Set([
  'tab',
  'enter',
  'space',
  'escape',
  'arrowUp',
  'arrowDown',
  'arrowLeft',
  'arrowRight',
  'home',
  'period',
  'end',
  'delete',
  'backspace',
  'a',
  'b',
  'c',
  'd',
  'e',
  'f',
  'g',
  'h',
  'i',
  'j',
  'k',
  'l',
  'm',
  'n',
  'o',
  'p',
  'q',
  'r',
  's',
  't',
  'u',
  'v',
  'w',
  'x',
  'y',
  'z',
])
const ALLOWED_MODIFIERS = new Set(['shift', 'control', 'option', 'command'])

function numberedIds(prefix, count) {
  return Array.from(
    { length: count },
    (_, index) => `${prefix}-${String(index + 1).padStart(2, '0')}`,
  )
}

export const AUDIT_IDS = Object.freeze([
  ...numberedIds('LAU', 9),
  ...numberedIds('SID', 4),
  ...numberedIds('STR', 5),
  ...numberedIds('THU', 7),
  ...numberedIds('OTH', 3),
  ...numberedIds('SEA', 5),
  ...numberedIds('FIL', 4),
  ...numberedIds('MEN', 3),
  ...numberedIds('RAD', 7),
  ...numberedIds('PRE', 7),
  ...numberedIds('COM', 4),
  ...numberedIds('DOC', 7),
  ...numberedIds('INF', 2),
  ...numberedIds('DIA', 7),
  ...numberedIds('TAS', 5),
  ...numberedIds('RES', 5),
  ...numberedIds('A11Y', 5),
])

const FIXTURE_VARIANT_BY_PREFIX = Object.freeze({
  LAU: 'entry-and-loading',
  SID: 'sidebar-and-drag',
  STR: 'project-structure',
  THU: 'thumbnail-selection',
  OTH: 'other-files',
  SEA: 'search-results',
  FIL: 'filter-controls',
  MEN: 'workspace-menus',
  RAD: 'radial-menu',
  PRE: 'image-preview',
  COM: 'image-comparison',
  DOC: 'document-preview',
  INF: 'information-inspector',
  DIA: 'dialogs',
  TAS: 'task-surface',
  RES: 'results-and-recovery',
  A11Y: 'accessibility',
})

function parseLedgerRecipeRows() {
  const ledgerPath = new URL(
    '../docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md',
    import.meta.url,
  )
  const markdown = readFileSync(ledgerPath, 'utf8')
  const rows = new Map()

  for (const line of markdown.split('\n')) {
    if (!/^\| [A-Z0-9]+-\d{2} \|/.test(line)) continue
    const cells = line
      .slice(1, -1)
      .split(' | ')
      .map((cell) => cell.trim())
    if (cells.length !== 9) {
      throw new Error(`Malformed native acceptance ledger row: ${line}`)
    }
    const [id, waveCell, referenceCell, , instruction] = cells
    const waveMatch = waveCell.match(/^Wave ([1-4])$/)
    const referenceMatch = referenceCell.match(/^`([^`]+)`$/)
    if (!waveMatch || !referenceMatch || instruction.length === 0) {
      throw new Error(`Incomplete native acceptance ledger recipe: ${id}`)
    }
    rows.set(id, {
      wave: Number(waveMatch[1]),
      referenceState: referenceMatch[1],
      instruction,
    })
  }

  return rows
}

function createStateRecipe(id, ledgerRecipe) {
  const prefix = id.split('-', 1)[0]
  const fixtureVariant = FIXTURE_VARIANT_BY_PREFIX[prefix]
  if (!fixtureVariant || !ledgerRecipe) {
    throw new Error(`Missing native acceptance recipe metadata for ${id}`)
  }
  const steps = Object.freeze([
    Object.freeze({ kind: 'resetFixture', variant: fixtureVariant }),
    Object.freeze({
      kind: 'nativeUserSequence',
      instruction: ledgerRecipe.instruction,
    }),
    Object.freeze({
      kind: 'waitForVisibleState',
      referenceState: ledgerRecipe.referenceState,
      timeoutMs: 10_000,
    }),
  ])
  return Object.freeze({
    id,
    wave: ledgerRecipe.wave,
    fixtureVariant,
    referenceState: ledgerRecipe.referenceState,
    steps,
    visibleAssertion: `Viewer visibly presents the approved ${ledgerRecipe.referenceState} state after the documented native user sequence.`,
  })
}

const LEDGER_RECIPES = parseLedgerRecipeRows()
export const STATE_RECIPES = new Map(
  AUDIT_IDS.map((id) => [id, createStateRecipe(id, LEDGER_RECIPES.get(id))]),
)

export class AcceptanceError extends Error {
  constructor(code, message, details = {}) {
    super(message)
    this.name = 'AcceptanceError'
    this.code = code
    this.details = details
  }
}

const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])

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

export async function readRgbaPng(filePath) {
  const png = await readFile(filePath)
  if (!png.subarray(0, 8).equals(PNG_SIGNATURE)) {
    throw new AcceptanceError('CAPTURE_PNG', 'Evidence is not a PNG image', {
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
    throw new AcceptanceError('CAPTURE_PNG', 'PNG evidence is incomplete', {
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
    throw new AcceptanceError(
      'CAPTURE_PNG',
      'PNG evidence must be non-interlaced 8-bit RGB or RGBA',
      { bitDepth, colorType },
    )
  }
  const bytesPerPixel = colorType === 6 ? 4 : 3
  const rowBytes = width * bytesPerPixel
  const filtered = inflateSync(Buffer.concat(compressed))
  if (filtered.length !== height * (rowBytes + 1)) {
    throw new AcceptanceError('CAPTURE_PNG', 'PNG scanline size is invalid')
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
        throw new AcceptanceError('CAPTURE_PNG', 'PNG uses an unknown filter', {
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
    throw new AcceptanceError('CAPTURE_PNG', 'Invalid RGBA evidence image')
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
    throw new AcceptanceError(
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

const RUN_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/
const VARIANT_PATTERN = /^[a-z][a-z0-9-]{0,63}$/

async function scanFixtureTree(root, relative = '') {
  const directory = path.join(root, relative)
  const entries = await readdir(directory, { withFileTypes: true })
  const files = []
  for (const entry of entries.sort((left, right) =>
    left.name.localeCompare(right.name, 'en'),
  )) {
    const entryRelative = path.join(relative, entry.name)
    const entryPath = path.join(root, entryRelative)
    const metadata = await lstat(entryPath)
    if (metadata.isSymbolicLink()) {
      throw new AcceptanceError(
        'FIXTURE_SYMLINK',
        'Fixture baselines cannot contain symbolic links',
        { path: entryPath },
      )
    }
    if (metadata.isDirectory()) {
      files.push(...(await scanFixtureTree(root, entryRelative)))
      continue
    }
    if (!metadata.isFile()) {
      throw new AcceptanceError(
        'FIXTURE_FILE_TYPE',
        'Fixture baselines may contain only directories and regular files',
        { path: entryPath },
      )
    }
    const contents = await readFile(entryPath)
    files.push({
      path: entryRelative.split(path.sep).join('/'),
      size: metadata.size,
      sha256: createHash('sha256').update(contents).digest('hex'),
    })
  }
  return files
}

export async function createFixtureRun({ repoRoot, runId }) {
  if (!RUN_ID_PATTERN.test(runId)) {
    throw new AcceptanceError('SAFETY_FIXTURE_PATH', 'Invalid fixture run ID', {
      runId,
    })
  }
  const fixtureRoot = path.join(
    repoRoot,
    'target',
    'atlas-product-migration-fixture',
  )
  const sourceRoot = path.join(fixtureRoot, 'ViewerAcceptance')
  const sourceMetadata = await lstat(sourceRoot)
  if (!sourceMetadata.isDirectory() || sourceMetadata.isSymbolicLink()) {
    throw new AcceptanceError(
      'FIXTURE_BASELINE',
      'ViewerAcceptance fixture baseline must be a real directory',
      { sourceRoot },
    )
  }
  const sourceFiles = await scanFixtureTree(sourceRoot)
  const runsRoot = path.join(homedir(), 'ViewerAcceptanceRuns')
  const runRoot = path.join(runsRoot, runId)
  const baselineRoot = path.join(runRoot, 'baseline')
  const variantsRoot = runRoot
  const manifestPath = path.join(runRoot, 'fixture-manifest.json')
  await mkdir(runsRoot, { recursive: true })
  await assertNoSymlinkBetween(
    homedir(),
    runsRoot,
    'SAFETY_FIXTURE_PATH',
  )
  try {
    await mkdir(runRoot)
  } catch (error) {
    if (error?.code === 'EEXIST') {
      throw new AcceptanceError(
        'FIXTURE_RUN_EXISTS',
        'Fixture run ID already exists',
        { runId, runRoot },
      )
    }
    throw error
  }

  try {
    await cp(sourceRoot, baselineRoot, {
      recursive: true,
      dereference: false,
      errorOnExist: true,
      force: false,
    })
    const copiedFiles = await scanFixtureTree(baselineRoot)
    if (JSON.stringify(copiedFiles) !== JSON.stringify(sourceFiles)) {
      throw new AcceptanceError(
        'FIXTURE_COPY_MISMATCH',
        'Private fixture snapshot does not match the source baseline',
      )
    }
    const manifest = {
      schemaVersion: 1,
      runId,
      sourceRoot,
      baselineRoot,
      files: copiedFiles,
    }
    await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, {
      flag: 'wx',
    })
    return Object.freeze({
      repoRoot,
      runId,
      sourceRoot,
      runRoot,
      baselineRoot,
      variantsRoot,
      manifestPath,
      manifest: Object.freeze(manifest),
    })
  } catch (error) {
    await rm(runRoot, { recursive: true, force: true })
    throw error
  }
}

export async function resetFixtureVariant(run, variant) {
  if (!run || !RUN_ID_PATTERN.test(run.runId) || !VARIANT_PATTERN.test(variant)) {
    throw new AcceptanceError(
      'SAFETY_FIXTURE_PATH',
      'Invalid fixture run or variant',
      { runId: run?.runId, variant },
    )
  }
  const expectedRunRoot = path.join(
    homedir(),
    'ViewerAcceptanceRuns',
    run.runId,
  )
  const expectedBaselineRoot = path.join(expectedRunRoot, 'baseline')
  const expectedVariantsRoot = expectedRunRoot
  if (
    path.normalize(run.runRoot) !== path.normalize(expectedRunRoot) ||
    path.normalize(run.baselineRoot) !== path.normalize(expectedBaselineRoot) ||
    path.normalize(run.variantsRoot) !== path.normalize(expectedVariantsRoot)
  ) {
    throw new AcceptanceError(
      'SAFETY_FIXTURE_PATH',
      'Fixture run root does not match its repository and run ID',
      {
        actual: {
          runRoot: run.runRoot,
          baselineRoot: run.baselineRoot,
          variantsRoot: run.variantsRoot,
        },
        expected: {
          runRoot: expectedRunRoot,
          baselineRoot: expectedBaselineRoot,
          variantsRoot: expectedVariantsRoot,
        },
      },
    )
  }
  await scanFixtureTree(run.baselineRoot)
  const target = path.join(run.variantsRoot, variant)
  const temporary = path.join(
    run.variantsRoot,
    `.${variant}.new-${process.pid}-${randomUUID()}`,
  )
  const previous = path.join(
    run.variantsRoot,
    `.${variant}.old-${process.pid}-${randomUUID()}`,
  )
  await Promise.all([
    validateFixturePath(temporary, run),
    validateFixturePath(previous, run),
    validateFixturePath(target, run),
  ])
  try {
    await cp(run.baselineRoot, temporary, {
      recursive: true,
      dereference: false,
      errorOnExist: true,
      force: false,
    })
    let hadPrevious = false
    try {
      await rename(target, previous)
      hadPrevious = true
    } catch (error) {
      if (error?.code !== 'ENOENT') throw error
    }
    try {
      await rename(temporary, target)
    } catch (error) {
      if (hadPrevious) await rename(previous, target)
      throw error
    }
    if (hadPrevious) await rm(previous, { recursive: true })
    return target
  } catch (error) {
    await rm(temporary, { recursive: true, force: true })
    throw error
  }
}

export async function removeFixtureRun(run) {
  if (!run || !RUN_ID_PATTERN.test(run.runId)) {
    throw new AcceptanceError(
      'SAFETY_FIXTURE_PATH',
      'Invalid fixture run cleanup request',
      { runId: run?.runId },
    )
  }
  const expectedRunRoot = path.join(
    homedir(),
    'ViewerAcceptanceRuns',
    run.runId,
  )
  const expectedManifestPath = path.join(expectedRunRoot, 'fixture-manifest.json')
  if (
    path.normalize(run.runRoot) !== path.normalize(expectedRunRoot) ||
    path.normalize(run.variantsRoot) !== path.normalize(expectedRunRoot) ||
    path.normalize(run.manifestPath) !== path.normalize(expectedManifestPath)
  ) {
    throw new AcceptanceError(
      'SAFETY_FIXTURE_PATH',
      'Fixture cleanup target does not match its home-scoped run ID',
      { runId: run.runId, runRoot: run.runRoot },
    )
  }
  await validateFixturePath(expectedRunRoot, { runId: run.runId })
  await rm(expectedRunRoot, { recursive: true })
}

function deepFreeze(value) {
  if (!value || typeof value !== 'object' || Object.isFrozen(value)) return value
  for (const child of Object.values(value)) deepFreeze(child)
  return Object.freeze(value)
}

function validEvidenceArtifact(artifact) {
  return (
    artifact &&
    typeof artifact.path === 'string' &&
    path.isAbsolute(artifact.path) &&
    /^[a-f0-9]{64}$/.test(artifact.sha256)
  )
}

export function buildEvidenceManifest(context) {
  const recipe = STATE_RECIPES.get(context?.id)
  const viewport = context?.viewport
  const window = context?.window
  const fixture = context?.fixture
  const assertion = context?.assertion
  const evidence = context?.evidence
  const valid =
    recipe &&
    context.wave === recipe.wave &&
    /^[a-f0-9]{40}$/.test(context.commit ?? '') &&
    typeof context.branch === 'string' &&
    context.branch.length > 0 &&
    typeof context.dirty === 'boolean' &&
    Number.isInteger(context.pid) &&
    context.pid > 0 &&
    typeof context.executablePath === 'string' &&
    path.isAbsolute(context.executablePath) &&
    ['1024x720', '1440x900'].includes(viewport) &&
    window &&
    Number.isInteger(window.windowId) &&
    window.windowId > 0 &&
    Number.isFinite(window.x) &&
    Number.isFinite(window.y) &&
    `${window.width}x${window.height}` === viewport &&
    fixture &&
    RUN_ID_PATTERN.test(fixture.runId) &&
    VARIANT_PATTERN.test(fixture.variant) &&
    fixture.variant === recipe.fixtureVariant &&
    typeof fixture.path === 'string' &&
    path.isAbsolute(fixture.path) &&
    Array.isArray(context.actions) &&
    context.actions.length > 0 &&
    context.actions.every((action) => action && typeof action === 'object') &&
    assertion &&
    typeof assertion.description === 'string' &&
    assertion.description.length > 0 &&
    typeof assertion.passed === 'boolean' &&
    evidence &&
    validEvidenceArtifact(evidence.raw) &&
    validEvidenceArtifact(evidence.reference) &&
    validEvidenceArtifact(evidence.combined) &&
    !Number.isNaN(Date.parse(context.timestamp)) &&
    ['pending-visual-review', 'pass', 'fail'].includes(context.verdict)

  if (!valid) {
    throw new AcceptanceError(
      'EVIDENCE_MANIFEST_INVALID',
      'Evidence manifest is incomplete or inconsistent with its recipe',
      { id: context?.id },
    )
  }

  return deepFreeze({
    schemaVersion: 1,
    id: context.id,
    wave: context.wave,
    commit: context.commit,
    branch: context.branch,
    dirty: context.dirty,
    process: {
      pid: context.pid,
      executablePath: context.executablePath,
    },
    window: { ...window },
    viewport,
    fixture: { ...fixture },
    actions: context.actions.map((action) => ({ ...action })),
    assertion: { ...assertion },
    evidence: {
      raw: { ...evidence.raw },
      reference: { ...evidence.reference },
      combined: { ...evidence.combined },
    },
    timestamp: context.timestamp,
    verdict: context.verdict,
  })
}

function cliError(message, details = {}) {
  return new AcceptanceError('CLI_ARGUMENT', message, details)
}

export function parseNativeAcceptanceCli(argv, { repoRoot }) {
  const argumentsList = argv[0] === '--' ? argv.slice(1) : argv
  const selectors = []
  let viewport = null
  let outputRoot = path.join(
    repoRoot,
    'target',
    'atlas-product-migration-acceptance',
  )

  for (let index = 0; index < argumentsList.length; index += 1) {
    const argument = argumentsList[index]
    if (argument === '--list') selectors.push({ mode: 'list', value: null })
    else if (argument === '--preflight') {
      selectors.push({ mode: 'preflight', value: null })
    } else if (argument === '--all') selectors.push({ mode: 'all', value: null })
    else if (argument === '--id') {
      const id = argumentsList[++index]
      if (!STATE_RECIPES.has(id)) throw cliError('Unknown acceptance ID', { id })
      selectors.push({ mode: 'id', value: id })
    } else if (argument === '--wave') {
      const wave = Number(argumentsList[++index])
      if (![1, 2, 3, 4].includes(wave)) {
        throw cliError('Wave must be one of 1, 2, 3 or 4', { wave })
      }
      selectors.push({ mode: 'wave', value: wave })
    } else if (argument === '--viewport') {
      viewport = argumentsList[++index]
      if (!['1024x720', '1440x900'].includes(viewport)) {
        throw cliError('Viewport must be 1024x720 or 1440x900', { viewport })
      }
    } else if (argument === '--output-root') {
      outputRoot = argumentsList[++index]
      if (!outputRoot) throw cliError('Missing output root')
    } else {
      throw cliError('Unknown native acceptance argument', { argument })
    }
  }

  if (selectors.length !== 1) {
    throw cliError('Choose exactly one acceptance selector')
  }
  const [{ mode, value: selector }] = selectors
  if (mode !== 'list' && viewport === null) {
    throw cliError('Viewport is required for preflight and capture')
  }
  const approvedOutputRoot = path.join(
    repoRoot,
    'target',
    'atlas-product-migration-acceptance',
  )
  const normalizedOutput = validateLiteralAbsolutePath(outputRoot, 'CLI_ARGUMENT')
  if (!isContainedPath(approvedOutputRoot, normalizedOutput)) {
    throw cliError('Output root escapes the acceptance evidence directory', {
      outputRoot: normalizedOutput,
    })
  }

  return {
    mode,
    selector,
    viewport,
    outputRoot: normalizedOutput,
    destructive: ['id', 'wave', 'all'].includes(mode),
  }
}

export function validateCapturePreflight({
  options,
  dirty,
  processes,
  windows,
  executablePath,
  controllerPid,
  helperPid,
}) {
  if (options?.destructive && dirty) {
    throw new AcceptanceError(
      'PRECONDITION_DIRTY_WORKTREE',
      'Native evidence capture requires a clean worktree',
    )
  }
  const viewportMatch = options?.viewport?.match(/^(\d+)x(\d+)$/)
  if (!viewportMatch) {
    throw new AcceptanceError(
      'PRECONDITION_VIEWPORT',
      'Capture preflight requires an exact viewport',
    )
  }
  const viewer = selectExactViewer(processes, {
    executablePath,
    controllerPid,
    helperPid,
  })
  const window = selectExactWindow(windows, {
    pid: viewer.pid,
    viewport: {
      width: Number(viewportMatch[1]),
      height: Number(viewportMatch[2]),
    },
  })
  return { viewer, window }
}

export function waitFor(predicate, { timeoutMs, intervalMs }) {
  if (
    !Number.isFinite(timeoutMs) ||
    timeoutMs <= 0 ||
    timeoutMs > 10_000 ||
    !Number.isFinite(intervalMs) ||
    intervalMs <= 0
  ) {
    throw new AcceptanceError(
      'PRECONDITION_WAIT_LIMIT',
      'Condition waits must be positive and capped at ten seconds',
      { timeoutMs, intervalMs },
    )
  }
  return (async () => {
    const deadline = Date.now() + timeoutMs
    while (true) {
      const result = await predicate()
      if (result) return result
      if (Date.now() >= deadline) {
        throw new AcceptanceError(
          'PRECONDITION_WAIT_TIMEOUT',
          'Visible state did not become ready before the condition timeout',
          { timeoutMs },
        )
      }
      await new Promise((resolve) =>
        setTimeout(resolve, Math.min(intervalMs, Math.max(1, deadline - Date.now()))),
      )
    }
  })()
}

export async function discoverNativeWindows({ helperPath, pid, protocolTest = false }) {
  if (!Number.isInteger(pid) || pid <= 0) {
    throw new AcceptanceError(
      'PRECONDITION_VIEWER_PID',
      'Window discovery requires a positive Viewer PID',
      { pid },
    )
  }
  const argumentsList = ['--discover-windows', String(pid)]
  if (protocolTest) argumentsList.push('--protocol-test-window-discovery')
  let stdout
  try {
    ;({ stdout } = await execFileAsync(helperPath, argumentsList, {
      encoding: 'utf8',
      maxBuffer: 1024 * 1024,
    }))
  } catch (error) {
    throw new AcceptanceError(
      'PRECONDITION_WINDOW_COUNT',
      'Native helper could not discover Viewer windows',
      { stderr: error?.stderr ?? '', stdout: error?.stdout ?? '' },
    )
  }
  let envelope
  try {
    envelope = JSON.parse(stdout.trim())
  } catch {
    throw new AcceptanceError(
      'SAFETY_PROTOCOL',
      'Native window discovery returned malformed JSON',
    )
  }
  if (
    !envelope ||
    Object.keys(envelope).length !== 1 ||
    !Array.isArray(envelope.windows)
  ) {
    throw new AcceptanceError(
      'SAFETY_PROTOCOL',
      'Native window discovery returned an invalid envelope',
    )
  }
  return envelope.windows.map((window) => {
    const keys = Object.keys(window).sort()
    if (
      JSON.stringify(keys) !==
        JSON.stringify(['height', 'pid', 'title', 'width', 'windowId', 'x', 'y']) ||
      window.pid !== pid ||
      !Number.isInteger(window.windowId) ||
      window.windowId <= 0 ||
      typeof window.title !== 'string' ||
      !['x', 'y', 'width', 'height'].every((key) => Number.isInteger(window[key])) ||
      window.width <= 0 ||
      window.height <= 0
    ) {
      throw new AcceptanceError(
        'SAFETY_PROTOCOL',
        'Native window discovery returned an invalid window',
        { window },
      )
    }
    return window
  })
}

function commandExecutable(command) {
  return command.trim().split(/\s+/, 1)[0]
}

export function parseProcessTable(output) {
  return output
    .split('\n')
    .map((line) => line.match(/^\s*(\d+)\s+(\d+)\s+(\d+)\s+(.+?)\s*$/))
    .filter((match) => match !== null)
    .map((match) => ({
      pid: Number(match[1]),
      ppid: Number(match[2]),
      pgid: Number(match[3]),
      command: match[4],
    }))
}

function isViewerCandidate(processInfo) {
  return path.basename(commandExecutable(processInfo.command)) === 'viewer-desktop'
}

export function selectExactViewer(
  processes,
  { executablePath, controllerPid, helperPid },
) {
  const candidates = processes.filter(isViewerCandidate)

  if (candidates.length === 0) {
    throw new AcceptanceError(
      'PRECONDITION_VIEWER_COUNT',
      'Expected exactly one Viewer process, found none',
      { candidatePids: [] },
    )
  }

  if (candidates.length > 1) {
    throw new AcceptanceError(
      'PRECONDITION_VIEWER_COUNT',
      `Expected exactly one Viewer process, found ${candidates.length}`,
      { candidatePids: candidates.map((item) => item.pid) },
    )
  }

  const viewer = candidates[0]
  if (commandExecutable(viewer.command) !== executablePath) {
    throw new AcceptanceError(
      'PRECONDITION_VIEWER_PATH',
      'Viewer executable is not the exact current-worktree development binary',
      { actual: commandExecutable(viewer.command), expected: executablePath },
    )
  }

  if (viewer.pid === controllerPid || viewer.pid === helperPid) {
    throw new AcceptanceError(
      'PRECONDITION_VIEWER_SELF',
      'Controller or helper process cannot be selected as Viewer',
      { pid: viewer.pid },
    )
  }

  return viewer
}

function isAllowedViewport(viewport) {
  return (
    (viewport?.width === 1024 && viewport?.height === 720) ||
    (viewport?.width === 1440 && viewport?.height === 900)
  )
}

export function validateWindow(window, { pid, viewport }) {
  if (
    window?.pid !== pid ||
    !Number.isInteger(window?.windowId) ||
    window.windowId <= 0
  ) {
    throw new AcceptanceError(
      'PRECONDITION_WINDOW_OWNER',
      'Viewer window is not owned by the approved process',
      { actualPid: window?.pid, expectedPid: pid, windowId: window?.windowId },
    )
  }

  if (
    !isAllowedViewport(viewport) ||
    window.width !== viewport.width ||
    window.height !== viewport.height
  ) {
    throw new AcceptanceError(
      'PRECONDITION_VIEWPORT',
      'Viewer window does not match an exact acceptance viewport',
      {
        actual: { width: window.width, height: window.height },
        expected: viewport,
      },
    )
  }

  return window
}

export function selectExactWindow(windows, options) {
  if (windows.length !== 1) {
    throw new AcceptanceError(
      'PRECONDITION_WINDOW_COUNT',
      `Expected exactly one Viewer main window, found ${windows.length}`,
      { windowIds: windows.map((window) => window.windowId) },
    )
  }

  return validateWindow(windows[0], options)
}

export function validateWindowPoint(point, window) {
  if (
    !Number.isFinite(point?.x) ||
    !Number.isFinite(point?.y) ||
    point.x < 0 ||
    point.y < 0 ||
    point.x >= window.width ||
    point.y >= window.height
  ) {
    throw new AcceptanceError(
      'SAFETY_POINT_OUTSIDE_WINDOW',
      'Pointer coordinate is outside the approved Viewer window',
      { point, window },
    )
  }

  return {
    x: window.x + point.x,
    y: window.y + point.y,
  }
}

function isContainedPath(root, candidate) {
  const relative = path.relative(root, candidate)
  return relative === '' || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative))
}

function validateLiteralAbsolutePath(candidate, code) {
  if (
    typeof candidate !== 'string' ||
    candidate.length === 0 ||
    !path.isAbsolute(candidate) ||
    /[$~*?\[\]{}]/.test(candidate)
  ) {
    throw new AcceptanceError(code, 'Path must be a literal absolute path', {
      candidate,
    })
  }

  return path.normalize(candidate)
}

async function nearestExistingAncestor(candidate) {
  let current = candidate
  while (true) {
    try {
      await lstat(current)
      return current
    } catch (error) {
      if (error?.code !== 'ENOENT') throw error
      const parent = path.dirname(current)
      if (parent === current) throw error
      current = parent
    }
  }
}

async function assertNoSymlinkBetween(root, candidate, code) {
  const relative = path.relative(root, candidate)
  if (!isContainedPath(root, candidate)) {
    throw new AcceptanceError(code, 'Path escapes the approved root', {
      root,
      candidate,
    })
  }

  let current = root
  const segments = relative === '' ? [] : relative.split(path.sep)
  for (const segment of segments) {
    current = path.join(current, segment)
    try {
      const metadata = await lstat(current)
      if (metadata.isSymbolicLink()) {
        throw new AcceptanceError(code, 'Symbolic links are not accepted', {
          candidate,
          symbolicLink: current,
        })
      }
    } catch (error) {
      if (error?.code === 'ENOENT') break
      throw error
    }
  }
}

async function validateContainedPath(candidate, approvedRoot, code) {
  const normalized = validateLiteralAbsolutePath(candidate, code)
  const normalizedRoot = path.normalize(approvedRoot)

  if (!isContainedPath(normalizedRoot, normalized)) {
    throw new AcceptanceError(code, 'Path escapes the approved root', {
      root: normalizedRoot,
      candidate: normalized,
    })
  }

  const [realRoot, existingAncestor] = await Promise.all([
    realpath(normalizedRoot),
    nearestExistingAncestor(normalized),
  ])
  const realAncestor = await realpath(existingAncestor)
  if (!isContainedPath(realRoot, realAncestor)) {
    throw new AcceptanceError(code, 'Resolved path escapes the approved root', {
      root: realRoot,
      candidate: realAncestor,
    })
  }

  await assertNoSymlinkBetween(normalizedRoot, normalized, code)
  return normalized
}

export async function validateFixturePath(candidate, { runId }) {
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/.test(runId)) {
    throw new AcceptanceError('SAFETY_FIXTURE_PATH', 'Invalid fixture run ID', {
      runId,
    })
  }

  const approvedRoot = path.join(
    homedir(),
    'ViewerAcceptanceRuns',
    runId,
  )
  await assertNoSymlinkBetween(
    homedir(),
    approvedRoot,
    'SAFETY_FIXTURE_PATH',
  )
  return validateContainedPath(candidate, approvedRoot, 'SAFETY_FIXTURE_PATH')
}

export async function validateEvidencePath(
  candidate,
  { repoRoot, commit, viewport, id },
) {
  if (
    !/^[0-9a-f]{6,64}$/.test(commit) ||
    !['1024x720', '1440x900'].includes(viewport) ||
    !/^(?:LAU|SID|STR|THU|OTH|SEA|FIL|MEN|RAD|PRE|COM|DOC|INF|DIA|TAS|RES|A11Y)-\d{2}$/.test(id)
  ) {
    throw new AcceptanceError(
      'SAFETY_EVIDENCE_PATH',
      'Invalid evidence path context',
      { commit, viewport, id },
    )
  }

  const approvedRoot = path.join(
    repoRoot,
    'target',
    'atlas-product-migration-acceptance',
    commit,
    viewport,
    id,
  )
  return validateContainedPath(candidate, approvedRoot, 'SAFETY_EVIDENCE_PATH')
}

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function hasExactKeys(value, expected) {
  if (!isRecord(value)) return false
  const actual = Object.keys(value).sort()
  const wanted = [...expected].sort()
  return actual.length === wanted.length && actual.every((key, index) => key === wanted[index])
}

function hasOnlyKeys(value, allowed) {
  return isRecord(value) && Object.keys(value).every((key) => allowed.includes(key))
}

function commandError(message, details = {}) {
  return new AcceptanceError('SAFETY_COMMAND', message, details)
}

function validateTarget(target) {
  const allowed = ['role', 'name', 'identifier', 'position']
  if (!hasOnlyKeys(target, allowed) || Object.keys(target).length === 0) {
    throw commandError('Target must contain only approved selector fields')
  }
  if (target.position !== undefined && target.position !== 'rightmost') {
    throw commandError('Target position must be the deterministic rightmost option')
  }
  for (const value of Object.values(target)) {
    if (typeof value !== 'string' || value.length === 0 || [...value].length > 256) {
      throw commandError('Target selector values must be bounded non-empty strings')
    }
  }
}

function validatePayload(command, payload, window) {
  if (!isRecord(payload)) throw commandError('Command payload must be an object')

  switch (command) {
    case 'inspect':
    case 'shutdown':
      if (!hasExactKeys(payload, [])) throw commandError(`${command} takes no payload fields`)
      return
    case 'query':
    case 'activate':
    case 'focus':
      if (!hasExactKeys(payload, ['target'])) throw commandError(`${command} requires a target`)
      validateTarget(payload.target)
      return
    case 'setValue':
      if (!hasExactKeys(payload, ['target', 'text'])) {
        throw commandError('setValue requires target and text')
      }
      validateTarget(payload.target)
      if (typeof payload.text !== 'string' || [...payload.text].length > 4096) {
        throw commandError('setValue text exceeds the approved limit')
      }
      return
    case 'key': {
      if (!hasExactKeys(payload, ['key', 'modifiers'])) {
        throw commandError('key requires key and modifiers')
      }
      if (!ALLOWED_KEYS.has(payload.key) || !Array.isArray(payload.modifiers)) {
        throw commandError('Unsupported key input')
      }
      const uniqueModifiers = new Set(payload.modifiers)
      if (
        uniqueModifiers.size !== payload.modifiers.length ||
        payload.modifiers.some((modifier) => !ALLOWED_MODIFIERS.has(modifier))
      ) {
        throw commandError('Unsupported or duplicate key modifier')
      }
      return
    }
    case 'pointer':
      if (payload.kind === 'scroll') {
        if (
          !hasExactKeys(payload, ['kind', 'point', 'deltaY']) ||
          !Number.isInteger(payload.deltaY) ||
          payload.deltaY === 0 ||
          Math.abs(payload.deltaY) > 1000
        ) {
          throw commandError('scroll requires a bounded non-zero integer delta')
        }
      } else if (!hasExactKeys(payload, ['kind', 'point'])) {
        throw commandError('pointer requires kind and point')
      }
      if (!['click', 'doubleClick', 'rightClick', 'scroll'].includes(payload.kind)) {
        throw commandError('Unsupported pointer action')
      }
      try {
        validateWindowPoint(payload.point, window)
      } catch (error) {
        throw commandError('Pointer coordinate is outside the approved window', {
          cause: error.code,
        })
      }
      return
    case 'drag':
      if (!hasExactKeys(payload, ['from', 'to', 'durationMs'])) {
        throw commandError('drag requires from, to and durationMs')
      }
      if (
        !Number.isInteger(payload.durationMs) ||
        payload.durationMs < 50 ||
        payload.durationMs > 5000
      ) {
        throw commandError('Drag duration is outside the approved range')
      }
      try {
        validateWindowPoint(payload.from, window)
        validateWindowPoint(payload.to, window)
      } catch (error) {
        throw commandError('Drag coordinate is outside the approved window', {
          cause: error.code,
        })
      }
      return
    case 'capture':
      if (!hasExactKeys(payload, ['path']) || typeof payload.path !== 'string') {
        throw commandError('capture requires a path')
      }
      return
    default:
      throw commandError('Unsupported command')
  }
}

export function validateCommand(request, { window } = {}) {
  const envelopeKeys = [
    'version',
    'sequence',
    'pid',
    'windowId',
    'command',
    'timeoutMs',
    'payload',
  ]
  if (!hasExactKeys(request, envelopeKeys)) {
    throw new AcceptanceError(
      'SAFETY_PROTOCOL',
      'Request envelope fields do not match protocol version 1',
    )
  }
  if (
    request.version !== PROTOCOL_VERSION ||
    !Number.isInteger(request.sequence) ||
    request.sequence <= 0 ||
    !Number.isInteger(request.pid) ||
    request.pid <= 0 ||
    !Number.isInteger(request.windowId) ||
    request.windowId <= 0 ||
    !ALLOWED_COMMANDS.has(request.command) ||
    !Number.isInteger(request.timeoutMs) ||
    request.timeoutMs < 100 ||
    request.timeoutMs > 10000
  ) {
    throw new AcceptanceError(
      'SAFETY_PROTOCOL',
      'Request envelope values violate protocol version 1',
    )
  }

  validatePayload(request.command, request.payload, window)
  return request
}

function validateResponseEnvelope(response) {
  if (!isRecord(response)) return false
  const expectedKeys = response.ok
    ? ['version', 'sequence', 'ok', 'result']
    : ['version', 'sequence', 'ok', 'error']
  return (
    hasExactKeys(response, expectedKeys) &&
    response.version === PROTOCOL_VERSION &&
    Number.isInteger(response.sequence) &&
    response.sequence > 0 &&
    typeof response.ok === 'boolean'
  )
}

export class NativeAcceptanceClient {
  constructor({
    executablePath,
    args = [],
    env = process.env,
    pid,
    window,
    defaultTimeoutMs = 3000,
  }) {
    this.executablePath = executablePath
    this.args = args
    this.env = env
    this.pid = pid
    this.window = window
    this.defaultTimeoutMs = defaultTimeoutMs
    this.sequence = 0
    this.child = undefined
    this.stderr = ''
    this.pending = new Map()
    this.exitPromise = Promise.resolve()
  }

  async start() {
    if (this.child) {
      throw new AcceptanceError(
        'PRECONDITION_HELPER_STATE',
        'Native acceptance helper is already running',
      )
    }

    const child = spawn(this.executablePath, this.args, {
      env: this.env,
      stdio: ['pipe', 'pipe', 'pipe'],
    })
    this.child = child
    child.stdout.setEncoding('utf8')
    child.stderr.setEncoding('utf8')
    child.stderr.on('data', (chunk) => {
      this.stderr = `${this.stderr}${chunk}`.slice(-32 * 1024)
    })

    const lines = readline.createInterface({ input: child.stdout })
    lines.on('line', (line) => this.handleResponseLine(line))
    child.once('error', (error) => {
      this.failPending(
        new AcceptanceError(
          'PRECONDITION_HELPER_EXIT',
          'Native acceptance helper failed to start',
          { cause: error.message, stderr: this.stderr },
        ),
      )
    })
    this.exitPromise = new Promise((resolve) => {
      child.once('close', (code, signal) => {
        lines.close()
        if (this.child === child) this.child = undefined
        this.failPending(
          new AcceptanceError(
            'PRECONDITION_HELPER_EXIT',
            'Native acceptance helper exited before responding',
            { exitCode: code, signal, stderr: this.stderr },
          ),
        )
        resolve({ code, signal })
      })
    })

    return this.request('inspect', {})
  }

  handleResponseLine(line) {
    let response
    try {
      response = JSON.parse(line)
    } catch {
      this.failProtocol('Native acceptance helper returned malformed JSON')
      return
    }

    if (!validateResponseEnvelope(response)) {
      this.failProtocol('Native acceptance helper returned an invalid envelope')
      return
    }

    const pending = this.pending.get(response.sequence)
    if (!pending) {
      this.failProtocol('Native acceptance helper returned an unexpected sequence', {
        sequence: response.sequence,
      })
      return
    }

    this.pending.delete(response.sequence)
    clearTimeout(pending.timer)
    if (response.ok) {
      pending.resolve(response.result)
      return
    }

    const error = isRecord(response.error) ? response.error : {}
    pending.reject(
      new AcceptanceError(
        typeof error.code === 'string' ? error.code : 'SAFETY_PROTOCOL',
        typeof error.message === 'string' ? error.message : 'Native helper rejected command',
        { native: error, stderr: this.stderr },
      ),
    )
  }

  failProtocol(message, details = {}) {
    this.failPending(
      new AcceptanceError('SAFETY_PROTOCOL', message, {
        ...details,
        stderr: this.stderr,
      }),
    )
    void this.terminate()
  }

  failPending(error) {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer)
      pending.reject(error)
    }
    this.pending.clear()
  }

  request(command, payload, { timeoutMs = this.defaultTimeoutMs } = {}) {
    const child = this.child
    if (!child || child.exitCode !== null || child.signalCode !== null) {
      return Promise.reject(
        new AcceptanceError(
          'PRECONDITION_HELPER_EXIT',
          'Native acceptance helper is not running',
          { stderr: this.stderr },
        ),
      )
    }

    const request = validateCommand(
      {
        version: PROTOCOL_VERSION,
        sequence: ++this.sequence,
        pid: this.pid,
        windowId: this.window.windowId,
        command,
        timeoutMs,
        payload,
      },
      { window: this.window },
    )

    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(request.sequence)
        reject(
          new AcceptanceError(
            'PRECONDITION_HELPER_TIMEOUT',
            'Native acceptance helper response timed out',
            { sequence: request.sequence, command, stderr: this.stderr },
          ),
        )
        void this.terminate()
      }, timeoutMs)
      this.pending.set(request.sequence, { resolve, reject, timer })
      child.stdin.write(`${JSON.stringify(request)}\n`, (error) => {
        if (!error) return
        const pending = this.pending.get(request.sequence)
        if (!pending) return
        this.pending.delete(request.sequence)
        clearTimeout(pending.timer)
        reject(
          new AcceptanceError(
            'PRECONDITION_HELPER_EXIT',
            'Failed to write to native acceptance helper',
            { cause: error.message, stderr: this.stderr },
          ),
        )
      })
    })
  }

  async close() {
    if (!this.child) return
    try {
      await this.request('shutdown', {})
      this.child?.stdin.end()
      await this.exitPromise
    } catch (error) {
      await this.terminate()
      throw error
    }
  }

  async terminate() {
    const child = this.child
    if (!child) return
    this.child = undefined
    this.failPending(
      new AcceptanceError(
        'PRECONDITION_HELPER_EXIT',
        'Native acceptance helper was terminated',
        { stderr: this.stderr },
      ),
    )
    if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL')
    child.stdin.destroy()
    await this.exitPromise
  }
}

export async function buildNativeHelper({ repoRoot, sourcePath }) {
  const source = await readFile(sourcePath)
  const sourceHash = createHash('sha256').update(source).digest('hex')
  const buildDirectory = path.join(
    repoRoot,
    'target',
    'native-acceptance-tools',
    sourceHash,
  )
  const executablePath = path.join(
    buildDirectory,
    'viewer-native-acceptance-helper',
  )

  try {
    await access(executablePath)
    return { sourceHash, executablePath }
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error
  }

  await mkdir(buildDirectory, { recursive: true })
  const temporaryPath = `${executablePath}.${process.pid}.${randomUUID()}`
  try {
    await execFileAsync('xcrun', [
      'swiftc',
      '-warnings-as-errors',
      sourcePath,
      '-o',
      temporaryPath,
    ])
    await rename(temporaryPath, executablePath)
  } catch (error) {
    await rm(temporaryPath, { force: true })
    throw error
  }

  return { sourceHash, executablePath }
}

export async function collectNativePreflight({ repoRoot, options }) {
  const executablePath = await realpath(
    path.join(repoRoot, 'target', 'debug', 'viewer-desktop'),
  )
  const [{ stdout: processOutput }, { stdout: branchOutput }, { stdout: commitOutput }, { stdout: statusOutput }] =
    await Promise.all([
      execFileAsync('ps', ['-axo', 'pid=,ppid=,pgid=,command='], {
        encoding: 'utf8',
        maxBuffer: 4 * 1024 * 1024,
      }),
      execFileAsync('git', ['rev-parse', '--abbrev-ref', 'HEAD'], {
        cwd: repoRoot,
        encoding: 'utf8',
      }),
      execFileAsync('git', ['rev-parse', 'HEAD'], {
        cwd: repoRoot,
        encoding: 'utf8',
      }),
      execFileAsync('git', ['status', '--porcelain=v1', '--untracked-files=normal'], {
        cwd: repoRoot,
        encoding: 'utf8',
        maxBuffer: 4 * 1024 * 1024,
      }),
    ])
  const processes = parseProcessTable(processOutput)
  const viewer = selectExactViewer(processes, {
    executablePath,
    controllerPid: process.pid,
  })
  const sourcePath = fileURLToPath(
    new URL('./viewer-native-acceptance.swift', import.meta.url),
  )
  const helper = await buildNativeHelper({ repoRoot, sourcePath })
  const windows = await discoverNativeWindows({
    helperPath: helper.executablePath,
    pid: viewer.pid,
  })
  const preflight = validateCapturePreflight({
    options,
    dirty: statusOutput.trim().length > 0,
    processes,
    windows,
    executablePath,
    controllerPid: process.pid,
  })
  const client = new NativeAcceptanceClient({
    executablePath: helper.executablePath,
    pid: viewer.pid,
    window: preflight.window,
  })
  let inspect
  try {
    inspect = await client.start()
  } catch (error) {
    await client.terminate()
    throw error
  }
  await client.close()
  return {
    branch: branchOutput.trim(),
    commit: commitOutput.trim(),
    dirty: statusOutput.trim().length > 0,
    process: {
      pid: viewer.pid,
      executablePath,
    },
    window: preflight.window,
    inspect,
    helper: {
      sourceHash: helper.sourceHash,
      executablePath: helper.executablePath,
    },
  }
}

async function sha256File(filePath) {
  return createHash('sha256').update(await readFile(filePath)).digest('hex')
}

async function requestWithActionLog(client, actions, command, payload) {
  const startedAt = new Date().toISOString()
  try {
    const result = await client.request(command, payload)
    actions.push({
      sequence: actions.length + 1,
      command,
      payload,
      startedAt,
      completedAt: new Date().toISOString(),
      ok: true,
      result,
    })
    return result
  } catch (error) {
    actions.push({
      sequence: actions.length + 1,
      command,
      payload,
      startedAt,
      completedAt: new Date().toISOString(),
      ok: false,
      error: {
        code: error?.code ?? 'UNEXPECTED',
        message: error?.message ?? String(error),
      },
    })
    throw error
  }
}

async function ensureLaunchNoProject(client, actions) {
  await requestWithActionLog(client, actions, 'focus', {
    target: { role: 'AXWindow' },
  })
  try {
    return await requestWithActionLog(client, actions, 'query', {
      target: { role: 'AXButton', name: '选择项目文件夹' },
    })
  } catch (error) {
    if (error?.code !== 'STATE_TARGET_NOT_FOUND') throw error
  }
  await requestWithActionLog(client, actions, 'activate', {
    target: { role: 'AXButton', name: '更多' },
  })
  await waitFor(
    async () => {
      try {
        return await requestWithActionLog(client, actions, 'query', {
          target: { name: '关闭项目' },
        })
      } catch (error) {
        if (error?.code === 'STATE_TARGET_NOT_FOUND') return false
        throw error
      }
    },
    { timeoutMs: 3000, intervalMs: 50 },
  )
  await requestWithActionLog(client, actions, 'activate', {
    target: { name: '关闭项目' },
  })
  return waitFor(
    async () => {
      try {
        return await requestWithActionLog(client, actions, 'query', {
          target: { role: 'AXButton', name: '选择项目文件夹' },
        })
      } catch (error) {
        if (error?.code === 'STATE_TARGET_NOT_FOUND') return false
        throw error
      }
    },
    { timeoutMs: 10_000, intervalMs: 100 },
  )
}

async function queryVisibleElement(client, actions, target, timeoutMs = 3000) {
  const result = await waitFor(
    async () => {
      try {
        return await client.request('query', { target })
      } catch (error) {
        if (error?.code === 'STATE_TARGET_NOT_FOUND') return false
        throw error
      }
    },
    { timeoutMs, intervalMs: 50 },
  )
  actions.push({
    sequence: actions.length + 1,
    command: 'query',
    payload: { target },
    completedAt: new Date().toISOString(),
    ok: true,
    result,
  })
  return result.elements[0]
}

export async function openProjectViaPanel({
  client,
  actions,
  projectPath,
  window,
}) {
  const userHome = homedir()
  const relative = path.relative(userHome, projectPath)
  if (!isContainedPath(userHome, projectPath) || relative === '') {
    throw new AcceptanceError(
      'SAFETY_FIXTURE_PATH',
      'Open-panel navigation requires a fixture below the current home directory',
      { projectPath },
    )
  }
  const segments = relative.split(path.sep)
  if (
    actions.some(
      (action) =>
        action.command === 'activate' &&
        action.payload?.target?.name === '选择项目文件夹',
    )
  ) {
    throw new AcceptanceError(
      'STATE_OPEN_PANEL_REENTRY',
      'A recipe may open the native project chooser only once',
    )
  }
  let panelOpen = false
  try {
    await requestWithActionLog(client, actions, 'activate', {
      target: { role: 'AXButton', name: '选择项目文件夹' },
    })
    panelOpen = true
    const home = await queryVisibleElement(client, actions, {
      role: 'AXStaticText',
      name: path.basename(userHome),
    })
    await requestWithActionLog(client, actions, 'pointer', {
      kind: 'click',
      point: {
        x: home.frame.x - window.x + home.frame.width / 2,
        y: home.frame.y - window.y + home.frame.height / 2,
      },
    })
    for (const segment of segments) {
      if (segment.startsWith('.')) {
        await requestWithActionLog(client, actions, 'key', {
          key: 'period',
          modifiers: ['shift', 'command'],
        })
      }
      const element = await queryVisibleElement(
        client,
        actions,
        { role: 'AXTextField', name: segment, position: 'rightmost' },
        5000,
      )
      let point = {
        x: element.frame.x - window.x + element.frame.width / 2,
        y: element.frame.y - window.y + element.frame.height / 2,
      }
      if (
        point.x < 0 ||
        point.x >= window.width ||
        point.y < 0 ||
        point.y >= window.height
      ) {
        await requestWithActionLog(client, actions, 'pointer', {
          kind: 'click',
          point: {
            x: Math.max(8, Math.min(window.width - 8, point.x)),
            y: Math.floor(window.height / 2),
          },
        })
        for (let scroll = 0; scroll < 4; scroll += 1) {
          await requestWithActionLog(client, actions, 'pointer', {
            kind: 'scroll',
            point: {
              x: Math.max(8, Math.min(window.width - 8, point.x)),
              y: Math.floor(window.height / 2),
            },
            deltaY: 12,
          })
        }
        const firstCharacter = segment[0].toLowerCase()
        if (
          !ALLOWED_KEYS.has(firstCharacter) ||
          !/^[a-z]$/.test(firstCharacter)
        ) {
          throw new AcceptanceError(
            'STATE_TARGET_OFFSCREEN',
            'Offscreen open-panel segment cannot be reached by approved type-select',
            { segment },
          )
        }
        await requestWithActionLog(client, actions, 'key', {
          key: firstCharacter,
          modifiers: [],
        })
        const visibleElement = await queryVisibleElement(
          client,
          actions,
          { role: 'AXTextField', name: segment, position: 'rightmost' },
          3000,
        )
        point = {
          x: visibleElement.frame.x - window.x + visibleElement.frame.width / 2,
          y:
            visibleElement.frame.y -
            window.y +
            visibleElement.frame.height / 2,
        }
      }
      await requestWithActionLog(client, actions, 'pointer', {
        kind: 'click',
        point,
      })
    }
    await requestWithActionLog(client, actions, 'activate', {
      target: { role: 'AXButton', name: 'Open' },
    })
    const projectVisible = await queryVisibleElement(
      client,
      actions,
      { role: 'AXButton', name: '更多' },
      10_000,
    )
    panelOpen = false
    return projectVisible
  } catch (error) {
    if (panelOpen) {
      try {
        await requestWithActionLog(client, actions, 'activate', {
          target: { role: 'AXButton', name: 'Cancel' },
        })
      } catch {
        try {
          await requestWithActionLog(client, actions, 'key', {
            key: 'escape',
            modifiers: [],
          })
        } catch {}
      }
    }
    throw error
  }
}

async function createEvidenceDirectory({ repoRoot, commit, viewport, id }) {
  const root = path.join(
    repoRoot,
    'target',
    'atlas-product-migration-acceptance',
    commit,
    viewport,
  )
  const directory = path.join(root, id)
  await mkdir(root, { recursive: true })
  try {
    await mkdir(directory)
  } catch (error) {
    if (error?.code === 'EEXIST') {
      throw new AcceptanceError(
        'CAPTURE_EVIDENCE_EXISTS',
        'Evidence directory already exists for this commit, viewport and state',
        { directory },
      )
    }
    throw error
  }
  await validateEvidencePath(directory, { repoRoot, commit, viewport, id })
  return directory
}

export async function resolveAtlasReferencePath({ repoRoot, viewport, id }) {
  if (!['1024x720', '1440x900'].includes(viewport) || !STATE_RECIPES.has(id)) {
    throw new AcceptanceError(
      'CAPTURE_REFERENCE_PATH',
      'Atlas reference requires an approved viewport and acceptance state',
      { viewport, id },
    )
  }
  const atlasPath = path.join(
    repoRoot,
    'docs',
    'prototypes',
    'viewer-complete-ui-visual-atlas.html',
  )
  const atlasHash = createHash('sha256')
    .update(await readFile(atlasPath))
    .digest('hex')
  const referencePath = path.join(
    repoRoot,
    'target',
    'atlas-product-migration-reference',
    atlasHash,
    viewport,
    id,
    'reference.png',
  )
  try {
    await access(referencePath)
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error
    throw new AcceptanceError(
      'CAPTURE_REFERENCE_MISSING',
      'The current atlas hash has no exported reference for this state',
      { atlasHash, viewport, id, referencePath },
    )
  }
  const image = await readRgbaPng(referencePath)
  const [width, height] = viewport.split('x').map(Number)
  if (image.width !== width || image.height !== height) {
    throw new AcceptanceError(
      'CAPTURE_REFERENCE_DIMENSIONS',
      'Atlas reference dimensions do not match the acceptance viewport',
      {
        viewport,
        id,
        referencePath,
        actual: { width: image.width, height: image.height },
      },
    )
  }
  return referencePath
}

async function normalizeEvidenceImages({ repoRoot, viewport, id, directory, rawPath }) {
  const [width, height] = viewport.split('x').map(Number)
  const nativePath = path.join(directory, 'native.png')
  const referencePath = path.join(directory, 'reference.png')
  const combinedPath = path.join(directory, 'combined.png')
  const approvedReference = await resolveAtlasReferencePath({
    repoRoot,
    viewport,
    id,
  })
  await Promise.all([
    execFileAsync('/usr/bin/sips', [
      '-z',
      String(height),
      String(width),
      rawPath,
      '--out',
      nativePath,
    ]),
    execFileAsync('/usr/bin/sips', [
      '-s',
      'format',
      'png',
      '-z',
      String(height),
      String(width),
      approvedReference,
      '--out',
      referencePath,
    ]),
  ])
  await combinePngEvidence({
    reference: referencePath,
    native: nativePath,
    output: combinedPath,
  })
  return { nativePath, referencePath, combinedPath }
}

async function captureLaunchNoProject({ repoRoot, options, preflight }) {
  const id = 'LAU-01'
  const recipe = STATE_RECIPES.get(id)
  const run = await createFixtureRun({
    repoRoot,
    runId: `${preflight.commit.slice(0, 12)}-${process.pid}-${randomUUID()}`,
  })
  const variantPath = await resetFixtureVariant(run, recipe.fixtureVariant)
  const directory = await createEvidenceDirectory({
    repoRoot,
    commit: preflight.commit,
    viewport: options.viewport,
    id,
  })
  const rawPath = path.join(directory, 'native@2x.png')
  await validateEvidencePath(rawPath, {
    repoRoot,
    commit: preflight.commit,
    viewport: options.viewport,
    id,
  })
  const actions = []
  const client = new NativeAcceptanceClient({
    executablePath: preflight.helper.executablePath,
    pid: preflight.process.pid,
    window: preflight.window,
  })
  try {
    await client.start()
    const visible = await ensureLaunchNoProject(client, actions)
    await requestWithActionLog(client, actions, 'capture', { path: rawPath })
    const images = await normalizeEvidenceImages({
      repoRoot,
      viewport: options.viewport,
      id,
      directory,
      rawPath,
    })
    const evidence = {
      raw: { path: rawPath, sha256: await sha256File(rawPath) },
      reference: {
        path: images.referencePath,
        sha256: await sha256File(images.referencePath),
      },
      combined: {
        path: images.combinedPath,
        sha256: await sha256File(images.combinedPath),
      },
    }
    const manifest = buildEvidenceManifest({
      id,
      wave: recipe.wave,
      commit: preflight.commit,
      branch: preflight.branch,
      dirty: preflight.dirty,
      pid: preflight.process.pid,
      executablePath: preflight.process.executablePath,
      window: preflight.window,
      viewport: options.viewport,
      fixture: {
        runId: run.runId,
        variant: recipe.fixtureVariant,
        path: variantPath,
      },
      actions,
      assertion: {
        description: recipe.visibleAssertion,
        passed: Array.isArray(visible?.elements) && visible.elements.length === 1,
      },
      evidence,
      timestamp: new Date().toISOString(),
      verdict: 'pending-visual-review',
    })
    await Promise.all([
      writeFile(
        path.join(directory, 'actions.jsonl'),
        `${actions.map((action) => JSON.stringify(action)).join('\n')}\n`,
        { flag: 'wx' },
      ),
      writeFile(
        path.join(directory, 'manifest.json'),
        `${JSON.stringify(manifest, null, 2)}\n`,
        { flag: 'wx' },
      ),
    ])
    return { directory, manifest }
  } catch (error) {
    await writeFile(
      path.join(directory, 'actions.failed.jsonl'),
      `${actions.map((action) => JSON.stringify(action)).join('\n')}\n`,
      { flag: 'wx' },
    ).catch(() => {})
    throw error
  } finally {
    await client.close().catch(() => client.terminate())
    await removeFixtureRun(run)
  }
}

export async function runNativeAcceptanceCli(
  argv,
  { repoRoot = path.resolve(fileURLToPath(new URL('..', import.meta.url))) } = {},
) {
  const options = parseNativeAcceptanceCli(argv, { repoRoot })
  if (options.mode === 'list') {
    return {
      mode: 'list',
      count: AUDIT_IDS.length,
      recipes: AUDIT_IDS.map((id) => STATE_RECIPES.get(id)),
    }
  }
  const preflight = await collectNativePreflight({ repoRoot, options })
  if (options.mode === 'preflight') {
    return { mode: 'preflight', viewport: options.viewport, ...preflight }
  }
  if (options.mode === 'id' && options.selector === 'LAU-01') {
    return {
      mode: 'capture',
      viewport: options.viewport,
      ...(await captureLaunchNoProject({ repoRoot, options, preflight })),
    }
  }
  throw new AcceptanceError(
    'STATE_RECIPE_EXECUTOR',
    'State capture requires the recipe executor implemented by the next plan task',
    { mode: options.mode, selector: options.selector },
  )
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : ''
if (invokedPath === fileURLToPath(import.meta.url)) {
  try {
    const result = await runNativeAcceptanceCli(process.argv.slice(2))
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
  } catch (error) {
    const failure =
      error instanceof AcceptanceError
        ? { code: error.code, message: error.message, details: error.details }
        : { code: 'UNEXPECTED', message: error?.message ?? String(error) }
    process.stderr.write(`${JSON.stringify(failure, null, 2)}\n`)
    process.exitCode = 1
  }
}
