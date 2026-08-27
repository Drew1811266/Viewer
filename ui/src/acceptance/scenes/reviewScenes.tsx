import { useMemo } from 'react'
import type { ViewerCommandError } from '../../api/types'
import type { AcceptanceSceneRegistry } from '../AcceptanceApp'
import type { AcceptanceBridgeOverrides } from '../acceptanceBridge'
import {
  ACCEPTANCE_FILES,
  ACCEPTANCE_PROJECT_SNAPSHOT,
  ACCEPTANCE_REVIEW_CONFLICTS,
  acceptanceReviewCompletion,
  acceptanceReviewSnapshot,
} from '../acceptanceFixtures'
import type { AcceptanceRequest } from '../acceptanceRequest'
import { REVIEW_WORKBENCH_SCENES, workbenchBridgeOverrides } from './reviewWorkbenchScenes'
import AcceptanceProductScene from './sceneHarness'

export const REVIEW_ACCEPTANCE_SCENE_IDS = [
  'RVW-01',
  'RVW-02',
  'RVW-03',
  'RVW-04',
  'RVW-05',
  'RVW-06',
  'RVW-07',
  'RVW-08',
  'RVW-09',
  'RVW-10',
  'RVW-11',
  'RVW-12',
  'RVW-13',
  'RVW-14',
  'RVW-15',
  'RVW-16',
  'RVW-17',
  'RVW-18',
  'RVW-19',
  'RVW-20',
  'RVW-21',
  'RVW-22',
  'RVW-23',
  'RVW-24',
] as const

export const REVIEW_SCENES: AcceptanceSceneRegistry = Object.fromEntries(
  REVIEW_ACCEPTANCE_SCENE_IDS.map((id) => [
    id,
    (props: { request: AcceptanceRequest }) => <ReviewScene {...props} />,
  ]),
)

function ReviewScene({ request }: { request: AcceptanceRequest }) {
  const bridgeOverrides = useMemo(() => reviewAcceptanceBridgeOverrides(request.id), [request.id])
  const ready = useMemo(() => reviewAcceptanceRecipe(request.id), [request.id])
  return (
    <AcceptanceProductScene
      bridgeOverrides={bridgeOverrides}
      ready={ready}
      attributes={{ 'data-review-acceptance-state': request.id }}
    >
      {null}
    </AcceptanceProductScene>
  )
}

export function reviewAcceptanceBridgeOverrides(id: string): AcceptanceBridgeOverrides {
  const snapshot = snapshotForState(id)
  const overrides: AcceptanceBridgeOverrides = {
    async reviewStatus() {
      return snapshot
    },
    async reviewPreviewStart({ scope }) {
      const selectedCount = scope.kind === 'selection' ? scope.entityIds.length : 30
      return {
        proposalId: id === 'RVW-02' ? 42 : 41,
        resolution: {
          candidateCount: selectedCount,
          imageCount: selectedCount,
          videoCount: 0,
          excludedCount: 0,
        },
      }
    },
    async reviewResume() {
      return acceptanceReviewSnapshot()
    },
    async reviewCompletionSummary() {
      return acceptanceReviewCompletion(id === 'RVW-11' ? ACCEPTANCE_REVIEW_CONFLICTS : [])
    },
  }
  if (id === 'RVW-06') {
    overrides.reviewAddFeedback = async () => {
      throw {
        code: 'review_save_failed',
        category: 'environment',
        userMessage: '意见尚未保存，请重试。',
        retryable: true,
        taskId: null,
        itemId: null,
      } satisfies ViewerCommandError
    }
  }
  if (id === 'RVW-09') {
    overrides.openProject = async () => ({ ...ACCEPTANCE_PROJECT_SNAPSHOT, access: 'read_only' })
  }
  return { ...overrides, ...workbenchBridgeOverrides(id) }
}

