import { execFile } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { copyFile, mkdir, readFile, rename, writeFile } from 'node:fs/promises'
import { createServer } from 'node:http'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { promisify } from 'node:util'
import {
  combinePngEvidence,
  readRgbaPng,
  sha256File,
} from './viewer-acceptance-evidence.mjs'

const repoRoot = path.resolve(import.meta.dirname, '..')
const execFileAsync = promisify(execFile)
const catalogPath = fileURLToPath(
  new URL('../ui/src/acceptance/acceptanceStateCatalog.json', import.meta.url),
)
const ACCEPTANCE_STATE_DEFINITIONS = Object.freeze(
  JSON.parse(readFileSync(catalogPath, 'utf8')),
)
const CATALOG_BY_ID = new Map(
  ACCEPTANCE_STATE_DEFINITIONS.map((definition) => [definition.id, definition]),
)
const ACCEPTANCE_VIEWPORTS = new Set(['1024x720', '1440x900'])

export class VisualAcceptanceError extends Error {
  constructor(code, message, details = {}) {
    super(message)
    this.name = 'VisualAcceptanceError'
    this.code = code
    this.details = details
  }
}

export function parseVisualAcceptanceCli(argv, options = {}) {
  const root = path.resolve(options.repoRoot ?? repoRoot)
  const parsed = {
    mode: null,
    ids: [],
    wave: null,
    viewports: [],
    outputRoot: path.join(root, 'target', 'viewer-visual-acceptance'),
  }
  let selector = null
  let explicitOutputRoot = false

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index]
    if (argument === '--' && index === 0) continue
    if (argument === '--id') {
      select('ids')
      const id = requireValue(argv, index, '--id')
      index += 1
      if (!CATALOG_BY_ID.has(id)) cliError(`Unknown Viewer acceptance state: ${id}`)
      if (parsed.ids.includes(id)) cliError(`Duplicate Viewer acceptance state: ${id}`)
      parsed.ids.push(id)
      continue
    }
    if (argument === '--wave') {
      select('wave')
      const value = requireValue(argv, index, '--wave')
      index += 1
      const wave = Number(value)
      if (!Number.isInteger(wave) || wave < 1 || wave > 4) {
        cliError(`Unsupported Viewer acceptance wave: ${value}`)
      }
      parsed.wave = wave
      continue
    }
    if (argument === '--all' || argument === '--changed') {
      select(argument.slice(2))
      continue
    }
    if (argument === '--viewport') {
      const viewport = requireValue(argv, index, '--viewport')
      index += 1
      if (!ACCEPTANCE_VIEWPORTS.has(viewport)) {
        cliError(`Unsupported Viewer acceptance viewport: ${viewport}`)
      }
      if (parsed.viewports.includes(viewport)) {
        cliError(`Duplicate Viewer acceptance viewport: ${viewport}`)
      }
      parsed.viewports.push(viewport)
      continue
    }
    if (argument === '--output-root') {
      const value = requireValue(argv, index, '--output-root')
      index += 1
      if (explicitOutputRoot) cliError('--output-root may appear only once')
      explicitOutputRoot = true
      parsed.outputRoot = path.resolve(root, value)
      continue
    }
    cliError(`Unknown Viewer visual acceptance argument: ${argument}`)
  }

  if (selector === null) cliError('One Viewer acceptance selector is required')
  parsed.mode = selector
  if (parsed.viewports.length === 0) parsed.viewports.push('1024x720', '1440x900')
  if (explicitOutputRoot) assertOutputRoot(root, parsed.outputRoot)
  return parsed

  function select(mode) {
    if (selector !== null && selector !== mode) {
      cliError(`Viewer acceptance selectors cannot be combined: ${selector} and ${mode}`)
    }
    if (mode !== 'ids' && selector === mode) {
      cliError(`Viewer acceptance selector may appear only once: ${mode}`)
    }
    selector = mode
  }
}

