import { spawnSync } from 'node:child_process'
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { blake3Hex } from './read-latest.mjs'
import { readCurrentReview } from './read-current.mjs'
import { readReviewHistory } from './read-history.mjs'

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')
const fixtureRoot = path.join(repositoryRoot, 'tests/fixtures')
const projectId = '00000000-0000-4000-8000-000000000001'
const maximumHarnessBytes = 64 * 1024
const scenarioNames = new Set([
  'two_iterations',
  'partial_shared',
  'source_replaced',
  'unknown_basis',
  'restore_conflict',
  'lost_receipt',
  'migration',
])

const fixtureCopies = {
  two_iterations: [
    ['images/srgb.jpg', 'source-1.jpg'],
    ['images/p3.jpg', 'source-2.jpg'],
    ['images/rotated-6.jpg', 'source-3.jpg'],
  ],
  partial_shared: [
    ['images/srgb.jpg', 'source-1.jpg'],
    ['images/p3.jpg', 'source-2.jpg'],
  ],
  source_replaced: [
    ['images/srgb.jpg', 'source-1.jpg'],
    ['images/p3.jpg', 'source-2.jpg'],
  ],
  unknown_basis: [['images/srgb.jpg', 'source-1.jpg']],
  restore_conflict: [
    ['images/srgb.jpg', 'source-1.jpg'],
    ['images/p3.jpg', 'source-2.jpg'],
  ],
  lost_receipt: [['images/srgb.jpg', 'source-1.jpg']],
  migration: [['images/srgb.jpg', 'source-1.jpg']],
}

export async function createContinuousReviewScenario(name) {
  if (!scenarioNames.has(name)) throw new Error(`unknown continuous review scenario: ${name}`)
  const projectRoot = await mkdtemp(path.join(os.tmpdir(), `viewer-continuous-${name}-`))
  await mkdir(path.join(projectRoot, '.viewer'), { recursive: true, mode: 0o700 })
  await writeFile(
    path.join(projectRoot, '.viewer/project.json'),
    JSON.stringify({ schemaVersion: 1, projectId, createdAtMs: 1 }),
    { mode: 0o600 },
  )
  const copies = fixtureCopies[name]
  const fixtureDigests = new Map()
  for (const [source, destination] of copies) {
    const fixture = path.join(fixtureRoot, source)
    fixtureDigests.set(source, blake3Hex(await readFile(fixture)))
    await copyFile(fixture, path.join(projectRoot, destination))
  }
  if (name === 'migration') await installLegacyFixture(projectRoot)

  let cleaned = false
  return {
    projectRoot,
    async run() {
      if (cleaned) throw new Error('continuous review scenario was already cleaned up')
      const result =
        name === 'two_iterations'
          ? await runTwoIterations(projectRoot)
          : runHarness(projectRoot, name)
      validateHarnessResult(result, name, projectRoot)
      const current = await readCurrentReview({ projectRoot, reviewStreamId: result.streamId })
      if (current.status !== 'ok' || current.role !== 'current')
        throw new Error('independent reader did not return the committed current review')
      if (current.snapshotRef.snapshotId !== result.currentSnapshotId)
        throw new Error('independent reader returned a different current snapshot')
      if (result.archiveId !== null) {
        const history = await readReviewHistory({
          projectRoot,
          reviewStreamId: result.streamId,
          archiveId: result.archiveId,
        })
        if (history.status !== 'ok' || history.role !== 'history')
          throw new Error('independent reader did not return explicit history')
      }
      if (result.legacyRoundId !== null) {
        const history = await readReviewHistory({
          projectRoot,
          reviewStreamId: result.streamId,
          legacyRoundId: result.legacyRoundId,
        })
        if (history.status !== 'ok' || history.role !== 'history')
          throw new Error('independent reader did not return explicit legacy history')
      }
      let originalFixtureUnchanged = true
      for (const [source, digest] of fixtureDigests) {
        originalFixtureUnchanged &&= blake3Hex(await readFile(path.join(fixtureRoot, source))) === digest
      }
      if (!originalFixtureUnchanged) throw new Error('repository fixture changed during scenario')
      return {
        ...result,
        checks: { ...result.checks, originalFixtureUnchanged },
      }
    },
    async cleanup() {
      if (cleaned) return
      cleaned = true
      await rm(projectRoot, { recursive: true, force: true })
    },
  }
}

function runHarness(projectRoot, name, step) {
  const invocation = spawnSync(
        'cargo',
        [
          'run',
          '--quiet',
          '--locked',
          '-p',
          'viewer-desktop',
          '--example',
          'continuous_review_harness',
          '--',
          '--project',
          projectRoot,
          '--scenario',
          name,
          ...(step === undefined ? [] : ['--step', step]),
        ],
        { cwd: repositoryRoot, encoding: 'utf8', maxBuffer: 1024 * 1024 },
      )
  if (invocation.error) throw invocation.error
  if (invocation.status !== 0) {
    throw new Error(
      `continuous review harness exited ${invocation.status}: ${invocation.stderr.trim()}`,
    )
  }
  const bytes = Buffer.byteLength(invocation.stdout, 'utf8')
  if (bytes === 0 || bytes > maximumHarnessBytes)
    throw new Error(`continuous review harness emitted ${bytes} bytes`)
  try {
    return JSON.parse(invocation.stdout)
  } catch {
    throw new Error('continuous review harness did not emit one JSON result')
  }
}

