import { describe, expect, it } from 'vitest'
import type { ReviewEditorState, ReviewSessionSnapshot } from '../../api/types'
import { deriveReviewScope, eligibleReviewTargetIds, hasUnsavedReviewText } from './reviewModel'

describe('reviewModel', () => {
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
})
