import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { defined } from '../../defined'
import type { AcceptanceRequest } from '../acceptanceRequest'
import { ACCEPTANCE_STATE_DEFINITIONS } from '../acceptanceStateCatalog'
import {
  REVIEW_ACCEPTANCE_SCENE_IDS,
  REVIEW_SCENES,
  reviewAcceptanceBridgeOverrides,
} from './reviewScenes'

const request: AcceptanceRequest = {
  id: 'RVW-08',
  viewport: '1024x720',
  width: 1024,
  height: 720,
}

describe('review acceptance scenes', () => {
  it('registers every review catalog state exactly once and in approved order', () => {
    const catalogIds = ACCEPTANCE_STATE_DEFINITIONS.filter(
      ({ sceneGroup }) => sceneGroup === 'review',
    ).map(({ id }) => id)

    expect(REVIEW_ACCEPTANCE_SCENE_IDS).toEqual(catalogIds)
    expect(Object.keys(REVIEW_SCENES)).toEqual(catalogIds)
  })

  it('drives the real App tree through bridge fixtures instead of mounting review components', async () => {
    const source = readFileSync(resolve(import.meta.dirname, 'reviewScenes.tsx'), 'utf8')
    expect(source).toContain('<AcceptanceProductScene')
    expect(source).not.toMatch(/import .*Review(?:StartDialog|Inspector|CompletionDialog)/)
    expect(source).not.toMatch(/\.json['"]/)

    const Scene = defined(REVIEW_SCENES['RVW-08'], 'Missing RVW-08 acceptance scene')
    render(<Scene request={request} />)

    expect(await screen.findByText('评审暂时不可写')).toBeVisible()
    await waitFor(() => expect(document.querySelector('.viewer-shell')).not.toBeNull())
    expect(screen.getByText(/普通浏览和预览仍可继续。/)).toBeVisible()
  })

  it('keeps grid review and optional full preview on the same completion eligibility', async () => {
    const gridBridge = reviewAcceptanceBridgeOverrides('RVW-03')
    const previewBridge = reviewAcceptanceBridgeOverrides('RVW-04')
    const guard = {
      sessionId: 'acceptance-session',
      generation: 1,
      reviewRoundId: 'acceptance-review-round',
      expectedRevision: 4,
    }
    const gridStatus = await defined(
      gridBridge.reviewStatus,
      'Missing grid review status',
    )({
      sessionId: guard.sessionId,
      generation: guard.generation,
    })
    const previewStatus = await defined(
      previewBridge.reviewStatus,
      'Missing preview review status',
    )({
      sessionId: guard.sessionId,
      generation: guard.generation,
    })
    const gridCompletion = await defined(
      gridBridge.reviewCompletionSummary,
      'Missing grid completion summary',
    )(guard)
    const previewCompletion = await defined(
      previewBridge.reviewCompletionSummary,
      'Missing preview completion summary',
    )(guard)

    expect(previewStatus.counts).toEqual(gridStatus.counts)
    expect(previewCompletion.summary).toEqual(gridCompletion.summary)
    expect(previewCompletion.summary.canComplete).toBe(true)
    expect(JSON.stringify(previewCompletion)).not.toMatch(/viewedCount|viewed_count|浏览进度/)
  })

  it('resolves selection and aggregate-folder proposals from explicit deterministic scopes', async () => {
    const selectionBridge = reviewAcceptanceBridgeOverrides('RVW-01')
    const folderBridge = reviewAcceptanceBridgeOverrides('RVW-02')
    const context = { sessionId: 'acceptance-session', generation: 1 }
    const selection = await defined(
      selectionBridge.reviewPreviewStart,
      'Missing selection preview',
    )({ ...context, scope: { kind: 'selection', entityIds: ['acceptance-image-01'] } })
    const aggregate = await defined(
      folderBridge.reviewPreviewStart,
      'Missing folder preview',
    )({
      ...context,
      scope: { kind: 'folder', folderId: null, includeDescendants: true },
    })

    expect(selection.resolution).toEqual({
      candidateCount: 1,
      imageCount: 1,
      videoCount: 0,
      excludedCount: 0,
    })
    expect(aggregate.resolution).toEqual({
      candidateCount: 30,
      imageCount: 30,
      videoCount: 0,
      excludedCount: 0,
    })
  })

  it('renders conflict paths as project-relative data and blocks completion', async () => {
    const bridge = reviewAcceptanceBridgeOverrides('RVW-11')
    const proposal = await defined(
      bridge.reviewCompletionSummary,
      'Missing conflict completion summary',
    )({
      sessionId: 'acceptance-session',
      generation: 1,
      reviewRoundId: 'acceptance-review-round',
      expectedRevision: 4,
    })

    expect(proposal.summary.canComplete).toBe(false)
    expect(proposal.summary.conflicts).not.toHaveLength(0)
    for (const conflict of proposal.summary.conflicts) {
      expect(conflict.relativePath.startsWith('/')).toBe(false)
      expect(conflict.relativePath).not.toContain('ViewerAcceptance')
    }
  })
})