export function selectVisualAcceptanceIds(options, changedFiles = []) {
  if (options.mode === 'ids') {
    const requested = new Set(options.ids)
    return ACCEPTANCE_STATE_DEFINITIONS.filter(({ id }) => requested.has(id)).map(({ id }) => id)
  }
  if (options.mode === 'wave') {
    return ACCEPTANCE_STATE_DEFINITIONS.filter(({ wave }) => wave === options.wave).map(
      ({ id }) => id,
    )
  }
  if (options.mode === 'all') return ACCEPTANCE_STATE_DEFINITIONS.map(({ id }) => id)
  if (options.mode === 'changed') return affectedAcceptanceIds(changedFiles)
  throw new VisualAcceptanceError('CLI_ARGUMENT', `Unsupported acceptance mode: ${options.mode}`)
}

export async function waitForAcceptanceReady(page, request, timeoutMs = 10_000) {
  await page.waitForFunction(
    ({ id, viewport }) => {
      const frame = document.querySelector(
        `[data-acceptance-frame][data-acceptance-id="${CSS.escape(id)}"]`,
      )
      return (
        frame instanceof HTMLElement &&
        frame.dataset.acceptanceViewport === viewport &&
        (frame.dataset.acceptanceStatus === 'ready' ||
          frame.dataset.acceptanceStatus === 'error')
      )
    },
    request,
    { timeout: timeoutMs },
  )
  const root = page.locator(
    `[data-acceptance-frame][data-acceptance-id="${request.id}"]`,
  )
  const status = await root.getAttribute('data-acceptance-status')
  if (status !== 'ready') {
    throw new VisualAcceptanceError(
      'ACCEPTANCE_SCENE',
      `Viewer acceptance scene ${request.id} reported ${status ?? 'missing'} status`,
      { request, status },
    )
  }
  return page.evaluate(async (expected) => {
    await document.fonts.ready
    const visibleImages = [...document.images].filter((image) => {
      const style = getComputedStyle(image)
      const bounds = image.getBoundingClientRect()
      return (
        style.display !== 'none' &&
        style.visibility !== 'hidden' &&
        Number(style.opacity) !== 0 &&
        bounds.width > 0 &&
        bounds.height > 0
      )
    })
    await Promise.all(
      visibleImages.map(async (image) => {
        if (!image.complete) {
          await new Promise((resolve, reject) => {
            image.addEventListener('load', resolve, { once: true })
            image.addEventListener('error', reject, { once: true })
          })
        }
        await image.decode()
      }),
    )
    await new Promise((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(resolve)),
    )
    const frame = document.querySelector('[data-acceptance-frame]')
    if (!(frame instanceof HTMLElement)) throw new Error('Missing Viewer acceptance frame')
    const bounds = frame.getBoundingClientRect()
    return {
      width: Math.round(bounds.width),
      height: Math.round(bounds.height),
      id: frame.dataset.acceptanceId ?? null,
      viewport: frame.dataset.acceptanceViewport ?? null,
      horizontalOverflow: frame.scrollWidth > frame.clientWidth,
      expected,
    }
  }, request)
}

export async function captureVisualAcceptanceState({
  page,
  baseUrl,
  request,
  outputDirectory,
  timeoutMs = 10_000,
  mkdir: makeDirectory = mkdir,
  now = Date.now,
}) {
  const startedAt = now()
  const consoleErrors = []
  const pageErrors = []
  const onConsole = (message) => {
    if (message.type() === 'error') consoleErrors.push(message.text())
  }
  const onPageError = (error) => pageErrors.push(error.message)
  page.on('console', onConsole)
  page.on('pageerror', onPageError)
  try {
    await makeDirectory(outputDirectory, { recursive: true })
    const environment = acceptanceEnvironmentFor(request.id)
    await page.emulateMedia({
      reducedMotion: environment.reducedMotion,
      forcedColors: environment.forcedColors,
    })
    await page.setViewportSize({ width: request.width, height: request.height })
    const url = new URL('/visual-acceptance.html', baseUrl)
    url.searchParams.set('id', request.id)
    url.searchParams.set('viewport', request.viewport)
    await page.goto(url.href, { waitUntil: 'domcontentloaded', timeout: timeoutMs })
    await page.evaluate(({ zoom }) => {
      document.documentElement.style.zoom = zoom === 1 ? '' : String(zoom)
    }, environment)
    const metrics = await waitForAcceptanceReady(page, request, timeoutMs)
    const readyAt = now()
    assertAcceptanceMetrics(metrics, request)
    await page.addStyleTag({
      content:
        '*, *::before, *::after { animation: none !important; transition: none !important; caret-color: transparent !important; }',
    })
    const productPath = path.join(outputDirectory, 'product.png')
    await page.screenshot({ path: productPath, type: 'png' })
    const capturedAt = now()
    return {
      request,
      productPath,
      metrics,
      consoleErrors,
      pageErrors,
      timing: {
        startedAt,
        readyAt,
        capturedAt,
        readyMs: readyAt - startedAt,
        screenshotMs: capturedAt - readyAt,
        totalMs: capturedAt - startedAt,
      },
    }
  } finally {
    page.off('console', onConsole)
    page.off('pageerror', onPageError)
  }
}

