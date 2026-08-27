import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'

import { createManualRoundScenario } from './manual-round-e2e.mjs'
import { blake3Hex } from './read-latest.mjs'

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')
const readerPath = path.join(repositoryRoot, 'scripts/review-protocol/read-latest.mjs')
const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i
const feedbackTexts = [
  '降低高光强度，保留布料纹理。',
  '统一背景色温，并修正边缘伪影。',
]
const annotationTexts = [
  '领口需要收窄，保留原有材质。',
  '右袖边缘有伪影，请重画这一段。',
  '左侧接缝需要拉直。',
  '裤脚颜色请与衣身统一。',
]

async function withScenario(name, assertion) {
  const scenario = await createManualRoundScenario(name)
  try {
    await assertion(scenario)
  } finally {
    await scenario.cleanup()
  }
}

function invokeReader(projectRoot, streamId) {
  return spawnSync(
    process.execPath,
    [readerPath, '--project', projectRoot, '--stream', streamId],
    { cwd: repositoryRoot, encoding: 'utf8' },
  )
}

function parseSuccessfulReader(result) {
  assert.equal(result.status, 0, result.stderr)
  return JSON.parse(result.stdout)
}

function countOutcomes(outcomes) {
  return outcomes.reduce(
    (counts, outcome) => ({ ...counts, [outcome.kind]: counts[outcome.kind] + 1 }),
    { pass: 0, revise: 0, unreviewable: 0 },
  )
}

function assertUuid(value) {
  assert.match(value, uuidPattern)
}

function assertSafeRelativePaths(paths) {
  assert.ok(paths.length > 0)
  for (const value of paths) {
    assert.equal(path.isAbsolute(value), false)
    assert.equal(value.includes('..'), false)
    assert.equal(value.includes('\\'), false)
    assert.equal(value.toLowerCase().includes('.viewer'), false)
  }
}

function assertStandardCompletion(result) {
  assert.equal(result.status, 'completed')
  assertUuid(result.streamId)
  assertUuid(result.roundId)
  assert.equal(result.draftIgnoredBeforeCompletion, true)
  assert.deepEqual(result.counts, { total: 4, revise: 2, unreviewable: 1, pass: 1 })
  assert.deepEqual(result.feedbackTexts, feedbackTexts)
  assert.deepEqual(result.feedbackTargetCounts, [1, 2])
  assert.deepEqual(result.unreviewableFailures, ['damaged'])
  assert.equal(result.markerStateIgnored, true)
  assert.equal(result.favoriteStateIgnored, true)
  assertSafeRelativePaths(result.relativePaths)
}

test('a Draft is invisible, then the same Stream completes and is byte-exactly Agent-readable', async () => {
  await withScenario('standard', async (scenario) => {
    const draft = await scenario.run()
    assert.equal(draft.status, 'draft')
    assertUuid(draft.streamId)
    assertUuid(draft.roundId)

    const draftRead = invokeReader(scenario.projectRoot, draft.streamId)
    assert.equal(draftRead.status, 1)
    assert.match(draftRead.stderr, /review stream was not found|no completed review head/)

    const completed = await scenario.run()
    assertStandardCompletion(completed)
    assert.equal(completed.streamId, draft.streamId)
    assert.equal(completed.roundId, draft.roundId)

    const round = parseSuccessfulReader(invokeReader(scenario.projectRoot, completed.streamId))
    assert.equal(round.reviewStreamId, completed.streamId)
    assert.equal(round.reviewRoundId, completed.roundId)
    assert.deepEqual(round.assets.map((asset) => asset.relativePath), completed.relativePaths)
    assert.deepEqual(round.feedback.map((feedback) => feedback.text), feedbackTexts)
    assert.deepEqual(round.feedback.map((feedback) => feedback.targets.length), [1, 2])
    assert.deepEqual(countOutcomes(round.outcomes), { pass: 1, revise: 2, unreviewable: 1 })
  })
})

