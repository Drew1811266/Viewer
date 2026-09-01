import { useMemo } from 'react'
import type { ReviewHistorySelector, ReviewWorkspacePort } from '../../api/reviewWorkspaceTypes'
import type { AcceptanceSceneRegistry } from '../AcceptanceApp'
import { createAcceptanceReviewWorkspacePort } from '../acceptanceBridge'
import type { AcceptanceRequest } from '../acceptanceRequest'
import AcceptanceProductScene from './sceneHarness'

export const CONTINUOUS_REVIEW_ACCEPTANCE_SCENE_IDS = [
  'RVW-25',
  'RVW-26',
  'RVW-27',
  'RVW-28',
  'RVW-29',
  'RVW-30',
  'RVW-31',
  'RVW-32',
] as const

export const CONTINUOUS_REVIEW_SCENES: AcceptanceSceneRegistry = Object.fromEntries(
  CONTINUOUS_REVIEW_ACCEPTANCE_SCENE_IDS.map((id) => [
    id,
    (props: { request: AcceptanceRequest }) => <ContinuousReviewScene {...props} />,
  ]),
)

export function ContinuousReviewScene({
  request,
  reviewWorkspacePort,
}: {
  request: AcceptanceRequest
  reviewWorkspacePort?: ReviewWorkspacePort
}) {
  const fixturePort = useMemo(() => createAcceptanceReviewWorkspacePort(request.id), [request.id])
  const historySelector = historySelectorFor(request.id)
  return (
    <AcceptanceProductScene
      reviewWorkspacePort={reviewWorkspacePort ?? fixturePort}
      reviewWorkspacePresentation={historySelector === undefined ? undefined : { historySelector }}
      ready={() => continuousSceneReady(request.id)}
      attributes={{ 'data-continuous-review-state': request.id }}
    >
      {null}
    </AcceptanceProductScene>
  )
}

function historySelectorFor(id: string): ReviewHistorySelector | undefined {
  return id === 'RVW-28' || id === 'RVW-29' || id === 'RVW-32'
    ? { kind: 'archive', archiveId: 'acceptance-archive-b' }
    : undefined
}

export function continuousSceneReady(id: string): boolean {
  if (document.querySelector('.viewer-shell') === null) return false
  if (id === 'RVW-25') {
    if (document.querySelector('.image-preview .review-feedback-rail') !== null) {
      return document.body.textContent?.includes('袖口收紧，保留褶皱；新图继续独立评审。') === true
    }
    const a01 = document.querySelector<HTMLElement>('[role="treeitem"][aria-label="衣服/A01"]')
    if (a01 !== null && a01.dataset.acceptanceAction !== 'open-a01') {
      a01.dataset.acceptanceAction = 'open-a01'
      a01.click()
      return false
    }
    const image = document.querySelector<HTMLElement>('.image-cell')
    if (image !== null && image.dataset.acceptanceAction !== 'open-continuous-workbench') {
      image.dataset.acceptanceAction = 'open-continuous-workbench'
      image.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }))
    }
    return false
  }
  if (id === 'RVW-26' || id === 'RVW-27' || id === 'RVW-30') {
    if (
      document
        .querySelector('[role="dialog"][aria-labelledby]')
        ?.textContent?.includes('确认存档范围')
    ) {
      return true
    }
    clickOnce('存档意见', `open-${id.toLowerCase()}`)
    return false
  }
  if (id === 'RVW-28') {
    const evidence = document.querySelector<HTMLImageElement>('img[alt^="历史证据："]')
    if (evidence !== null) return true
    if (!clickOnce('查看历史证据', 'request-rvw-28-evidence')) {
      clickOnce('历史', 'open-rvw-28-history')
    }
    return false
  }
  if (id === 'RVW-29') {
    if (document.querySelector('[aria-label="已预览的恢复影响"]') !== null) return true
    if (document.body.textContent?.includes('图二的新意见，不可覆盖')) {
      if (!chooseOnce('使用历史意见', 'choose-rvw-29-history')) {
        clickOnce('查看恢复影响', 'preview-rvw-29-restore')
      }
      return false
    }
    if (!clickFirstUncheckedRestoreTarget()) clickOnce('历史', 'open-rvw-29-history')
    return false
  }
  if (id === 'RVW-32') {
    if (document.body.textContent?.includes('这批历史没有可恢复目标')) return true
    clickOnce('历史', 'open-rvw-32-history')
    return false
  }
  const expected = {
    'RVW-31': '迁移旧评审记录',
  }[id]
  return expected !== undefined && document.body.textContent?.includes(expected) === true
}

function clickOnce(label: string, marker: string): boolean {
  const button = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
    (candidate) => candidate.textContent?.trim() === label,
  )
  if (button === undefined || button.disabled) return false
  if (button.dataset.acceptanceAction !== marker) {
    button.dataset.acceptanceAction = marker
    button.click()
  }
  return true
}

function clickFirstUncheckedRestoreTarget(): boolean {
  const scope = document.querySelector<HTMLElement>('[aria-label="恢复范围"]')
  const checkbox = scope?.querySelector<HTMLInputElement>('input[type="checkbox"]:not(:checked)')
  if (checkbox === undefined || checkbox === null || checkbox.disabled) return false
  checkbox.click()
  return true
}

function chooseOnce(label: string, marker: string): boolean {
  const input = [...document.querySelectorAll<HTMLLabelElement>('label')]
    .find((candidate) => candidate.textContent?.trim() === label)
    ?.querySelector<HTMLInputElement>('input')
  if (input === undefined || input === null || input.checked || input.disabled) return false
  input.dataset.acceptanceAction = marker
  input.click()
  return true
}
