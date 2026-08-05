import assert from 'node:assert/strict'
import { execFile } from 'node:child_process'
import { mkdtemp, readFile, readdir, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { promisify } from 'node:util'
import { describe, it } from 'node:test'

const execFileAsync = promisify(execFile)
const repoRoot = path.resolve(import.meta.dirname, '..')

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
