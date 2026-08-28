import assert from 'node:assert/strict'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import test from 'node:test'
import { readReviewHistory } from './read-history.mjs'
import { readCurrentReview } from './read-current.mjs'
import { createProjectV3Case } from './v3-fixtures.mjs'
import { readLatestCompletedReview } from './read-latest.mjs'
import { validateFixture } from './schema-fixture-validation.mjs'
import { blake3Hex } from './blake3.mjs'

test('legacy history numbering is independent of feedback storage order', async t => {
  const f = await createProjectV3Case('legacy_mixed')
  t.after(f.cleanup)
  const base = path.join(f.projectRoot, '.viewer/reviews')
  const index = JSON.parse(await readFile(path.join(base, 'index.json')))
  const stream = index.streams.find(s => s.legacyRefs.some(r => r.protocolVersion === 'viewer.review/2'))
  const reference = stream.legacyRefs.find(r => r.protocolVersion === 'viewer.review/2')
  const file = path.join(base, reference.location)
  const record = JSON.parse(await readFile(file))
  record.feedback.reverse()
  const imageFeedback = record.feedback.find(f => f.targets.some(t => t.anchor.kind === 'imageRect'))
  const target = imageFeedback.targets.find(t => t.anchor.kind === 'imageRect')
  imageFeedback.targets.push({ ...target, anchor: { kind: 'imageRect', x: 0.7, y: 0.7, width: 0.1, height: 0.1 } })
  record.artifacts[0].annotations.reverse()
  for (const outcome of record.outcomes) {
    outcome.feedbackIds = record.feedback.filter(f => f.targets.some(t => t.assetVersionId === outcome.assetVersionId)).map(f => f.feedbackId)
  }
  const bytes = Buffer.from(JSON.stringify(record))
  reference.blake3 = blake3Hex(bytes)
  await writeFile(file, bytes)
  await writeFile(path.join(base, 'index.json'), JSON.stringify(index))
  const value = await readReviewHistory({ projectRoot: f.projectRoot, reviewStreamId: stream.reviewStreamId, legacyRoundId: reference.roundId })
  assert.equal(value.status, 'ok', value.message)
  assert.equal(value.entries[0].feedback[0].feedbackId, record.feedback[0].feedbackId)
  assert.equal(await validateFixture(value, schema), true)
})

const schema = JSON.parse(await readFile(new URL('../../docs/protocol/viewer-review-read-result-v3.schema.json', import.meta.url)))

test('explicit migrated v2 draft history reports absent evidence without inventing an annotated PNG', async t => {
  const f = await createProjectV3Case('legacy_mixed')
  t.after(f.cleanup)
  const base = path.join(f.projectRoot, '.viewer/reviews')
  const index = JSON.parse(await readFile(path.join(base, 'index.json')))
  const draft = JSON.parse(await readFile(new URL('../../tests/fixtures/review-protocol/review-draft-v2.valid.json', import.meta.url)))
  draft.reviewRoundId = '00000000-0000-4000-8000-000000000999'
  const bytes = Buffer.from(JSON.stringify(draft))
  const stream = index.streams.find(s => s.reviewStreamId === draft.reviewStreamId)
  const location = `drafts/${draft.reviewRoundId}.json`
  stream.legacyRefs.push({ kind: 'draft', roundId: draft.reviewRoundId, protocolVersion: draft.protocolVersion, location, blake3: blake3Hex(bytes) })
  await mkdir(path.join(base, 'drafts'), { recursive: true })
  await writeFile(path.join(base, location), bytes)
  await writeFile(path.join(base, 'index.json'), JSON.stringify(index))
  const result = await readReviewHistory({ projectRoot: f.projectRoot, reviewStreamId: stream.reviewStreamId, legacyRoundId: draft.reviewRoundId })
  assert.equal(result.status, 'ok', result.message)
  assert.deepEqual(result.entries[0].evidence, [])
  assert.ok(result.limitations.includes('legacyEvidenceAbsent'))
  assert.equal(await validateFixture(result, schema), true)
})
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