export function acceptanceEnvironmentFor(id) {
  return {
    reducedMotion: id === 'A11Y-03' ? 'reduce' : 'no-preference',
    forcedColors: id === 'A11Y-04' ? 'active' : 'none',
    zoom: id === 'A11Y-05' ? 2 : 1,
  }
}

export async function runVisualAcceptanceBatch(options, dependencies = {}) {
  const changedFiles =
    dependencies.changedFiles ??
    (options.mode === 'changed' ? await collectChangedFiles(repoRoot) : [])
  const selectedIds = selectVisualAcceptanceIds(options, changedFiles)
  const startServer = dependencies.startServer ?? startVisualAcceptanceServer
  const launchBrowser = dependencies.launchBrowser ?? launchChromium
  const makeDirectory = dependencies.mkdir ?? mkdir
  const write = dependencies.writeFile ?? writeFile
  const now = dependencies.now ?? Date.now
  const captureState = dependencies.captureState ?? captureVisualAcceptanceState
  const collectMetadata = dependencies.runMetadata ?? collectRunMetadata
  const finalizeEvidence = dependencies.finalizeEvidence ?? finalizeVisualEvidence
  const resumeEvidence = dependencies.resumeEvidence ?? resumeVisualEvidence
  const startedAt = now()
  const deadline = startedAt + 20 * 60 * 1_000
  const summary = { exitCode: 0, succeeded: [], failed: [], unrun: [], results: [] }
  let server
  let browser
  let runMetadata
  try {
    server = await startServer({ repoRoot, options })
    browser = await launchBrowser()
    runMetadata = await collectMetadata({ repoRoot, browser, now })
    for (const viewport of options.viewports) {
      const { width, height } = dimensionsFor(viewport)
      const context = await browser.newContext()
      const page = await context.newPage()
      try {
        for (const id of selectedIds) {
          if (now() > deadline) {
            summary.exitCode = 1
            summary.unrun.push(
              ...selectedIds.filter((candidate) => !summary.succeeded.includes(candidate) && !summary.failed.some((failure) => failure.id === candidate)),
            )
            break
          }
          const request = { id, viewport, width, height }
          const outputDirectory = path.join(
            options.outputRoot,
            runMetadata.evidenceKey,
            viewport,
            id,
          )
          let outputCreated = false
          try {
            const resumed = await resumeEvidence({ request, outputDirectory, runMetadata })
            if (resumed !== null) {
              summary.succeeded.push(id)
              summary.results.push(resumed)
              continue
            }
            await makeDirectory(path.dirname(outputDirectory), { recursive: true })
            await makeDirectory(outputDirectory)
            outputCreated = true
            const result = await captureState({
              page,
              baseUrl: server.url,
              request,
              outputDirectory,
              mkdir: async () => undefined,
              now,
            })
            const definition = CATALOG_BY_ID.get(id)
            const finalized = await finalizeEvidence(result, {
              repoRoot,
              outputDirectory,
              definition,
              runMetadata,
              writeFile: write,
            })
            summary.succeeded.push(id)
            summary.results.push(finalized)
          } catch (caught) {
            summary.exitCode = 1
            const error = serializeError(caught)
            summary.failed.push({ id, viewport, error })
            if (outputCreated) {
              try {
                await page.screenshot({
                  path: path.join(outputDirectory, 'failure.png'),
                  type: 'png',
                })
              } catch (diagnosticError) {
                error.diagnostic = serializeError(diagnosticError)
              }
              await write(
                path.join(outputDirectory, 'failure.json'),
                `${JSON.stringify({ request, error }, null, 2)}\n`,
              )
            }
          }
        }
      } finally {
        await page.close()
        await context.close()
      }
      if (summary.unrun.length > 0) break
    }
  } finally {
    await browser?.close()
    await server?.close()
  }
  return summary
}

