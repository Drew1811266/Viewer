import { expect, it } from 'vitest'
import {
  continuousReviewState,
  emptyContinuousEditor,
  normalizeReviewWorkspaceError,
} from './continuousReviewModel'
import { failure, workspace } from './continuousReviewTestFixtures'

it('treats an absent or empty current state as ready without inventing historical feedback', () => {
  expect(continuousReviewState(workspace())).toEqual({ kind: 'ready' })
  expect(continuousReviewState(workspace('empty-current'))).toEqual({ kind: 'ready' })
  expect(emptyContinuousEditor().text).toBe('')
})

it('does not trust a malformed committed receipt or a partial framework error as a typed write outcome', () => {
  expect(
    normalizeReviewWorkspaceError({ ...failure('io'), committedReceipt: 'invalid' }),
  ).toMatchObject({ code: 'internal', committedReceipt: null })
  expect(
    normalizeReviewWorkspaceError({ code: 'io', message: 'broken transport', retryable: true }),
  ).toMatchObject({ code: 'internal', message: 'broken transport', committedReceipt: null })
})

it('preserves a complete typed receipt even if the associated code is stale_session', () => {
  const receipt = {
    commandId: 'command-1',
    payloadDigest: 'ab'.repeat(32),
    snapshot: { snapshotId: 'committed', blake3: 'cd'.repeat(32) },
  }
  expect(
    normalizeReviewWorkspaceError({
      ...failure('stale_session', false),
      committedReceipt: receipt,
    }),
  ).toEqual({ ...failure('stale_session', false), committedReceipt: receipt })
})