test('four image annotations survive a service reopen and publish one verified artifact', async () => {
  await withScenario('annotations', async (scenario) => {
    const source = path.join(scenario.projectRoot, 'hero.png')
    const originalDigest = blake3Hex(await readFile(source))
    const completed = await scenario.run()

    assert.equal(completed.status, 'completed')
    assert.equal(completed.resumedAnnotationsVerified, true)
    assert.equal(completed.draftIgnoredBeforeCompletion, true)
    assert.deepEqual(completed.counts, { total: 3, revise: 1, unreviewable: 0, pass: 2 })
    assert.deepEqual(completed.feedbackTexts, annotationTexts)

    const round = parseSuccessfulReader(invokeReader(scenario.projectRoot, completed.streamId))
    assert.equal(round.protocolVersion, 'viewer.review/2')
    assert.equal(round.reviewRoundId, completed.roundId)
    assert.deepEqual(round.feedback.map((feedback) => feedback.text), annotationTexts)
    assert.deepEqual(
      round.feedback.map((feedback) => feedback.targets[0].anchor.kind),
      ['imageRect', 'imageStroke', 'imageRect', 'imageRect'],
    )
    assert.deepEqual(countOutcomes(round.outcomes), { pass: 2, revise: 1, unreviewable: 0 })
    assert.equal(round.artifacts.length, 1)
    const [artifact] = round.artifacts
    const orderedFeedback = [...round.feedback].sort((left, right) => (
      left.createdAtMs - right.createdAtMs || left.feedbackId.localeCompare(right.feedbackId)
    ))
    assert.deepEqual(
      artifact.annotations,
      orderedFeedback.map((feedback, index) => ({ ordinal: index + 1, feedbackId: feedback.feedbackId })),
    )
    assert.equal(artifact.assetVersionId, round.feedback[0].targets[0].assetVersionId)
    assert.equal(artifact.mediaType, 'image/png')
    assert.equal(artifact.width, 640)
    assert.equal(artifact.height, 480)
    assertSafeRelativePaths([artifact.relativePath])
    const artifactBytes = await readFile(path.join(
      scenario.projectRoot,
      '.viewer/reviews/rounds',
      round.reviewRoundId,
      artifact.relativePath,
    ))
    assert.equal(blake3Hex(artifactBytes), artifact.blake3)
    assert.equal(blake3Hex(await readFile(source)), originalDigest)
  })
})

test('read-only and contended projects fail closed without creating a writable round', async () => {
  for (const [name, errorCode] of [
    ['read_only', 'review_read_only'],
    ['writer_busy', 'review_busy'],
  ]) {
    await withScenario(name, async (scenario) => {
      const result = await scenario.run()
      assert.equal(result.status, 'blocked')
      assert.equal(result.errorCode, errorCode)
      assert.equal(result.streamId, null)
      assert.equal(result.roundId, null)
    })
  }
})

test('stable preparation failures freeze deterministic unreviewable outcomes', async () => {
  for (const [name, failure] of [
    ['corrupt_image', 'damaged'],
    ['stable_video_failure', 'unreadable'],
  ]) {
    await withScenario(name, async (scenario) => {
      const result = await scenario.run()
      assert.equal(result.status, 'completed')
      assert.deepEqual(result.counts, { total: 1, revise: 0, unreviewable: 1, pass: 0 })
      assert.deepEqual(result.unreviewableFailures, [failure])
      assertSafeRelativePaths(result.relativePaths)
    })
  }
})

test('replaced, moved, and deleted assets block publication with explicit conflicts', async () => {
  for (const [name, conflict] of [
    ['replaced', 'replaced'],
    ['moved', 'moved'],
    ['deleted', 'missing'],
  ]) {
    await withScenario(name, async (scenario) => {
      const result = await scenario.run()
      assert.equal(result.status, 'conflict')
      assert.deepEqual(result.conflictKinds, [conflict])
      assert.equal(result.completedRoundPublished, false)
      assertUuid(result.streamId)
      assertUuid(result.roundId)
    })
  }
})

test('an interrupted publication recovers before exposing exactly one completed head', async () => {
  await withScenario('publish_recovery', async (scenario) => {
    const draft = await scenario.run()
    assert.equal(draft.status, 'draft')

    const interrupted = await scenario.run()
    assert.equal(interrupted.status, 'publishInterrupted')
    assert.equal(invokeReader(scenario.projectRoot, draft.streamId).status, 1)

    const recovered = await scenario.run()
    assertStandardCompletion(recovered)
    assert.equal(recovered.recoveredPublish, true)

    const round = parseSuccessfulReader(invokeReader(scenario.projectRoot, recovered.streamId))
    assert.equal(round.reviewRoundId, recovered.roundId)
    assert.deepEqual(countOutcomes(round.outcomes), { pass: 1, revise: 2, unreviewable: 1 })
  })
})

test('multiple Drafts enter recovery-required state instead of guessing a winner', async () => {
  await withScenario('multiple_drafts', async (scenario) => {
    const result = await scenario.run()
    assert.equal(result.status, 'recoveryRequired')
    assert.equal(result.errorCode, 'review_recovery_required')
    assert.equal(result.streamId, null)
    assert.equal(result.roundId, null)
  })
})