export async function resumeVisualEvidence({ request, outputDirectory, runMetadata }) {
  if (runMetadata.dirty !== false) return null
  const manifestPath = path.join(outputDirectory, 'manifest.json')
  let manifest
  try {
    manifest = JSON.parse(await readFile(manifestPath, 'utf8'))
  } catch (error) {
    if (error?.code === 'ENOENT' || error instanceof SyntaxError) return null
    throw error
  }
  if (
    manifest.schemaVersion !== 1 ||
    manifest.dirty !== false ||
    manifest.commit !== runMetadata.commit ||
    manifest.id !== request.id ||
    manifest.viewport !== request.viewport ||
    manifest.sourceHashes?.catalog !== runMetadata.catalogHash ||
    manifest.sourceHashes?.atlas !== runMetadata.atlasHash ||
    JSON.stringify(manifest.sourceHashes?.css) !== JSON.stringify(runMetadata.cssHashes) ||
    manifest.verdict !== 'pending-visual-review'
  ) {
    return null
  }
  const artifactPaths = {
    product: path.join(outputDirectory, 'product.png'),
    reference: path.join(outputDirectory, 'reference.png'),
    combined: path.join(outputDirectory, 'combined.png'),
  }
  if (
    Object.entries(artifactPaths).some(
      ([key, expected]) => path.resolve(manifest.artifacts?.[key]?.path ?? '') !== path.resolve(expected),
    )
  ) {
    return null
  }
  try {
    await Promise.all([
      ...Object.values(artifactPaths).map((artifactPath) => readFile(artifactPath)),
      readFile(path.join(outputDirectory, 'console.json')),
    ])
  } catch (error) {
    if (error?.code === 'ENOENT') return null
    throw error
  }
  return { request, manifestPath, manifest, resumed: true }
}

const GLOBAL_VISUAL_PATHS = new Set([
  'ui/src/App.tsx',
  'ui/src/styles/app.css',
  'ui/src/styles/primitives.css',
  'ui/src/styles/tokens.css',
  'ui/src/acceptance/acceptanceStateCatalog.json',
  'ui/vite.visual-acceptance.config.ts',
])

const VIEWER_BUTTON_DEPENDENTS = new Set([
  'App',
  'BatchRenameDialog',
  'CloseOperationDialog',
  'CompareWorkspace',
  'DestinationDialog',
  'EmptyProject',
  'FolderFilmstripRow',
  'GlobalNoticeStack',
  'ImagePreview',
  'OperationResults',
  'OtherFilePanel',
  'ReadOnlyBanner',
  'RenameDialog',
  'SearchResults',
  'SearchToolbar',
  'SettingsDialog',
  'TaskBar',
  'TextPreview',
  'TrashConfirmation',
  'UnsupportedFilePreview',
  'ViewerButton',
])

const BOUNDED_COMPONENT_IDS = new Map([
  ['ImagePreview', numberedAcceptanceIds('PRE', 7)],
  [
    'RadialFileMenu',
    [...numberedAcceptanceIds('RAD', 7), 'A11Y-01', 'A11Y-02'],
  ],
])