async function runTwoIterations(projectRoot) {
  const first = runHarness(projectRoot, 'two_iterations', 'save-first')
  validateHarnessResult(first, 'two_iterations', projectRoot)
  const firstRead = await readCurrentReview({ projectRoot, reviewStreamId: first.streamId })
  if (firstRead.feedback[0]?.text !== '袖口收紧') throw new Error('first cycle independent read failed')

  await copyFile(path.join(projectRoot, 'source-2.jpg'), path.join(projectRoot, 'source-1.jpg'))
  const firstArchive = runHarness(projectRoot, 'two_iterations', 'archive-first')
  validateHarnessResult(firstArchive, 'two_iterations', projectRoot)
  const firstHistory = await readReviewHistory({
    projectRoot,
    reviewStreamId: first.streamId,
    archiveId: firstArchive.archiveId,
  })
  if (firstHistory.entries[0]?.feedback[0]?.text !== '袖口收紧')
    throw new Error('first cycle history read failed')

  const second = runHarness(projectRoot, 'two_iterations', 'save-second')
  validateHarnessResult(second, 'two_iterations', projectRoot)
  const secondRead = await readCurrentReview({ projectRoot, reviewStreamId: first.streamId })
  if (!secondRead.feedback.some((feedback) => feedback.text === '第二轮待归档意见'))
    throw new Error('second cycle independent read failed')

  await copyFile(path.join(projectRoot, 'source-3.jpg'), path.join(projectRoot, 'source-1.jpg'))
  const secondArchive = runHarness(projectRoot, 'two_iterations', 'archive-second')
  validateHarnessResult(secondArchive, 'two_iterations', projectRoot)
  const secondHistory = await readReviewHistory({
    projectRoot,
    reviewStreamId: first.streamId,
    archiveId: secondArchive.archiveId,
  })
  if (
    !secondHistory.entries
      .flatMap((entry) => entry.feedback)
      .some((feedback) => feedback.text === '第二轮待归档意见')
  )
    throw new Error('second cycle history read failed')

  const final = runHarness(projectRoot, 'two_iterations', 'save-final')
  validateHarnessResult(final, 'two_iterations', projectRoot)
  const finalRead = await readCurrentReview({ projectRoot, reviewStreamId: first.streamId })
  if (!finalRead.feedback.some((feedback) => feedback.text === '第三版的新意见'))
    throw new Error('final current independent read failed')

  return {
    ...final,
    basisSnapshotId: first.basisSnapshotId,
    archiveId: firstArchive.archiveId,
    archiveIds: [firstArchive.archiveId, secondArchive.archiveId],
    editedFeedbackId: firstArchive.editedFeedbackId,
    cycleReads: 2,
    checks: {
      ...final.checks,
      currentPreservedLaterEdit: firstArchive.checks.currentPreservedLaterEdit,
      historyPreservedBasisText: firstArchive.checks.historyPreservedBasisText,
      sourceReplacementDetected: firstArchive.checks.sourceReplacementDetected,
      twoDistinctCycles:
        firstArchive.archiveId !== secondArchive.archiveId &&
        firstHistory.entries[0]?.snapshotRef.snapshotId !==
        secondHistory.entries[0]?.snapshotRef.snapshotId,
    },
  }
}

async function installLegacyFixture(projectRoot) {
  const reviews = path.join(projectRoot, '.viewer/reviews')
  await mkdir(path.join(reviews, 'rounds'), { recursive: true, mode: 0o700 })
  await mkdir(path.join(reviews, 'drafts'), { recursive: true, mode: 0o700 })
  const index = JSON.parse(
    await readFile(path.join(fixtureRoot, 'review-protocol/review-index-v1.valid.json'), 'utf8'),
  )
  index.streams = [index.streams[1]]
  await writeFile(path.join(reviews, 'index.json'), JSON.stringify(index), { mode: 0o600 })
  await copyFile(
    path.join(fixtureRoot, 'review-protocol/review-round-v1.valid.json'),
    path.join(reviews, 'rounds/00000000-0000-4000-8000-000000000202.json'),
  )
  await copyFile(
    path.join(fixtureRoot, 'review-protocol/review-draft-v1.valid.json'),
    path.join(reviews, 'drafts/00000000-0000-4000-8000-000000000203.json'),
  )
}

function validateHarnessResult(result, name, projectRoot) {
  if (result === null || typeof result !== 'object' || Array.isArray(result))
    throw new Error('continuous review harness returned a non-object')
  if (result.scenario !== name) throw new Error('continuous review harness returned another scenario')
  if (JSON.stringify(result).includes(projectRoot))
    throw new Error('continuous review harness exposed an absolute temporary project path')
  for (const field of ['streamId', 'currentSnapshotId']) {
    if (typeof result[field] !== 'string' || result[field].length === 0)
      throw new Error(`continuous review harness omitted ${field}`)
  }
  if (result.fixture !== undefined && path.isAbsolute(result.fixture))
    throw new Error('continuous review harness exposed an absolute fixture path')
  if (JSON.stringify(result).match(/agent.{0,24}(executed|completed|fixed|verified)/i))
    throw new Error('continuous review harness claimed external execution facts')
}
