import assert from 'node:assert/strict'
import { readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import test from 'node:test'
import { readReviewHistory } from './read-history.mjs'
import { readCurrentReview } from './read-current.mjs'
import { createProjectV3Case } from './v3-fixtures.mjs'
import { readLatestCompletedReview } from './read-latest.mjs'
import { validateFixture } from './schema-fixture-validation.mjs'

const schema = JSON.parse(await readFile(new URL('../../docs/protocol/viewer-review-read-result-v3.schema.json', import.meta.url)))
test('history requires one explicit selector and never becomes a current instruction list', async t => {
  const f = await createProjectV3Case('later_edit')
  t.after(f.cleanup)
  const options = { projectRoot: f.projectRoot, reviewStreamId: f.streamId }
  for (const selector of [{}, { snapshotId: f.snapshotIds[0], archiveId: f.archiveId }]) {
    assert.equal((await readReviewHistory({ ...options, ...selector })).status, 'error')
  }
  const old = await readReviewHistory({ ...options, archiveId: f.archiveId })
  assert.equal(old.status, 'ok', old.message)
  assert.equal(old.role, 'history')
  assert.equal(old.entries[0].feedback[0].text, '袖口收紧，保留褶皱。')
  assert.equal(old.entries[0].selectedTargets.length, 1)
  assert.equal(Object.hasOwn(old, 'actionable'), false)
  assert.equal(await validateFixture(old, schema), true)
  const current = await readCurrentReview(options)
  assert.equal(current.feedback[0].text, '袖口收紧，保留原材质。')
  const snapshot = await readReviewHistory({ ...options, snapshotId: f.snapshotIds[0] })
  assert.equal(snapshot.entries[0].selectedTargets.length, 2)
  await assert.rejects(readLatestCompletedReview(options), /read-current/)
})

test('explicit legacy reads select the requested round, preserve identities and state evidence limitations', async t => {
  const f = await createProjectV3Case('legacy_mixed')
  t.after(f.cleanup)
  const index = JSON.parse(await readFile(path.join(f.projectRoot, '.viewer/reviews/index.json')))
  for (const stream of index.streams.filter(item => item.legacyRefs.length)) {
    for (const reference of stream.legacyRefs) {
      const result = await readReviewHistory({ projectRoot: f.projectRoot, reviewStreamId: stream.reviewStreamId, legacyRoundId: reference.roundId })
      assert.equal(result.status, 'ok', result.message)
      assert.equal(result.entries[0].reference.roundId, reference.roundId)
      assert.ok(result.limitations.includes('legacyUsageUnknown'))
      assert.ok(result.limitations.includes('legacyEvidenceAbsent'))
      assert.equal(Object.hasOwn(result, 'actionable'), false)
      assert.equal(await validateFixture(result, schema), true)
    }
  }
})

test('missing declared historical PNG is integrity failure, not legacy evidence absence', async t => {
  const f = await createProjectV3Case('current_empty')
  t.after(f.cleanup)
  const options = { projectRoot: f.projectRoot, reviewStreamId: f.streamId, archiveId: f.archiveId }
  const result = await readReviewHistory(options)
  assert.equal(result.status, 'ok', result.message)
  const digest = result.entries[0].evidence[0].capability.base.blake3
  await writeFile(path.join(f.projectRoot, `.viewer/reviews/evidence/${digest}.png`), 'bad')
  const invalid = await readReviewHistory(options)
  assert.equal(invalid.code, 'integrity')
  assert.equal(Object.hasOwn(invalid, 'entries'), false)
})
