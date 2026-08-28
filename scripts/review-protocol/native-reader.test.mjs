import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'
import { invokeReader, readerExecutable } from './native-reader.mjs'
import { readCurrentReview } from './read-current.mjs'

const projectRoot = fileURLToPath(new URL('../../tests/fixtures/review-protocol/project', import.meta.url))
const reviewStreamId = '00000000-0000-4000-8000-000000000102'

test('native legacy boundary returns the original validated JSON and keeps errors typed', async () => {
  const round = await invokeReader('legacyLatest', { projectRoot, reviewStreamId })
  assert.equal(round.reviewRoundId, '00000000-0000-4000-8000-000000000202')
  assert.equal(round.status, 'completed')
  await assert.rejects(invokeReader('legacyLatest', { projectRoot }), { code: 'ambiguous_stream' })
  await assert.rejects(invokeReader('legacyLatest', { projectRoot, reviewStreamId, unexpected: 1 }), { code: 'integrity' })
})

test('binary selection is explicit and never supplied by the reviewed project', () => {
  assert.equal(readerExecutable({}), fileURLToPath(new URL('../../target/debug/viewer-review-reader', import.meta.url)))
  assert.equal(readerExecutable({ VIEWER_REVIEW_READER: '/trusted/reader' }), '/trusted/reader')
  assert.throws(() => readerExecutable({ VIEWER_REVIEW_READER: './reader' }), /absolute/)
})

test('missing native core is an explicit error, with no automatic build or fallback', () => {
  const script = fileURLToPath(new URL('./read-current.mjs', import.meta.url))
  const result = spawnSync(process.execPath, [script, '--project', projectRoot], {
    env: { ...process.env, VIEWER_REVIEW_READER: '/nonexistent-viewer-review-reader/core' }, encoding: 'utf8',
  })
  assert.equal(result.status, 1)
  assert.equal(result.stdout, '')
  assert.equal(JSON.parse(result.stderr).code, 'io')
  assert.match(JSON.parse(result.stderr).message, /build:review-reader/)
})

test('public current reader cancellation emits no partial successful instruction set', async () => {
  const controller = new AbortController()
  controller.abort()
  const result = await readCurrentReview({ projectRoot }, { signal: controller.signal })
  assert.equal(result.status, 'error')
  assert.equal(result.code, 'io')
  assert.match(result.message, /cancelled/)
  assert.equal(Object.hasOwn(result, 'actionable'), false)
})

test('native stdin is a closed bounded request and failures have no successful result', () => {
  for (const input of [
    JSON.stringify({ protocolVersion: 'viewer.review.reader/2', operation: 'legacyList', projectRoot }),
    JSON.stringify({ protocolVersion: 'viewer.review.reader/1', operation: 'legacyList', projectRoot, path: '../outside' }),
    ' '.repeat(64 * 1024 + 1),
  ]) {
    const result = spawnSync(readerExecutable(), [], { input, encoding: 'utf8', maxBuffer: 1024 * 1024 })
    assert.equal(result.status, 1)
    const response = JSON.parse(result.stdout)
    assert.equal(response.protocolVersion, 'viewer.review.reader/1')
    assert.ok(['integrity', 'unsupported_version', 'limit_exceeded'].includes(response.error.code))
    assert.equal(Object.hasOwn(response, 'result'), false)
    assert.equal(result.stderr, '')
  }
  assert.ok(path.isAbsolute(readerExecutable()))
})
