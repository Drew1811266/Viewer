import { describe, expect, it } from 'vitest'
import {
  compareEntryAvailability,
  compareValidationMessage,
  MAX_COMPARE_IMAGES,
  MIN_COMPARE_IMAGES,
  validateCompareCandidates,
} from './comparePolicy'

const image = (index: number) => ({ entityId: `image-${index}`, kind: 'jpeg' })

describe('comparePolicy', () => {
  it('classifies compare entry availability without changing validation copy', () => {
    expect(
      compareEntryAvailability({
        workspace: { workspace: 'content', images: [], videos: [], otherFiles: [] },
        searchResultsOpen: false,
        operationBusy: false,
      }),
    ).toBe('available')
    expect(
      compareEntryAvailability({
        workspace: { workspace: 'content', images: [], videos: [], otherFiles: [] },
        searchResultsOpen: false,
        operationBusy: true,
      }),
    ).toBe('busy')
    expect(
      compareEntryAvailability({
        workspace: { workspace: 'empty' },
        searchResultsOpen: false,
        operationBusy: false,
      }),
    ).toBe('folder-context-required')
    expect(
      compareEntryAvailability({
        workspace: { workspace: 'content', images: [], videos: [], otherFiles: [] },
        searchResultsOpen: true,
        operationBusy: false,
      }),
    ).toBe('folder-context-required')
  })

  it('accepts exactly 2 through 8 supported unique images', () => {
    expect(MIN_COMPARE_IMAGES).toBe(2)
    expect(MAX_COMPARE_IMAGES).toBe(8)
    expect(validateCompareCandidates([image(1), image(2)])).toEqual({ ok: true })
    expect(
      validateCompareCandidates(Array.from({ length: 8 }, (_, index) => image(index))),
    ).toEqual({ ok: true })
    expect(
      validateCompareCandidates([
        { entityId: 'jpg', kind: 'jpeg' },
        { entityId: 'raw', kind: 'unsupported_image' },
      ]),
    ).toEqual({ ok: true })
  })

  it('rejects invalid cardinality without truncating', () => {
    expect(validateCompareCandidates([image(1)])).toEqual({
      ok: false,
      reason: 'invalid_cardinality',
    })
    expect(
      validateCompareCandidates(Array.from({ length: 9 }, (_, index) => image(index))),
    ).toEqual({ ok: false, reason: 'invalid_cardinality' })
    expect(compareValidationMessage('invalid_cardinality')).toBe('请选择 2–8 张图片进行对比。')
  })

  it('rejects duplicates and unsupported kinds', () => {
    expect(validateCompareCandidates([image(1), image(1)])).toEqual({
      ok: false,
      reason: 'duplicate_entity',
    })
    expect(validateCompareCandidates([image(1), { entityId: 'note', kind: 'text' }])).toEqual({
      ok: false,
      reason: 'unsupported_type',
    })
  })
})
