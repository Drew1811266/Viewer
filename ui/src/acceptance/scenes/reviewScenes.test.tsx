import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { defined } from '../../defined'
import { ACCEPTANCE_FILES } from '../acceptanceFixtures'
import type { AcceptanceRequest } from '../acceptanceRequest'
import { ACCEPTANCE_STATE_DEFINITIONS } from '../acceptanceStateCatalog'
import {
  REVIEW_ACCEPTANCE_SCENE_IDS,
  REVIEW_SCENES,
  reviewAcceptanceBridgeOverrides,
} from './reviewScenes'
import { REVIEW_WORKBENCH_SCENES, workbenchToolbarFits } from './reviewWorkbenchScenes'

const request: AcceptanceRequest = {
  id: 'RVW-08',
  viewport: '1024x720',
  width: 1024,
  height: 720,
}

describe('review acceptance scenes', () => {
  it('rejects overlapping or clipped toolbar buttons even when image data is ready', () => {
    const { container } = render(
      <section className="image-preview">
        <header className="viewer-toolbar">
          <button type="button">缩放</button>
          <button type="button">画笔</button>
        </header>
      </section>,
    )
    const header = container.querySelector('header') as HTMLElement
    const buttons = container.querySelectorAll('button')
    header.getBoundingClientRect = () => new DOMRect(0, 0, 1000, 50)
    const first = defined(buttons[0], 'zoom')
    const second = defined(buttons[1], 'brush')
    first.getBoundingClientRect = () => new DOMRect(400, 10, 100, 30)
    second.getBoundingClientRect = () => new DOMRect(420, 10, 100, 30)
    expect(workbenchToolbarFits()).toBe(false)
    second.getBoundingClientRect = () => new DOMRect(520, 10, 100, 30)
    expect(workbenchToolbarFits()).toBe(true)
    second.getBoundingClientRect = () => new DOMRect(950, 10, 100, 30)
    expect(workbenchToolbarFits()).toBe(false)
  })
  it('registers the complete semantic workbench recipes that never settle on empty markup', () => {
    expect(Object.keys(REVIEW_WORKBENCH_SCENES)).toEqual([
      'RVW-16',
      'RVW-17',
      'RVW-18',
      'RVW-19',
      'RVW-20',
      'RVW-21',
      'RVW-22',
      'RVW-23',
      'RVW-24',
      'RVW-33',
      'RVW-34',
      'RVW-35',
      'RVW-36',
      'RVW-37',
    ])
    for (const [id, recipe] of Object.entries(REVIEW_WORKBENCH_SCENES)) {
      expect(REVIEW_SCENES[id]).toBeDefined()
      expect(recipe()).toBe(false)
    }
  })

  it('seeds four stable local annotations with the original Chinese text on one image', async () => {
    const bridge = reviewAcceptanceBridgeOverrides('RVW-17')
    const status = await defined(
      bridge.reviewStatus,
      'status',
    )({
      sessionId: 'acceptance-session',
      generation: 1,
    })
    expect(status.feedback.map(({ text }) => text)).toEqual([
      '领口需要收窄，保留原有材质。',
      '右袖边缘有伪影，请重画这一段。',
      '左侧接缝需要拉直。',
      '裤脚颜色请与衣身统一。',
    ])
    expect(status.feedback.map(({ feedbackId }) => feedbackId)).toEqual([
      'acceptance-annotation-1',
      'acceptance-annotation-2',
      'acceptance-annotation-3',
      'acceptance-annotation-4',
    ])
    expect(status.feedback.map(({ targets }) => targets[0]?.anchor.kind)).toEqual([
      'image_rect',
      'image_stroke',
      'image_point',
      'image_arrow',
    ])
    expect(status.members[0]?.feedbackItems).toBe(4)
    expect(status.counts).toMatchObject({ feedbackItems: 4, revise: 1 })
  })

  it('starts with no draft and resolves the first anchored write without pending network work', async () => {
    const bridge = reviewAcceptanceBridgeOverrides('RVW-16')
    const context = { sessionId: 'acceptance-session', generation: 1 }
    expect(await defined(bridge.reviewStatus, 'status')(context)).toMatchObject({
      phase: 'idle',
      reviewRoundId: null,
      feedback: [],
    })
    const proposal = await defined(
      bridge.reviewPreviewStart,
      'scope proposal',
    )({
      ...context,
      scope: { kind: 'folder', folderId: 'acceptance-folder-a01', includeDescendants: false },
    })
    const result = await defined(
      bridge.reviewStartWithFeedback,
      'first anchored write',
    )({
      ...context,
      proposalId: 41,
      text: '领口需要收窄，保留原有材质。',
      targets: [
        {
          entityId: 'acceptance-image-01',
          anchor: {
            kind: 'image_rect',
            x: 0.4,
            y: 0.1,
            width: 0.2,
            height: 0.15,
          },
        },
      ],
    })
    expect(result.phase).toBe('active')
    expect(result.feedback).toHaveLength(1)
    expect(result.counts.total).toBe(proposal.resolution.candidateCount)
    expect(result.members.map((member) => member.entityId)).toEqual(
      ACCEPTANCE_FILES.map((file) => file.entityId),
    )
    expect(await defined(bridge.reviewStatus, 'status')(context)).toEqual(result)
  })

  it('keeps failed annotation writes local and preserves the saved four opinions', async () => {
    const bridge = reviewAcceptanceBridgeOverrides('RVW-18')
    await expect(
      defined(
        bridge.reviewAddFeedback,
        'anchored write',
      )({
        sessionId: 'acceptance-session',
        generation: 1,
        reviewRoundId: 'acceptance-review-round',
        expectedRevision: 4,
        text: '这处标注尚未保存。',
        targets: [
          {
            entityId: 'acceptance-image-01',
            anchor: { kind: 'image_rect', x: 0.1, y: 0.1, width: 0.2, height: 0.2 },
          },
        ],
      }),
    ).rejects.toMatchObject({ code: 'review_save_failed' })
    expect(
      (
        await defined(
          bridge.reviewStatus,
          'status',
        )({
          sessionId: 'acceptance-session',
          generation: 1,
        })
      ).feedback,
    ).toHaveLength(4)
  })

  it('registers every review catalog state exactly once and in approved order', () => {
    const catalogIds = ACCEPTANCE_STATE_DEFINITIONS.filter(
      ({ sceneGroup, id }) =>
        sceneGroup === 'review' &&
        (Number(id.slice(4)) <= 24 || (Number(id.slice(4)) >= 33 && Number(id.slice(4)) <= 37)),
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