export function affectedAcceptanceIds(
  changedFiles,
  definitions = ACCEPTANCE_STATE_DEFINITIONS,
) {
  if (changedFiles.length === 0) return []
  const allIds = definitions.map(({ id }) => id)
  const selected = new Set()

  for (const unnormalizedFile of changedFiles) {
    const file = unnormalizedFile.replaceAll('\\', '/').replace(/^\.\//, '')
    if (GLOBAL_VISUAL_PATHS.has(file) || isVisualAuthorityDocument(file)) return allIds
    if (file.startsWith('ui/src/styles/')) return allIds
    if (file.startsWith('ui/src/acceptance/')) return allIds

    if (file.startsWith('docs/')) continue
    if (!file.startsWith('ui/src/components/')) continue

    const componentName = path.basename(file).replace(/\.(?:test\.)?[cm]?[jt]sx?$/, '')
    const boundedIds = BOUNDED_COMPONENT_IDS.get(componentName)
    if (boundedIds !== undefined) {
      addKnownIds(selected, boundedIds, definitions)
      continue
    }

    if (componentName === 'ViewerButton') {
      const dependentIds = definitions
        .filter(({ components }) =>
          components.some((component) => VIEWER_BUTTON_DEPENDENTS.has(component)),
        )
        .map(({ id }) => id)
      if (dependentIds.length === 0) return allIds
      addKnownIds(selected, dependentIds, definitions)
      continue
    }

    const directIds = definitions
      .filter(({ components }) => components.includes(componentName))
      .map(({ id }) => id)
    if (directIds.length === 0) return allIds
    addKnownIds(selected, directIds, definitions)
  }

  return allIds.filter((id) => selected.has(id))
}

function isVisualAuthorityDocument(file) {
  if (!file.startsWith('docs/')) return false
  return (
    file.includes('viewer-complete-ui-visual-atlas') ||
    file.includes('viewer-atlas-product-migration-ledger') ||
    file.includes('viewer-tiered-visual-acceptance-design')
  )
}

function numberedAcceptanceIds(prefix, count) {
  return Array.from(
    { length: count },
    (_, index) => `${prefix}-${String(index + 1).padStart(2, '0')}`,
  )
}

function addKnownIds(selected, ids, definitions) {
  const knownIds = new Set(definitions.map(({ id }) => id))
  for (const id of ids) {
    if (knownIds.has(id)) selected.add(id)
  }
}

export async function collectChangedFiles(root, execute = execFileAsync) {
  const [committed, tracked, untracked] = await Promise.all([
    execute('git', ['diff', '--name-only', 'HEAD^', 'HEAD'], { cwd: root }).catch(
      (error) => {
        if (error?.code === 128) return { stdout: '' }
        throw error
      },
    ),
    execute('git', ['diff', '--name-only', 'HEAD'], { cwd: root }),
    execute('git', ['ls-files', '--others', '--exclude-standard'], { cwd: root }),
  ])
  return [committed.stdout, tracked.stdout, untracked.stdout]
    .flatMap((output) => output.split(/\r?\n/))
    .filter((file) => file.length > 0)
    .filter((file, index, files) => files.indexOf(file) === index)
}

function dimensionsFor(viewport) {
  if (viewport === '1024x720') return { width: 1024, height: 720 }
  if (viewport === '1440x900') return { width: 1440, height: 900 }
  throw new VisualAcceptanceError('CLI_ARGUMENT', `Unsupported viewport: ${viewport}`)
}

function assertAcceptanceMetrics(metrics, request) {
  if (
    metrics.width !== request.width ||
    metrics.height !== request.height ||
    metrics.id !== request.id ||
    metrics.viewport !== request.viewport ||
    metrics.horizontalOverflow
  ) {
    throw new VisualAcceptanceError(
      'ACCEPTANCE_GEOMETRY',
      `Viewer acceptance geometry failed for ${request.id} at ${request.viewport}`,
      { request, metrics },
    )
  }
}

function serializeError(error) {
  return {
    name: error instanceof Error ? error.name : 'Error',
    message: error instanceof Error ? error.message : String(error),
    code: error && typeof error === 'object' && 'code' in error ? error.code : null,
  }
}

async function launchChromium() {
  const { chromium } = await import('playwright')
  return chromium.launch({ headless: true })
}

async function startVisualAcceptanceServer({ repoRoot: root }) {
  await execFileAsync('pnpm', ['build:visual-acceptance'], { cwd: root })
  const siteRoot = path.join(root, 'target', 'viewer-visual-acceptance', 'site')
  const server = createServer(async (request, response) => {
    try {
      const requestUrl = new URL(request.url ?? '/', 'http://127.0.0.1')
      const relativePath = decodeURIComponent(requestUrl.pathname).replace(/^\/+/, '')
      const requestedPath = path.resolve(siteRoot, relativePath)
      const relativeToSite = path.relative(siteRoot, requestedPath)
      if (
        relativePath.length === 0 ||
        relativeToSite.startsWith('..') ||
        path.isAbsolute(relativeToSite)
      ) {
        response.writeHead(404).end('Not found')
        return
      }
      const contents = await readFile(requestedPath)
      response.writeHead(200, {
        'content-type': contentType(requestedPath),
        'cache-control': 'no-store',
      })
      response.end(contents)
    } catch (error) {
      response.writeHead(error?.code === 'ENOENT' ? 404 : 500).end('Not found')
    }
  })
  await new Promise((resolve, reject) => {
    server.once('error', reject)
    server.listen(0, '127.0.0.1', resolve)
  })
  const address = server.address()
  if (address === null || typeof address === 'string') {
    server.close()
    throw new VisualAcceptanceError('SERVER_UNAVAILABLE', 'Visual server has no TCP address')
  }
  return {
    url: `http://127.0.0.1:${address.port}`,
    close: () => new Promise((resolve, reject) => server.close((error) => (error ? reject(error) : resolve()))),
  }
}

async function collectRunMetadata({ repoRoot: root, browser }) {
  const [commit, branch, status] = await Promise.all([
    execFileAsync('git', ['rev-parse', 'HEAD'], { cwd: root }),
    execFileAsync('git', ['branch', '--show-current'], { cwd: root }),
    execFileAsync('git', ['status', '--porcelain=v1'], { cwd: root }),
  ])
  const atlasPath = path.join(root, 'docs', 'prototypes', 'viewer-complete-ui-visual-atlas.html')
  const cssPaths = [
    'ui/src/styles/tokens.css',
    'ui/src/styles/primitives.css',
    'ui/src/styles/app.css',
    'ui/src/styles/filePreviewExtensions.css',
    'ui/src/styles/adaptiveOtherFilePanel.css',
  ]
  const cssHashes = Object.fromEntries(
    await Promise.all(
      cssPaths.map(async (relativePath) => [relativePath, await sha256File(path.join(root, relativePath))]),
    ),
  )
  const playwrightPackage = JSON.parse(
    await readFile(path.join(root, 'node_modules', 'playwright', 'package.json'), 'utf8'),
  )
  const commitHash = commit.stdout.trim()
  return {
    evidenceKey: commitHash,
    commit: commitHash,
    branch: branch.stdout.trim(),
    dirty: status.stdout.trim().length > 0,
    atlasHash: await sha256File(atlasPath),
    catalogHash: await sha256File(catalogPath),
    cssHashes,
    nodeVersion: process.version,
    playwrightVersion: playwrightPackage.version,
    browserVersion: typeof browser.version === 'function' ? browser.version() : 'unknown',
  }
}

async function finalizeVisualEvidence(
  capture,
  { repoRoot: root, outputDirectory, definition, runMetadata, writeFile: write },
) {
  if (definition === undefined) {
    throw new VisualAcceptanceError(
      'CATALOG_STATE',
      `Missing catalog definition for ${capture.request.id}`,
    )
  }
  const referenceSource = path.join(
    root,
    'target',
    'atlas-product-migration-reference',
    runMetadata.atlasHash,
    capture.request.viewport,
    capture.request.id,
    'reference.png',
  )
  const referencePath = path.join(outputDirectory, 'reference.png')
  const combinedPath = path.join(outputDirectory, 'combined.png')
  const consolePath = path.join(outputDirectory, 'console.json')
  const manifestPath = path.join(outputDirectory, 'manifest.json')
  await copyFile(referenceSource, referencePath)
  const combinedDimensions = await combinePngEvidence({
    reference: referencePath,
    native: capture.productPath,
    output: combinedPath,
  })
  const [productImage, referenceImage] = await Promise.all([
    readRgbaPng(capture.productPath),
    readRgbaPng(referencePath),
  ])
  await write(
    consolePath,
    `${JSON.stringify(
      { consoleErrors: capture.consoleErrors, pageErrors: capture.pageErrors },
      null,
      2,
    )}\n`,
  )
  const manifest = {
    schemaVersion: 1,
    commit: runMetadata.commit,
    branch: runMetadata.branch,
    dirty: runMetadata.dirty,
    id: capture.request.id,
    wave: definition.wave,
    referenceState: definition.referenceState,
    viewport: capture.request.viewport,
    sourceHashes: {
      catalog: runMetadata.catalogHash,
      css: runMetadata.cssHashes,
      atlas: runMetadata.atlasHash,
    },
    runtime: {
      node: runMetadata.nodeVersion,
      playwright: runMetadata.playwrightVersion,
      browser: runMetadata.browserVersion,
    },
    timing: capture.timing,
    assertions: capture.metrics,
    consoleErrorCount: capture.consoleErrors.length + capture.pageErrors.length,
    artifacts: {
      product: await imageArtifact(capture.productPath, productImage),
      reference: await imageArtifact(referencePath, referenceImage),
      combined: {
        ...(await imageArtifact(combinedPath, combinedDimensions)),
        width: combinedDimensions.width,
        height: combinedDimensions.height,
      },
    },
    verdict: 'pending-visual-review',
  }
  const temporaryManifest = `${manifestPath}.tmp`
  await write(temporaryManifest, `${JSON.stringify(manifest, null, 2)}\n`, { flag: 'wx' })
  await rename(temporaryManifest, manifestPath)
  return { ...capture, referencePath, combinedPath, manifestPath, manifest }
}

async function imageArtifact(filePath, image) {
  return {
    path: filePath,
    width: image.width,
    height: image.height,
    sha256: await sha256File(filePath),
  }
}

function contentType(filePath) {
  const extension = path.extname(filePath).toLowerCase()
  if (extension === '.html') return 'text/html; charset=utf-8'
  if (extension === '.js') return 'text/javascript; charset=utf-8'
  if (extension === '.css') return 'text/css; charset=utf-8'
  if (extension === '.svg') return 'image/svg+xml'
  if (extension === '.jpg' || extension === '.jpeg') return 'image/jpeg'
  if (extension === '.png') return 'image/png'
  return 'application/octet-stream'
}

function requireValue(argv, index, flag) {
  const value = argv[index + 1]
  if (value === undefined || value.startsWith('--')) cliError(`${flag} requires a value`)
  return value
}

function assertOutputRoot(root, outputRoot) {
  const approvedRoot = path.join(root, 'target', 'viewer-visual-acceptance')
  const relative = path.relative(approvedRoot, outputRoot)
  if (relative.length === 0 || relative.startsWith('..') || path.isAbsolute(relative)) {
    cliError('--output-root must remain below target/viewer-visual-acceptance')
  }
}

function cliError(message) {
  throw new VisualAcceptanceError('CLI_ARGUMENT', message)
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  const options = parseVisualAcceptanceCli(process.argv.slice(2))
  const summary = await runVisualAcceptanceBatch(options)
  process.stdout.write(`${JSON.stringify(printableSummary(summary), null, 2)}\n`)
  process.exitCode = summary.exitCode
}

function printableSummary(summary) {
  return {
    exitCode: summary.exitCode,
    succeeded: summary.succeeded,
    failed: summary.failed,
    unrun: summary.unrun,
    evidence: summary.results.map(({ request, manifestPath }) => ({
      id: request.id,
      viewport: request.viewport,
      manifestPath,
    })),
  }
}
