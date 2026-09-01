import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import path from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

import { readCurrentReview } from './read-current.mjs'
import { readReviewHistory } from './read-history.mjs'
import { createContinuousReviewScenario } from './continuous-review-e2e.mjs'

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..')

async function withScenario(name, assertion) {
  const scenario = await createContinuousReviewScenario(name)
  try {
    await assertion(scenario)
  } finally {
    await scenario.cleanup()
  }
}

test('bare documented two_iterations command closes two distinct cycles', async () => {
  await withScenario('two_iterations', async (scenario) => {
    const fixtures = await Promise.all(
      ['srgb.jpg', 'p3.jpg', 'rotated-6.jpg'].map((name) =>
        readFile(path.join(repositoryRoot, 'tests/fixtures/images', name)),
      ),
    )
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
        scenario.projectRoot,
        '--scenario',
        'two_iterations',
      ],
      { cwd: repositoryRoot, encoding: 'utf8', maxBuffer: 1024 * 1024 },
    )
    assert.equal(invocation.status, 0, invocation.stderr)
    const result = JSON.parse(invocation.stdout)

    assert.equal(result.archiveIds.length, 2)
    assert.notEqual(result.archiveIds[0], result.archiveIds[1])
    assert.equal(result.checks.twoDistinctCycles, true)
    assert.equal(result.checks.twoSourceReplacementsDetected, true)

    const current = await readCurrentReview({
      projectRoot: scenario.projectRoot,
      reviewStreamId: result.streamId,
    })
    const histories = await Promise.all(
      result.archiveIds.map((archiveId) =>
        readReviewHistory({
          projectRoot: scenario.projectRoot,
          reviewStreamId: result.streamId,
          archiveId,
        }),
      ),
    )
    assert.equal(current.snapshotRef.snapshotId, result.currentSnapshotId)
    assert.equal(current.feedback.some((feedback) => feedback.text === '第三版的新意见'), true)
    assert.deepEqual(
      histories.map((history) => history.role),
      ['history', 'history'],
    )
    assert.notEqual(
      histories[0].entries[0]?.snapshotRef.snapshotId,
      histories[1].entries[0]?.snapshotRef.snapshotId,
    )
    assert.deepEqual(await readFile(path.join(scenario.projectRoot, 'source-1.jpg')), fixtures[2])
    assert.deepEqual(
      await Promise.all(
        ['srgb.jpg', 'p3.jpg', 'rotated-6.jpg'].map((name) =>
          readFile(path.join(repositoryRoot, 'tests/fixtures/images', name)),
        ),
      ),
      fixtures,
    )
  })
})

test('archiving basis B after editing C preserves the complete later wording in current', async () => {
  await withScenario('two_iterations', async (scenario) => {
    const result = await scenario.run()
    const current = await readCurrentReview({
      projectRoot: scenario.projectRoot,
      reviewStreamId: result.streamId,
    })
    const history = await readReviewHistory({
      projectRoot: scenario.projectRoot,
      reviewStreamId: result.streamId,
      archiveId: result.archiveId,
    })

    assert.equal(result.scenario, 'two_iterations')
    assert.equal(current.snapshotRef.snapshotId, result.currentSnapshotId)
    assert.equal(history.role, 'history')
    assert.equal(
      current.feedback.find((item) => item.feedbackId === result.editedFeedbackId)?.text,
      '袖口收紧，保留褶皱',
    )
    const historicalFeedback = history.entries
      .flatMap((entry) => entry.feedback)
      .find((item) => item.feedbackId === result.editedFeedbackId)
    assert.equal(historicalFeedback?.text, '袖口收紧')
    assert.ok(current.needsConfirmation.length > 0)
    assert.equal(
      current.sourceChecks.some((check) => check.status === 'changed'),
      true,
    )
    assert.equal(JSON.stringify(result).includes(scenario.projectRoot), false)
    assert.equal(path.isAbsolute(result.fixture ?? ''), false)
    assert.equal(result.checks.currentPreservedLaterEdit, true)
    assert.equal(result.checks.historyPreservedBasisText, true)
    assert.equal(result.checks.sourceReplacementDetected, true)
    assert.equal(result.checks.originalFixtureUnchanged, true)
    assert.equal(result.cycleReads, 2)
    assert.equal(result.archiveIds.length, 2)
    assert.notEqual(result.archiveIds[0], result.archiveIds[1])
    const histories = await Promise.all(
      result.archiveIds.map((archiveId) =>
        readReviewHistory({
          projectRoot: scenario.projectRoot,
          reviewStreamId: result.streamId,
          archiveId,
        }),
      ),
    )
    assert.deepEqual(
      histories.map((entry) => entry.role),
      ['history', 'history'],
    )
    assert.notEqual(
      histories[0].entries[0]?.snapshotRef.snapshotId,
      histories[1].entries[0]?.snapshotRef.snapshotId,
    )
    assert.equal(current.feedback.some((item) => item.text === '第三版的新意见'), true)
    assert.equal(result.checks.twoDistinctCycles, true)
  })
})

