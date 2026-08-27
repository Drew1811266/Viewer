import { spawnSync } from 'node:child_process'
import { copyFile, mkdir, mkdtemp, rm } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')
const fixtureRoot = path.join(repositoryRoot, 'tests/fixtures')
const allowedScenarios = new Set([
  'standard',
  'annotations',
  'read_only',
  'writer_busy',
  'corrupt_image',
  'stable_video_failure',
  'replaced',
  'moved',
  'deleted',
  'publish_recovery',
  'multiple_drafts',
])
const maximumHarnessBytes = 64 * 1024

const fixturesByScenario = {
  annotations: [
    ['images/alpha.png', 'hero.png'],
    ['images/srgb.jpg', 'variant.jpg'],
    ['images/p3.jpg', 'pass.jpg'],
  ],
  standard: [
    ['images/alpha.png', 'hero.png'],
    ['images/srgb.jpg', 'variant.jpg'],
    ['images/p3.jpg', 'pass.jpg'],
    ['images/corrupt.jpg', 'broken.jpg'],
  ],
  publish_recovery: [
    ['images/alpha.png', 'hero.png'],
    ['images/srgb.jpg', 'variant.jpg'],
    ['images/p3.jpg', 'pass.jpg'],
    ['images/corrupt.jpg', 'broken.jpg'],
  ],
  corrupt_image: [['images/corrupt.jpg', 'corrupt.jpg']],
  stable_video_failure: [['videos/truncated.mp4', 'unreadable.mp4']],
}

export async function createManualRoundScenario(name) {
  if (!allowedScenarios.has(name)) throw new Error(`unknown manual review scenario: ${name}`)
  const projectRoot = await mkdtemp(path.join(os.tmpdir(), `viewer-review-${name}-`))
  await mkdir(path.join(projectRoot, '.viewer'), { mode: 0o700 })
  const fixtures = fixturesByScenario[name] ?? [['images/alpha.png', 'source.png']]
  for (const [source, destination] of fixtures) {
    await copyFile(path.join(fixtureRoot, source), path.join(projectRoot, destination))
  }

  let cleaned = false
  return {
    projectRoot,
    async run() {
      if (cleaned) throw new Error('manual review scenario was already cleaned up')
      const invocation = spawnSync(
        'cargo',
        [
          'run', '--quiet', '--locked', '-p', 'viewer-desktop', '--example',
          'review_loop_harness', '--', '--project', projectRoot, '--scenario', name,
        ],
        {
          cwd: repositoryRoot,
          encoding: 'utf8',
          maxBuffer: 1024 * 1024,
        },
      )
      if (invocation.error) throw invocation.error
      if (invocation.status !== 0) {
        throw new Error(
          `review loop harness exited ${invocation.status}: ${invocation.stderr.trim()}`,
        )
      }
      const bytes = Buffer.byteLength(invocation.stdout, 'utf8')
      if (bytes === 0 || bytes > maximumHarnessBytes) {
        throw new Error(`review loop harness emitted ${bytes} bytes`)
      }
      let result
      try {
        result = JSON.parse(invocation.stdout)
      } catch {
        throw new Error(`review loop harness did not emit one JSON result: ${invocation.stdout}`)
      }
      if (result.scenario !== name) throw new Error('review loop harness returned another scenario')
      if (JSON.stringify(result).includes(projectRoot)) {
        throw new Error('review loop harness exposed an absolute temporary project path')
      }
      return result
    },
    async cleanup() {
      if (cleaned) return
      cleaned = true
      await rm(projectRoot, { recursive: true, force: true })
    },
  }
}