function snapshotForState(id: string) {
  if (id === 'RVW-01' || id === 'RVW-02' || id === 'RVW-09') {
    return acceptanceReviewSnapshot({
      phase: 'idle',
      reviewStreamId: null,
      reviewRoundId: null,
      revision: 0,
      members: [],
      feedback: [],
      unreviewable: [],
      counts: { total: 0, feedbackItems: 0, revise: 0, unreviewable: 0, pass: 0 },
    })
  }
  if (id === 'RVW-07') {
    return acceptanceReviewSnapshot({
      phase: 'idle',
      resume: {
        reviewStreamId: 'acceptance-review-stream',
        reviewRoundId: 'acceptance-review-round',
        createdAtMs: 1_767_600_000_000,
        total: 6,
        feedbackItems: 1,
      },
      reviewStreamId: null,
      reviewRoundId: null,
      revision: 0,
      members: [],
      feedback: [],
      unreviewable: [],
      counts: { total: 0, feedbackItems: 0, revise: 0, unreviewable: 0, pass: 0 },
    })
  }
  if (id === 'RVW-08') {
    return acceptanceReviewSnapshot({
      phase: 'write_unavailable',
      error: { code: 'review_busy', retryable: true, affectedPaths: [] },
    })
  }
  if (id === 'RVW-12') {
    return acceptanceReviewSnapshot({
      phase: 'recovery_required',
      error: {
        code: 'review_invalid_data',
        retryable: false,
        affectedPaths: ['.viewer/review/v1/stream.jsonl'],
      },
    })
  }
  if (id === 'RVW-13') {
    return acceptanceReviewSnapshot({
      phase: 'completed_read_only',
      counts: { total: 6, feedbackItems: 1, revise: 2, unreviewable: 1, pass: 3 },
    })
  }
  return acceptanceReviewSnapshot()
}

function reviewAcceptanceRecipe(id: string): () => boolean {
  return () => {
    if (id === 'RVW-07') return hasText('发现未完成的评审')
    if (id === 'RVW-08') return hasText('评审暂时不可写')
    if (id === 'RVW-09') return hasText('评审为只读')
    if (id === 'RVW-12') return hasText('评审记录需要恢复处理')

    if (id === 'RVW-02') {
      if (!aggregateWorkspaceReady()) return false
      clickOnce(namedButton('开始评审'), 'start-review')
      return hasDialog('确认本轮评审范围') && hasText('固定 30 项并开始')
    }

    if (id === 'RVW-10' || id === 'RVW-11') {
      clickOnce(namedButton('完成本轮评审'), 'prepare-completion')
      return (
        hasDialog('完成本轮评审') &&
        (id !== 'RVW-11' || hasText(ACCEPTANCE_FILES[2]?.relativePath ?? '衣服/A01/商品-03.jpg'))
      )
    }

    if (id === 'RVW-13') {
      clickOnce(namedButton('查看评审意见'), 'open-read-only-inspector')
      return document.querySelector('.review-inspector') !== null && hasText('已完成记录')
    }

    if (!contentWorkspaceReady()) return false

    const workbenchRecipe = REVIEW_WORKBENCH_SCENES[id]
    if (workbenchRecipe !== undefined) {
      if (id !== 'RVW-23' && document.querySelector('.image-preview') === null) {
        const images = document.querySelectorAll<HTMLElement>('.image-cell')
        const image = images[id === 'RVW-21' ? 6 : 0]
        if (image !== undefined)
          dispatchOnce(image, 'open-workbench', new MouseEvent('dblclick', { bubbles: true }))
        return false
      }
      return workbenchRecipe()
    }

    if (id === 'RVW-01') {
      if (!selectImages(1)) return false
      clickOnce(namedButton('开始评审'), 'start-review')
      return hasDialog('确认本轮评审范围') && hasText('固定 1 项并开始')
    }

    if (id === 'RVW-03') return hasText('本轮评审') && selectedImageCount() === 0

    if (id === 'RVW-04') {
      const image = document.querySelector<HTMLElement>('.image-cell')
      if (document.querySelector('.image-preview') === null && image !== null) {
        dispatchOnce(image, 'open-preview', new MouseEvent('dblclick', { bubbles: true }))
      }
      return (
        document.querySelector('.image-preview') !== null && namedButton('完成本轮评审') !== null
      )
    }

    const targetCount = id === 'RVW-05' ? 2 : 1
    if (!selectImages(targetCount)) return false
    clickOnce(namedButton('评审意见'), 'open-inspector')
    const inspector = document.querySelector('.review-inspector')
    if (inspector === null) return false

    if (id === 'RVW-05') return hasText('2 个已选目标可添加意见')
    if (id === 'RVW-13') return hasText('已完成记录')

    const textarea = document.querySelector<HTMLTextAreaElement>('#review-feedback-text')
    if (id === 'RVW-06') {
      if (textarea === null) return false
      setTextAreaOnce(textarea, 'save-error', '请降低背景噪点并保留主体边缘。')
      clickOnce(namedButton('添加意见'), 'submit-feedback')
      return hasText('意见尚未保存，请重试。')
    }
    if (id === 'RVW-14') {
      if (textarea === null) return false
      setTextAreaOnce(textarea, 'unsaved-focus', '保留未保存的自然语言意见。')
      clickOnce(namedButton('完成本轮评审'), 'guard-completion')
      return hasDialog('放弃未保存的意见？') && dialogContainsFocusedControl()
    }
    if (id === 'RVW-15') {
      return (
        textarea !== null &&
        hasText('1 个已选目标可添加意见') &&
        document.querySelector('.task-bar') === null
      )
    }
    return false
  }
}

