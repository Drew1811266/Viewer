import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ReviewWorkspacePort } from '../../api/reviewWorkspaceTypes'
import { defined } from '../../defined'
import { createAcceptanceReviewWorkspacePort } from '../acceptanceBridge'
import type { AcceptanceRequest } from '../acceptanceRequest'
import { ACCEPTANCE_STATE_DEFINITIONS } from '../acceptanceStateCatalog'
import {
  CONTINUOUS_REVIEW_ACCEPTANCE_SCENE_IDS,
  CONTINUOUS_REVIEW_SCENES,
  ContinuousReviewScene,
  continuousSceneReady,
} from './continuousReviewScenes'
import { ACCEPTANCE_SCENES } from './index'
import AcceptanceProductScene from './sceneHarness'

const CONTINUOUS_REVIEW_IDS = [
  'RVW-25',
  'RVW-26',
  'RVW-27',
  'RVW-28',
  'RVW-29',
  'RVW-30',
  'RVW-31',
  'RVW-32',
] as const

describe('continuous review acceptance scenes', () => {
  it('registers the approved RVW-25 through RVW-32 acceptance matrix', () => {
    const catalogIds = ACCEPTANCE_STATE_DEFINITIONS.map(({ id }) => id)
    expect(
      catalogIds.filter((catalogId) =>
        CONTINUOUS_REVIEW_IDS.some((continuousReviewId) => continuousReviewId === catalogId),
      ),
    ).toEqual(CONTINUOUS_REVIEW_IDS)
    expect(CONTINUOUS_REVIEW_IDS.every((id) => ACCEPTANCE_SCENES[id] !== undefined)).toBe(true)
    expect(CONTINUOUS_REVIEW_ACCEPTANCE_SCENE_IDS).toEqual(CONTINUOUS_REVIEW_IDS)
    expect(Object.keys(CONTINUOUS_REVIEW_SCENES)).toEqual(CONTINUOUS_REVIEW_IDS)
  })

  it('provides deterministic typed states for replacement, migration, and empty-current history', async () => {
    const scope = { sessionId: 'acceptance-session', generation: 1 }
    const replacement = await createAcceptanceReviewWorkspacePort('RVW-28').getWorkspace(scope)
    const migration = await createAcceptanceReviewWorkspacePort('RVW-31').getWorkspace(scope)
    const empty = await createAcceptanceReviewWorkspacePort('RVW-32').getWorkspace(scope)

    expect(replacement.sourceChecks).toEqual([expect.objectContaining({ status: 'changed' })])
    expect(replacement.projection).toMatchObject({
      actionable: [],
      needsConfirmation: [expect.any(String)],
    })
    expect(migration.migration).toMatchObject({ legacyProtocol: 'viewer.review/1' })
    expect(empty.current?.state.feedback).toEqual([])
  })

  for (const id of ['RVW-26', 'RVW-27', 'RVW-30'] as const) {
    it(`${id} opens the archive state through the real App action`, async () => {
      const calls: string[] = []
      const port = recordingPort(id, calls)
      render(<ContinuousReviewScene request={request(id)} reviewWorkspacePort={port} />)

      expect(await screen.findByRole('dialog', { name: '确认存档范围' })).toBeVisible()
      expect(calls).toContain('previewArchive')
      expect(document.querySelector('.viewer-shell')).not.toBeNull()
    })
  }

  for (const id of ['RVW-29', 'RVW-32'] as const) {
    it(`${id} opens history through the real App action`, async () => {
      const calls: string[] = []
      render(
        <ContinuousReviewScene
          request={request(id)}
          reviewWorkspacePort={recordingPort(id, calls)}
        />,
      )

      expect(await screen.findByRole('dialog', { name: '历史意见' })).toBeVisible()
      expect(calls).toContain('getHistory')
      if (id === 'RVW-32') expect(screen.getByText('这批历史没有可恢复目标。')).toBeVisible()
      else {
        expect(await screen.findByText(/当前原文：图二的新意见，不可覆盖/)).toBeVisible()
        expect(await screen.findByRole('region', { name: '已预览的恢复影响' })).toBeVisible()
        expect(calls).toContain('previewRestore')
      }
    })
  }

  it('RVW-28 requests and visibly renders historical evidence through the real App', async () => {
    const calls: string[] = []
    render(
      <ContinuousReviewScene
        request={request('RVW-28')}
        reviewWorkspacePort={recordingPort('RVW-28', calls)}
      />,
    )

    expect(await screen.findByRole('dialog', { name: '历史意见' })).toBeVisible()
    expect(await screen.findByRole('img', { name: /历史证据：袖口收紧/ })).toBeVisible()
    expect(calls).toContain('getHistory')
    expect(calls).toContain('getEvidence')
  })

  it('recognizes the real RVW-25 workbench DOM as ready', () => {
    document.body.innerHTML = `
      <main class="viewer-shell">
        <section class="image-preview">
          <img class="image-preview-image" alt="商品-01.jpg" />
          <aside class="review-feedback-rail">袖口收紧，保留褶皱；新图继续独立评审。</aside>
        </section>
      </main>
    `

    expect(continuousSceneReady('RVW-25')).toBe(true)
  })

  it('lets the real continuous layer render legacy migration from the port view', async () => {
    const Scene = defined(CONTINUOUS_REVIEW_SCENES['RVW-31'], 'Missing RVW-31 scene')
    render(<Scene request={request('RVW-31')} />)

    expect(await screen.findByRole('dialog', { name: '迁移旧评审记录' })).toBeVisible()
    expect(screen.getByText('旧记录只作为历史保留')).toBeVisible()
  })

  it('does not mount an acceptance sibling over the real App before an action opens it', () => {
    const Scene = defined(CONTINUOUS_REVIEW_SCENES['RVW-26'], 'Missing RVW-26 scene')
    render(<Scene request={request('RVW-26')} />)

    expect(screen.queryByRole('dialog', { name: '确认存档范围' })).toBeNull()
  })

  it('keeps history disabled when the typed presentation selector is absent', async () => {
    render(
      <AcceptanceProductScene
        ready={() => false}
        reviewWorkspacePort={createAcceptanceReviewWorkspacePort('RVW-29')}
      >
        {null}
      </AcceptanceProductScene>,
    )

    expect(await screen.findByRole('button', { name: '历史' })).toBeDisabled()
  })
})

function request(id: string): AcceptanceRequest {
  return { id, viewport: '1024x720', width: 1024, height: 720 }
}

function recordingPort(id: string, calls: string[]): ReviewWorkspacePort {
  const port = createAcceptanceReviewWorkspacePort(id)
  return {
    ...port,
    async previewArchive(request) {
      calls.push('previewArchive')
      return port.previewArchive(request)
    },
    async getHistory(request) {
      calls.push('getHistory')
      return port.getHistory(request)
    },
    async previewRestore(request) {
      calls.push('previewRestore')
      return port.previewRestore(request)
    },
    async getEvidence(request) {
      calls.push('getEvidence')
      return port.getEvidence(request)
    },
  }
}