test('partial archive removes only the returned target of shared feedback', async () => {
  await withScenario('partial_shared', async (scenario) => {
    const result = await scenario.run()
    const current = await readCurrentReview({
      projectRoot: scenario.projectRoot,
      reviewStreamId: result.streamId,
    })
    const history = await readReviewHistory({
      projectRoot: scenario.projectRoot,
      reviewStreamId: result.streamId,
      archiveId: result.archiveId,
    })
    const currentFeedback = current.feedback.find(
      (item) => item.feedbackId === result.sharedFeedbackId,
    )
    assert.deepEqual(
      currentFeedback?.targets.map((target) => target.targetId),
      [result.retainedTargetId],
    )
    assert.deepEqual(history.entries[0].selectedTargets.map((target) => target.targetId), [
      result.archivedTargetId,
    ])
    assert.equal(result.checks.partialSharedRetained, true)
  })
})

test('source replacement changes read-time checks without dropping saved wording', async () => {
  await withScenario('source_replaced', async (scenario) => {
    const result = await scenario.run()
    const current = await readCurrentReview({
      projectRoot: scenario.projectRoot,
      reviewStreamId: result.streamId,
    })
    assert.equal(current.snapshotRef.snapshotId, result.currentSnapshotId)
    assert.equal(current.feedback[0]?.text, '保留已保存的原始意见')
    assert.deepEqual(current.actionable, [])
    assert.equal(current.needsConfirmation.length, 1)
    assert.equal(current.sourceChecks[0]?.status, 'changed')
    assert.equal(result.checks.snapshotStayedFixed, true)
  })
})

test('manual archive without a declaration remains explicitly unknown', async () => {
  await withScenario('unknown_basis', async (scenario) => {
    const result = await scenario.run()
    const current = await readCurrentReview({
      projectRoot: scenario.projectRoot,
      reviewStreamId: result.streamId,
    })
    const history = await readReviewHistory({
      projectRoot: scenario.projectRoot,
      reviewStreamId: result.streamId,
      archiveId: result.archiveId,
    })
    assert.deepEqual(current.feedback, [])
    assert.equal(history.role, 'history')
    assert.equal(result.checks.unknownBasisPreserved, true)
    assert.equal(result.usageId, null)
  })
})

test('restore conflict requires an explicit choice and never overwrites later shared text', async () => {
  await withScenario('restore_conflict', async (scenario) => {
    const result = await scenario.run()
    const current = await readCurrentReview({
      projectRoot: scenario.projectRoot,
      reviewStreamId: result.streamId,
    })
    assert.equal(current.feedback[0]?.text, '图二的新意见，不可覆盖')
    assert.equal(result.checks.restoreConflictDetected, true)
    assert.equal(result.checks.currentTextUnchanged, true)
    assert.equal(current.snapshotRef.snapshotId, result.restoreReceiptSnapshotId)
    assert.equal(result.restoreDecision, 'continue_as_new')
    assert.equal(result.checks.restoreApplied, true)
    assert.equal(result.checks.staleRestoreRejected, true)
    const history = await readReviewHistory({
      projectRoot: scenario.projectRoot,
      reviewStreamId: result.streamId,
      archiveId: result.archiveId,
    })
    assert.equal(history.role, 'history')
    assert.equal(history.entries[0]?.feedback[0]?.text, '两张的旧意见')
  })
})

test('lost receipt retry returns the committed receipt without rolling back the current head', async () => {
  await withScenario('lost_receipt', async (scenario) => {
    const result = await scenario.run()
    const current = await readCurrentReview({
      projectRoot: scenario.projectRoot,
      reviewStreamId: result.streamId,
    })
    assert.equal(current.snapshotRef.snapshotId, result.currentSnapshotId)
    assert.equal(current.feedback[0]?.text, '回执丢失后的后续意见')
    assert.equal(result.checks.retriedReceiptStable, true)
    assert.equal(result.checks.currentDidNotRollBack, true)
  })
})

test('legacy migration keeps active Draft current and Completed records historical', async () => {
  await withScenario('migration', async (scenario) => {
    const result = await scenario.run()
    const current = await readCurrentReview({
      projectRoot: scenario.projectRoot,
      reviewStreamId: result.streamId,
    })
    const history = await readReviewHistory({
      projectRoot: scenario.projectRoot,
      reviewStreamId: result.streamId,
      legacyRoundId: result.legacyRoundId,
    })
    assert.equal(current.feedback[0]?.text, '人物手部需要修正，整体光线保持不变。')
    assert.equal(current.feedback[0]?.feedbackId, result.legacyDraftFeedbackId)
    assert.equal(history.role, 'history')
    assert.equal(history.entries[0]?.kind, 'legacy')
    assert.equal(history.entries[0]?.feedback[0]?.text, '人物手部需要修正，整体光线保持不变。')
    assert.equal(result.checks.legacyIndexBytesPreserved, true)
    assert.equal(result.checks.legacyRoundBytesPreserved, true)
    assert.equal(result.checks.legacyDraftBytesPreserved, true)
    assert.equal(result.checks.activeDraftBecameCurrent, true)
    assert.equal(result.checks.completedRemainedHistory, true)
  })
})