function aggregateWorkspaceReady(): boolean {
  if (document.querySelector('.aggregate-label') !== null) return workspaceThumbnailsReady()
  const scopeFolder = document.querySelector<HTMLElement>('[role="treeitem"][aria-label="衣服"]')
  if (scopeFolder === null || scopeFolder.getAttribute('aria-selected') !== 'true') {
    clickOnce(scopeFolder, 'select-aggregate-scope')
    return false
  }
  if (
    document.querySelector('.folder-overview') === null ||
    document.querySelector('.folder-filmstrip-skeleton') !== null
  ) {
    return false
  }
  const menu = document.querySelector('[aria-label="视图选项"]')
  if (menu === null) {
    clickOnce(namedButton('视图'), 'open-view-menu')
    return false
  }
  clickOnce(namedButton('显示全部后代文件'), 'show-all-descendants')
  return false
}

function contentWorkspaceReady(): boolean {
  if (document.querySelector('.image-cell') !== null) return workspaceThumbnailsReady()
  const target = document.querySelector<HTMLElement>('[role="treeitem"][aria-label="衣服/A01"]')
  if (target !== null) {
    clickOnce(target, 'open-content-folder')
    return false
  }
  const disclosure = document.querySelector<HTMLElement>('[aria-label="展开 衣服"]')
  clickOnce(disclosure, 'expand-clothes')
  return false
}

function workspaceThumbnailsReady(): boolean {
  const thumbnails = [...document.querySelectorAll<HTMLElement>('.aspect-thumbnail')]
  return (
    thumbnails.length > 0 &&
    thumbnails.every((thumbnail) => thumbnail.dataset.thumbnailState === 'ready')
  )
}

function selectImages(count: number): boolean {
  const images = [...document.querySelectorAll<HTMLElement>('.image-cell')]
  if (images.length < count) return false
  const selected = images.filter((image) => image.getAttribute('aria-selected') === 'true')
  if (selected.length === count) return true
  const next = images.find((image) => image.getAttribute('aria-selected') !== 'true')
  if (next !== undefined) {
    next.dispatchEvent(
      new MouseEvent('click', {
        bubbles: true,
        cancelable: true,
        metaKey: selected.length > 0,
      }),
    )
  }
  return false
}

function selectedImageCount(): number {
  return document.querySelectorAll('.image-cell[aria-selected="true"]').length
}

function namedButton(name: string): HTMLElement | null {
  return (
    [...document.querySelectorAll<HTMLElement>('button, summary')].find(
      (element) =>
        element.textContent?.trim() === name || element.getAttribute('aria-label') === name,
    ) ?? null
  )
}

function clickOnce(element: HTMLElement | null, action: string) {
  if (element === null || element.dataset.acceptanceAction === action) return
  element.dataset.acceptanceAction = action
  element.click()
}

function dispatchOnce(element: HTMLElement, action: string, event: Event) {
  if (element.dataset.acceptanceAction === action) return
  element.dataset.acceptanceAction = action
  element.dispatchEvent(event)
}

function setTextAreaOnce(textarea: HTMLTextAreaElement, action: string, value: string) {
  if (textarea.dataset.acceptanceAction === action) return
  textarea.dataset.acceptanceAction = action
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set
  setter?.call(textarea, value)
  textarea.dispatchEvent(new Event('input', { bubbles: true }))
  textarea.dispatchEvent(new Event('change', { bubbles: true }))
}

function hasText(text: string): boolean {
  return [...document.querySelectorAll<HTMLElement>('body *')].some(
    (element) => element.children.length === 0 && element.textContent?.includes(text),
  )
}

function hasDialog(name: string): boolean {
  return [...document.querySelectorAll<HTMLElement>('[role="dialog"]')].some((dialog) =>
    dialog.textContent?.includes(name),
  )
}

function dialogContainsFocusedControl(): boolean {
  const dialog = document.querySelector<HTMLElement>('[role="dialog"]')
  return (
    dialog !== null && document.activeElement !== null && dialog.contains(document.activeElement)
  )
}
