import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'
import { readCurrentReview, listCurrentReviewStreams } from './read-current.mjs'
import { createProjectV3Case } from './v3-fixtures.mjs'
import { blake3Hex } from './blake3.mjs'
import { validateFixture } from './schema-fixture-validation.mjs'

const schema = JSON.parse(await readFile(new URL('../../docs/protocol/viewer-review-read-result-v3.schema.json', import.meta.url)))
const id = n => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`
async function fixture(t, name = 'current_nonempty') {
  const value = await createProjectV3Case(name)
  t.after(value.cleanup)
  return { ...value, reviewStreamId: value.streamId }
}
async function editCurrent(f, mutate) {
  const base = path.join(f.projectRoot, '.viewer/reviews')
  const index = JSON.parse(await readFile(path.join(base, 'index.json')))
  const ref = index.streams[0].currentRef
  const file = path.join(base, `states/${ref.snapshotId}.json`)
  const value = JSON.parse(await readFile(file))
  mutate(value)
  const bytes = Buffer.from(JSON.stringify(value))
  ref.blake3 = blake3Hex(bytes)
  await writeFile(file, bytes)
  await writeFile(path.join(base, 'index.json'), JSON.stringify(index))
}

test('empty current remains a committed current snapshot and never returns archived instructions', async t => {
  const f = await fixture(t, 'current_empty')
  const value = await readCurrentReview(f)
  assert.equal(value.status, 'ok')
  assert.equal(value.role, 'current')
  assert.equal(value.snapshotRef.snapshotId, f.snapshotIds.at(-1))
  assert.deepEqual(value.feedback, [])
  assert.deepEqual(value.actionable, [])
  assert.deepEqual(value.needsConfirmation, [])
  assert.equal(Object.hasOwn(value, 'outcomes'), false)
  assert.equal(await validateFixture(value, schema), true)
})

test('current preserves exact original text and partitions every target by saved and live source status', async t => {
  for (const name of ['current_nonempty', 'pending_source', 'partial_archive', 'later_edit']) {
    const f = await fixture(t, name)
    const value = await readCurrentReview(f)
    assert.equal(value.status, 'ok', value.message)
    assert.equal(await validateFixture(value, schema), true)
    assert.equal(value.feedback[0].text, name === 'later_edit' ? '袖口收紧，保留原材质。' : '袖口收紧，保留褶皱。')
    const targets = value.feedback.flatMap(item => item.targets).map(item => item.targetId)
    assert.deepEqual([...value.actionable, ...value.needsConfirmation].sort(), targets.sort())
    if (name === 'pending_source') assert.deepEqual(value.actionable, [])
  }
})

test('source changes between reads change sourceChecks, never the snapshot or committed delta', async t => {
  const f = await fixture(t)
  const first = await readCurrentReview(f)
  await writeFile(path.join(f.projectRoot, 'image-a.png'), 'replacement')
  const second = await readCurrentReview({ ...f, sinceSnapshotId: first.snapshotRef.snapshotId })
  assert.deepEqual(second.snapshotRef, first.snapshotRef)
  assert.deepEqual(second.feedback, first.feedback)
  assert.equal(second.actionable.length, 1)
  assert.deepEqual(second.needsConfirmation, [id(411)])
  assert.equal(second.sourceChecks[0].status, 'changed')
  assert.deepEqual(second.delta.targets, [])
})

test('corrupt required evidence or state yields typed error, not a successful empty/pending result', async t => {
  const f = await fixture(t)
  const first = await readCurrentReview(f)
  const digest = first.evidence[0].capability.base.blake3
  await writeFile(path.join(f.projectRoot, `.viewer/reviews/evidence/${digest}.png`), 'corrupt')
  const value = await readCurrentReview(f)
  assert.equal(value.status, 'error')
  assert.equal(value.code, 'integrity')
  assert.equal(Object.hasOwn(value, 'actionable'), false)
  assert.equal(await validateFixture(value, schema), true)
})

test('rejects digest-valid illegal nested data and incomplete annotation/evidence relationships', async t => {
  for (const mutate of [
    state => { state.feedback[0].targets[0].availability.passed = true },
    state => { state.feedback[0].text = '图'.repeat(22000) },
    state => { state.feedback[0].targets.push(state.feedback[0].targets[0]) },
    state => { state.evidence = [] },
    state => { state.feedback[0].targets[0].anchor = { kind: 'imageRect', x: 0.5, y: 0, width: 0.8, height: 0.2 } },
    state => { state.parent = { snapshotId: state.snapshotId, blake3: 'f'.repeat(64) } },
  ]) {
    const f = await fixture(t)
    await editCurrent(f, mutate)
    assert.equal((await readCurrentReview(f)).status, 'error')
  }
})

test('missing index uses only the existing project identity and publishes no state', async t => {
  const projectRoot = await mkdtemp(path.join(tmpdir(), 'viewer-no-review-'))
  t.after(() => rm(projectRoot, { recursive: true, force: true }))
  await mkdir(path.join(projectRoot, '.viewer'))
  await writeFile(path.join(projectRoot, '.viewer/project.json'), JSON.stringify({ schemaVersion: 1, projectId: id(1), createdAtMs: 1 }))
  const value = await readCurrentReview({ projectRoot })
  assert.deepEqual(value, { protocolVersion: 'viewer.review/3', status: 'no_review_state', role: 'current', projectId: id(1), reviewStreamId: null })
  assert.equal(await validateFixture(value, schema), true)
  await mkdir(path.join(projectRoot, '.viewer/reviews/drafts'), { recursive: true })
  await writeFile(path.join(projectRoot, '.viewer/reviews/drafts/orphan.json'), '{}')
  const orphaned = await readCurrentReview({ projectRoot })
  assert.equal(orphaned.status, 'error', 'missing index with legacy draft is not an empty project')
  assert.equal(orphaned.code, 'integrity')
})

test('unknown since keeps complete current and reports unavailable; archive removal has a proven net reason', async t => {
  const f = await fixture(t, 'partial_archive')
  const unknown = await readCurrentReview({ ...f, sinceSnapshotId: id(999) })
  assert.equal(unknown.status, 'ok')
  assert.deepEqual(unknown.delta, { status: 'unavailable', reason: 'unknown_snapshot' })
  const known = await readCurrentReview({ ...f, sinceSnapshotId: f.snapshotIds[0] })
  assert.equal(known.status, 'ok', known.message)
  assert.equal(known.delta.status, 'available')
  assert.equal(known.delta.targets.length, 1)
  assert.equal(known.delta.targets[0].removalReason, 'archived')
  assert.equal(known.delta.targets[0].after, null)
  assert.equal(Object.hasOwn(known.delta.targets[0], 'text'), false)
})

test('stream selectors are exact and legacy/future protocols never revive Completed as current', async t => {
  const f = await fixture(t, 'legacy_mixed')
  assert.equal((await readCurrentReview({ projectRoot: f.projectRoot })).code, 'ambiguous_stream')
  assert.equal((await readCurrentReview({ ...f, taskId: 'x', batchId: 'y' })).status, 'error')
  assert.equal((await readCurrentReview({ ...f, reviewStreamId: id(999) })).code, 'unknown_stream')
  const list = await listCurrentReviewStreams({ projectRoot: f.projectRoot })
  assert.ok(list.streams.some(item => item.reviewStreamId === f.streamId))
  const file = path.join(f.projectRoot, '.viewer/reviews/index.json')
  await writeFile(file, JSON.stringify({ protocolVersion: 'viewer.review/2' }))
  assert.equal((await readCurrentReview(f)).code, 'migration_required')
  await writeFile(file, JSON.stringify({ protocolVersion: 'viewer.review/99' }))
  assert.equal((await readCurrentReview(f)).code, 'unsupported_version')
})

test('current CLI sends typed errors only to stderr and successful complete JSON to stdout', async t => {
  const f = await fixture(t)
  const script = new URL('./read-current.mjs', import.meta.url).pathname
  const ok = spawnSync(process.execPath, [script, '--project', f.projectRoot], { encoding: 'utf8' })
  assert.equal(ok.status, 0, ok.stderr)
  assert.equal(JSON.parse(ok.stdout).status, 'ok')
  assert.equal(ok.stderr, '')
  const bad = spawnSync(process.execPath, [script, '--project', f.projectRoot, '--stream', 'invalid'], { encoding: 'utf8' })
  assert.notEqual(bad.status, 0)
  assert.equal(bad.stdout, '')
  assert.equal(JSON.parse(bad.stderr).status, 'error')
})

test('unreachable or other-stream since never changes the selected current instruction set', async t => {
  const f = await fixture(t, 'legacy_mixed')
  const base = path.join(f.projectRoot, '.viewer/reviews')
  const index = JSON.parse(await readFile(path.join(base, 'index.json')))
  const other = index.streams.find(s => s.reviewStreamId !== f.streamId)
  const state = { protocolVersion: 'viewer.review/3', kind: 'state', projectId: index.projectId,
    reviewStreamId: other.reviewStreamId, snapshotId: id(880), parent: null, commandId: id(881), payloadDigest: '0'.repeat(64),
    assets: [], feedback: [], evidence: [], changes: [] }
  const bytes = Buffer.from(JSON.stringify(state))
  await writeFile(path.join(base, `states/${state.snapshotId}.json`), bytes)
  other.currentRef = { snapshotId: state.snapshotId, blake3: blake3Hex(bytes) }
  await writeFile(path.join(base, 'index.json'), JSON.stringify(index))
  const current = await readCurrentReview(f)
  const different = await readCurrentReview({ ...f, sinceSnapshotId: state.snapshotId })
  assert.equal(different.delta.reason, 'wrong_context')
  assert.deepEqual(different.feedback, current.feedback)
  const uncommitted = { ...state, reviewStreamId: f.streamId, snapshotId: id(882) }
  await writeFile(path.join(base, `states/${uncommitted.snapshotId}.json`), JSON.stringify(uncommitted))
  const result = await readCurrentReview({ ...f, sinceSnapshotId: uncommitted.snapshotId })
  assert.equal(result.delta.reason, 'unknown_snapshot')
  assert.deepEqual(result.snapshotRef, current.snapshotRef)
})

test('a damaged delta chain leaves verified current intact and never falls back to older prose', async t => {
  const f = await fixture(t, 'later_edit')
  const base = path.join(f.projectRoot, '.viewer/reviews')
  const current = await readCurrentReview(f)
  await writeFile(path.join(base, `states/${f.snapshotIds[0]}.json`), 'corrupt old snapshot')
  const result = await readCurrentReview({ ...f, sinceSnapshotId: f.snapshotIds[0] })
  assert.equal(result.status, 'ok', result.message)
  assert.deepEqual(result.feedback, current.feedback)
  assert.deepEqual(result.delta, { status: 'unavailable', reason: 'integrity' })
})

test('delta rejects an extra archival event that its committed checkpoint does not authorize', async t => {
  const f = await fixture(t, 'partial_archive')
  await editCurrent(f, state => {
    const feedback = state.feedback[0]
    const target = feedback.targets[0]
    const key = { feedbackId: feedback.feedbackId, textRevisionId: feedback.textRevisionId, targetId: target.targetId, targetRevisionId: target.targetRevisionId }
    state.changes.push({ targetId: target.targetId, before: key, after: null, kind: 'archived', archiveId: state.changes[0].archiveId, historicalKey: key })
    state.feedback = []
    for (const binding of state.evidence) {
      if (binding.capability.kind === 'image') { binding.capability.annotations = []; binding.capability.annotated = null }
    }
  })
  const result = await readCurrentReview({ ...f, sinceSnapshotId: f.snapshotIds[0] })
  assert.equal(result.status, 'ok', result.message)
  assert.deepEqual(result.delta, { status: 'unavailable', reason: 'integrity' })
})
