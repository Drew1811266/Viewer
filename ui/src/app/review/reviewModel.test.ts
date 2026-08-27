import { describe, expect, it } from 'vitest'
import type { ReviewEditorState, ReviewSessionSnapshot } from '../../api/types'
import {
  deriveReviewScope,
  eligibleReviewTargetIds,
  hasUnsavedReviewText,
  idleReviewSnapshot,
  imageFeedbackForEntity,
  imageReviewReadOnlyReason,
} from './reviewModel'

describe('reviewModel', () => {
  it('uses creation time then identity for restored/resumed and equal-time image ordinals without mutating the snapshot', () => {
    const snapshot = idleReviewSnapshot()
    snapshot.feedback = [
      ['b', 20],
      ['z', 10],
      ['a', 20],
    ].map(([feedbackId, createdAtMs]) => ({
      feedbackId: String(feedbackId),
      createdAtMs: Number(createdAtMs),
      text: String(feedbackId),
      targetEntityIds: ['image-1'],
      targetCount: 1,
      targets: [
        {
          entityId: 'image-1',
          assetVersionId: 'asset-1',
          anchor: { kind: 'image_rect', x: 0.1, y: 0.1, width: 0.2, height: 0.2 },
        },
      ],
    }))
    expect(
      imageFeedbackForEntity(snapshot, 'image-1').map(({ feedbackId, ordinal }) => [
        feedbackId,
        ordinal,
      ]),
    ).toEqual([
      ['z', 1],
      ['a', 2],
      ['b', 3],
    ])
    expect(snapshot.feedback.map(({ feedbackId }) => feedbackId)).toEqual(['b', 'z', 'a'])
  })
  it('uses an explicit selection in folder and search contexts', () => {
    expect(
      deriveReviewScope({
        kind: 'folder',
        folderId: 'folder-1',
        includeDescendants: true,
        selectedEntityIds: ['image-1', 'image-1', 'video-1'],
      }),
    ).toEqual({ kind: 'selection', entityIds: ['image-1', 'video-1'] })
    expect(deriveReviewScope({ kind: 'search', selectedEntityIds: ['search-image-1'] })).toEqual({
      kind: 'selection',
      entityIds: ['search-image-1'],
    })
  })

  it('allows an implicit folder scope but never an implicit search-result scope', () => {
    expect(
      deriveReviewScope({
        kind: 'folder',
        folderId: null,
        includeDescendants: false,
        selectedEntityIds: [],
      }),
    ).toEqual({ kind: 'folder', folderId: null, includeDescendants: false })
    expect(deriveReviewScope({ kind: 'search', selectedEntityIds: [] })).toBeNull()
  })

  it('restricts feedback targets to fixed members while retaining selection order', () => {
    const snapshot = {
      members: [{ entityId: 'image-1' }, { entityId: null }, { entityId: 'video-1' }],
    } as ReviewSessionSnapshot

    expect(
      eligibleReviewTargetIds(['other-1', 'video-1', 'image-1', 'video-1'], snapshot.members),
    ).toEqual(['video-1', 'image-1'])
  })

  it('only guards nonempty text that differs from the saved editor value', () => {
    const editor = {
      text: '原意见',
      savedText: '原意见',
    } as ReviewEditorState
    expect(hasUnsavedReviewText(editor)).toBe(false)
    expect(hasUnsavedReviewText({ ...editor, text: '修改后' })).toBe(true)
    expect(hasUnsavedReviewText({ ...editor, text: '   ', savedText: '' })).toBe(false)
  })

  it('projects current-image feedback with ordinals only for visible local anchors', () => {
    const snapshot = {
      phase: 'active',
      reviewRoundId: 'round-1',
      members: [{ entityId: 'image-1' }],
      feedback: [
        {
          feedbackId: 'whole',
          text: '整图意见',
          createdAtMs: 1,
          targets: [{ entityId: 'image-1', anchor: { kind: 'asset' } }],
        },
        {
          feedbackId: 'local',
          text: '局部意见',
          createdAtMs: 2,
          targets: [
            {
              entityId: 'image-1',
              anchor: { kind: 'image_rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
            },
          ],
        },
      ],
    } as ReviewSessionSnapshot

    expect(imageFeedbackForEntity(snapshot, 'image-1')).toEqual([
      expect.objectContaining({ feedbackId: 'whole', ordinal: null }),
      expect.objectContaining({ feedbackId: 'local', ordinal: 1 }),
    ])
    expect(imageReviewReadOnlyReason(snapshot, 'image-1')).toBeNull()
    expect(imageReviewReadOnlyReason(snapshot, 'outside')).toBe('outside_scope')
  })
})
