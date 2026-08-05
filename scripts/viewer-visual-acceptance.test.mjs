import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { mkdtemp, readFile, readdir, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { promisify } from 'node:util'
import { describe, it } from 'node:test'
import {
  combinePngEvidence,
  readRgbaPng,
  writeRgbaPng,
} from './viewer-acceptance-evidence.mjs'
import {
  acceptanceEnvironmentFor,
  parseVisualAcceptanceCli,
  runVisualAcceptanceBatch,
  selectVisualAcceptanceIds,
  waitForAcceptanceReady,
} from './viewer-visual-acceptance.mjs'

const execFileAsync = promisify(execFile)
const repoRoot = path.resolve(import.meta.dirname, '..')

describe('visual acceptance CLI', () => {
  it('selects exactly one bounded catalog selector', () => {
    assert.deepEqual(parseVisualAcceptanceCli(['--id', 'PRE-01']).ids, ['PRE-01'])
    assert.deepEqual(parseVisualAcceptanceCli(['--', '--id', 'PRE-01']).ids, ['PRE-01'])
    assert.equal(parseVisualAcceptanceCli(['--wave', '2']).wave, 2)
    assert.equal(parseVisualAcceptanceCli(['--all']).mode, 'all')
    assert.equal(parseVisualAcceptanceCli(['--changed']).mode, 'changed')
    assert.throws(() => parseVisualAcceptanceCli(['--all', '--wave', '2']), {
      code: 'CLI_ARGUMENT',
    })
    assert.throws(() => parseVisualAcceptanceCli(['--id', 'PRE-99']), {
      code: 'CLI_ARGUMENT',
    })
    assert.throws(
      () =>
        parseVisualAcceptanceCli(['--all', '--output-root', '../outside'], {
          repoRoot,
        }),
      { code: 'CLI_ARGUMENT' },
    )
    assert.deepEqual(
      selectVisualAcceptanceIds(
        { mode: 'changed' },
        ['ui/src/components/ImagePreview.tsx'],
      ),
      ['PRE-01', 'PRE-02', 'PRE-03', 'PRE-04', 'PRE-05', 'PRE-06', 'PRE-07', 'A11Y-05'],
    )
  })
})

describe('Viewer visual acceptance browser lifecycle', () => {
  it('maps accessibility states to browser media and zoom without native platform claims', () => {
    assert.deepEqual(acceptanceEnvironmentFor('A11Y-03'), {
      reducedMotion: 'reduce',
      forcedColors: 'none',
      zoom: 1,
    })
    assert.deepEqual(acceptanceEnvironmentFor('A11Y-04'), {
      reducedMotion: 'no-preference',
      forcedColors: 'active',
      zoom: 1,
    })
    assert.deepEqual(acceptanceEnvironmentFor('A11Y-05'), {
      reducedMotion: 'no-preference',
      forcedColors: 'none',
      zoom: 2,
    })
    assert.deepEqual(acceptanceEnvironmentFor('PRE-01'), {
      reducedMotion: 'no-preference',
      forcedColors: 'none',
      zoom: 1,
    })
  })

  it('reuses one browser and one page per viewport in stable catalog order', async () => {
    const harness = fakePlaywright()
    const summary = await runVisualAcceptanceBatch(
      batchOptions(['PRE-01', 'PRE-02', 'PRE-03']),
      harness.dependencies,
    )

    assert.equal(harness.calls.filter((call) => call === 'launchBrowser').length, 1)
    assert.equal(harness.calls.filter((call) => call === 'newContext').length, 1)
    assert.equal(harness.calls.filter((call) => call === 'newPage').length, 1)
    assert.deepEqual(
      harness.calls.filter((call) => call.startsWith('goto:')),
      ['goto:PRE-01', 'goto:PRE-02', 'goto:PRE-03'],
    )
    assert.deepEqual(
      harness.calls.filter((call) => call.startsWith('viewport:')),
      ['viewport:1024x720', 'viewport:1024x720', 'viewport:1024x720'],
    )
    assert.deepEqual(
      harness.calls.filter((call) => call.startsWith('screenshot:')),
      ['screenshot:PRE-01/product.png', 'screenshot:PRE-02/product.png', 'screenshot:PRE-03/product.png'],
    )
    assert.equal(summary.exitCode, 0)
    assert.deepEqual(summary.succeeded, ['PRE-01', 'PRE-02', 'PRE-03'])
    assert.deepEqual(harness.calls.slice(-4), ['page.close', 'context.close', 'browser.close', 'server.close'])
  })

  it('isolates state failure, captures one diagnostic and continues without retrying', async () => {
    const harness = fakePlaywright({ 'PRE-02': 'error' })
    const summary = await runVisualAcceptanceBatch(
      batchOptions(['PRE-01', 'PRE-02', 'PRE-03']),
      harness.dependencies,
    )

    assert.equal(summary.exitCode, 1)
    assert.deepEqual(summary.succeeded, ['PRE-01', 'PRE-03'])
    assert.deepEqual(summary.failed.map(({ id }) => id), ['PRE-02'])
    assert.deepEqual(
      harness.calls.filter((call) => call === 'goto:PRE-02'),
      ['goto:PRE-02'],
    )
    assert.ok(harness.calls.includes('diagnostic:PRE-02/failure.png'))
    assert.ok(harness.calls.includes('goto:PRE-03'))
  })

  it('resumes exact clean evidence without reopening or overwriting that state', async () => {
    const harness = fakePlaywright()
    harness.dependencies.resumeEvidence = async ({ request, outputDirectory }) => {
      if (request.id !== 'PRE-02') return null
      harness.calls.push(`resume:${request.id}`)
      return {
        request,
        manifestPath: path.join(outputDirectory, 'manifest.json'),
        manifest: { id: request.id, viewport: request.viewport },
        resumed: true,
      }
    }

    const summary = await runVisualAcceptanceBatch(
      batchOptions(['PRE-01', 'PRE-02', 'PRE-03']),
      harness.dependencies,
    )

    assert.equal(summary.exitCode, 0)
    assert.deepEqual(summary.succeeded, ['PRE-01', 'PRE-02', 'PRE-03'])
    assert.ok(harness.calls.includes('resume:PRE-02'))
    assert.equal(harness.calls.includes('goto:PRE-02'), false)
    assert.equal(harness.calls.includes('screenshot:PRE-02/product.png'), false)
  })

  it('uses bounded ready, font, image and two-frame conditions', async () => {
    const harness = fakePlaywright()
    const request = { id: 'PRE-01', viewport: '1024x720', width: 1024, height: 720 }

    await waitForAcceptanceReady(harness.page, request, 3210)

    assert.ok(harness.calls.includes('waitForStatus:PRE-01:3210'))
    assert.ok(harness.calls.includes('status:PRE-01'))
    assert.ok(harness.calls.includes('settle:PRE-01:fonts:images:2frames'))
  })
})

describe('Viewer shared lossless evidence', () => {
  it('combines two 2 by 2 PNGs into one 4 by 2 comparison', async () => {
    const temporaryRoot = await mkdtemp(path.join(tmpdir(), 'viewer-visual-evidence-'))
    try {
      const reference = path.join(temporaryRoot, 'reference.png')
      const product = path.join(temporaryRoot, 'product.png')
      const combined = path.join(temporaryRoot, 'combined.png')
      await writeRgbaPng(reference, {
        width: 2,
        height: 2,
        data: Buffer.alloc(2 * 2 * 4, 32),
      })
      await writeRgbaPng(product, {
        width: 2,
        height: 2,
        data: Buffer.alloc(2 * 2 * 4, 224),
      })

      await combinePngEvidence({ reference, native: product, output: combined })
      const image = await readRgbaPng(combined)
      assert.equal(image.width, 4)
      assert.equal(image.height, 2)
      assert.deepEqual(image.data.subarray(0, 8), Buffer.alloc(8, 32))
      assert.deepEqual(image.data.subarray(8, 16), Buffer.alloc(8, 224))
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })

  it('rejects comparison PNGs with unequal heights', async () => {
    const temporaryRoot = await mkdtemp(path.join(tmpdir(), 'viewer-visual-evidence-size-'))
    try {
      const reference = path.join(temporaryRoot, 'reference.png')
      const product = path.join(temporaryRoot, 'product.png')
      await writeRgbaPng(reference, {
        width: 2,
        height: 2,
        data: Buffer.alloc(2 * 2 * 4),
      })
      await writeRgbaPng(product, {
        width: 2,
        height: 1,
        data: Buffer.alloc(2 * 1 * 4),
      })

      await assert.rejects(
        combinePngEvidence({
          reference,
          native: product,
          output: path.join(temporaryRoot, 'combined.png'),
        }),
        { code: 'CAPTURE_COMPARISON_SIZE' },
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
    }
  })
})

describe('Viewer visual acceptance release isolation', () => {
  it('keeps the acceptance entry out of production while building it separately', async () => {
    const temporaryRoot = await mkdtemp(path.join(tmpdir(), 'viewer-release-isolation-'))
    const productionRoot = path.join(temporaryRoot, 'production')
    const acceptanceRoot = path.join(
      repoRoot,
      'target',
      'viewer-visual-acceptance',
      'site',
    )
    try {
      await execFileAsync(
        'pnpm',
        ['--dir', 'ui', 'exec', 'vite', 'build', '--outDir', productionRoot, '--emptyOutDir'],
        { cwd: repoRoot },
      )
      const productionArtifacts = await emittedArtifacts(productionRoot)
      assert.ok(productionArtifacts.some(({ relativePath }) => relativePath === 'index.html'))
      assert.equal(
        productionArtifacts.some(({ relativePath, content }) =>
          /visual-acceptance|data-acceptance-id|acceptanceStateCatalog/.test(
            `${relativePath}\n${content}`,
          ),
        ),
        false,
      )

      await execFileAsync(
        'pnpm',
        ['--dir', 'ui', 'exec', 'vite', 'build', '--config', 'vite.visual-acceptance.config.ts'],
        { cwd: repoRoot },
      )
      const acceptanceArtifacts = await emittedArtifacts(acceptanceRoot)
      assert.deepEqual(
        acceptanceArtifacts
          .map(({ relativePath }) => relativePath)
          .filter((relativePath) => relativePath.endsWith('.html')),
        ['visual-acceptance.html'],
      )
      assert.equal(
        acceptanceArtifacts.filter(({ relativePath }) => /^商品-\d{2}\.jpg$/.test(relativePath))
          .length,
        30,
      )
      assert.equal(
        acceptanceArtifacts.some(({ content }) => content.includes('data-acceptance-id')),
        true,
      )
    } finally {
      await rm(temporaryRoot, { recursive: true, force: true })
      await rm(acceptanceRoot, { recursive: true, force: true })
    }
  })
})

async function emittedArtifacts(root) {
  const artifacts = []
  async function visit(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const absolutePath = path.join(directory, entry.name)
      if (entry.isDirectory()) {
        await visit(absolutePath)
      } else if (entry.isFile()) {
        const relativePath = path.relative(root, absolutePath)
        const bytes = await readFile(absolutePath)
        artifacts.push({
          relativePath,
          content: bytes.includes(0) ? '' : bytes.toString('utf8'),
        })
      }
    }
  }
  await visit(root)
  return artifacts.sort((left, right) => left.relativePath.localeCompare(right.relativePath))
}

function batchOptions(ids) {
  return {
    mode: 'ids',
    ids,
    wave: null,
    viewports: ['1024x720'],
    outputRoot: '/approved/target/viewer-visual-acceptance/test-run',
  }
}

function fakePlaywright(statusById = {}) {
  const calls = []
  let currentId = 'PRE-01'
  const page = {
    on() {},
    off() {},
    async setViewportSize({ width, height }) {
      calls.push(`viewport:${width}x${height}`)
    },
    async emulateMedia({ reducedMotion, forcedColors }) {
      calls.push(`media:${currentId}:${reducedMotion}:${forcedColors}`)
    },
    async goto(url) {
      currentId = new URL(url).searchParams.get('id')
      calls.push(`goto:${currentId}`)
    },
    async waitForFunction(_predicate, request, options) {
      currentId = request.id
      calls.push(`waitForStatus:${request.id}:${options.timeout}`)
    },
    locator() {
      return {
        async getAttribute(name) {
          assert.equal(name, 'data-acceptance-status')
          calls.push(`status:${currentId}`)
          return statusById[currentId] ?? 'ready'
        },
      }
    },
    async evaluate(_callback, request) {
      if ('zoom' in request) {
        calls.push(`zoom:${currentId}:${request.zoom}`)
        return undefined
      }
      calls.push(`settle:${request.id}:fonts:images:2frames`)
      return {
        width: request.width,
        height: request.height,
        id: request.id,
        viewport: request.viewport,
        horizontalOverflow: false,
      }
    },
    async addStyleTag() {
      calls.push(`style:${currentId}`)
    },
    async screenshot({ path: screenshotPath }) {
      const name = path.basename(screenshotPath)
      const kind = name === 'failure.png' ? 'diagnostic' : 'screenshot'
      calls.push(`${kind}:${currentId}/${name}`)
      return Buffer.from('png')
    },
    async close() {
      calls.push('page.close')
    },
  }
  const context = {
    async newPage() {
      calls.push('newPage')
      return page
    },
    async close() {
      calls.push('context.close')
    },
  }
  const browser = {
    async newContext() {
      calls.push('newContext')
      return context
    },
    async close() {
      calls.push('browser.close')
    },
  }
  const server = {
    url: 'http://127.0.0.1:4173',
    async close() {
      calls.push('server.close')
    },
  }
  return {
    calls,
    page,
    dependencies: {
      async startServer() {
        calls.push('startServer')
        return server
      },
      async launchBrowser() {
        calls.push('launchBrowser')
        return browser
      },
      async mkdir() {},
      async writeFile() {},
      now: () => 1_000,
      async runMetadata() {
        return { evidenceKey: 'test-commit' }
      },
      async finalizeEvidence(result) {
        return result
      },
    },
  }
}
