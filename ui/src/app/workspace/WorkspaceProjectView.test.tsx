import { describe, expect, it } from 'vitest'
import type { BrowserFile } from '../../api/types'
import { idleReviewSnapshot } from '../review/reviewModel'
import { resolveImageReviewRoute } from './WorkspaceProjectView'

const IMAGE_A: BrowserFile = {
  entityId: 'image-a',
  relativePath: 'batch/image-a.jpg',
  name: 'image-a.jpg',
  kind: 'jpeg',
  size: 100,
  modifiedNs: '1',
  marker: { reviewState: null, favorite: false },
  imageMetadata: { width: 640, height: 480 },
  imageUrl: null,
  videoMetadata: null,
}

describe('resolveImageReviewRoute', () => {
  it('uses the workbench for an idle writable scope and for members of an active fixed round', () => {
    const idle = idleReviewSnapshot()
    expect(
      resolveImageReviewRoute({
        snapshot: idle,
        file: IMAGE_A,
        capturedScope: { kind: 'selection', entityIds: ['image-a'] },
        projectAccess: 'read_write',
      }),
    ).toBe('workbench')

    expect(
      resolveImageReviewRoute({
        snapshot: {
          ...idle,
          phase: 'active',
          reviewRoundId: 'round-1',
          members: [
            {
              assetVersionId: 'asset-a',
              entityId: 'image-a',
              relativePath: IMAGE_A.relativePath,
              displayName: IMAGE_A.name,
              kind: 'image',
              feedbackItems: 0,
            },
          ],
        },
        file: IMAGE_A,
        capturedScope: { kind: 'selection', entityIds: ['different-selection'] },
        projectAccess: 'read_write',
      }),
    ).toBe('workbench')
  })

  it('keeps an active round fixed by routing a nonmember image to outside-scope preview', () => {
    expect(
      resolveImageReviewRoute({
        snapshot: {
          ...idleReviewSnapshot(),
          phase: 'active',
          reviewRoundId: 'round-1',
          members: [],
        },
        file: IMAGE_A,
        capturedScope: { kind: 'selection', entityIds: ['image-a'] },
        projectAccess: 'read_write',
      }),
    ).toBe('outside_scope')
  })

  it('retains ordinary preview when review is unavailable or the project is read-only', () => {
    for (const input of [
      { capturedScope: null, projectAccess: 'read_write' as const },
      {
        capturedScope: { kind: 'selection' as const, entityIds: ['image-a'] },
        projectAccess: 'read_only' as const,
      },
    ]) {
      expect(
        resolveImageReviewRoute({
          snapshot: idleReviewSnapshot(),
          file: IMAGE_A,
          ...input,
        }),
      ).toBe('ordinary')
    }
  })
})
