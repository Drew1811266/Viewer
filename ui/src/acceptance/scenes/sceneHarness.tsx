import { type ReactNode, useEffect, useMemo, useRef, useState } from 'react'
import App from '../../App'
import type {
  ReviewArchiveSelection,
  ReviewHistorySelector,
  ReviewWorkspacePort,
} from '../../api/reviewWorkspaceTypes'
import type { ViewerBridge } from '../../api/viewer'
import { type AcceptanceBridgeOverrides, createAcceptanceBridge } from '../acceptanceBridge'

const EMPTY_BRIDGE_OVERRIDES: AcceptanceBridgeOverrides = {}

export default function AcceptanceProductScene({
  children,
  ready,
  bridgeOverrides = EMPTY_BRIDGE_OVERRIDES,
  reviewWorkspacePort,
  reviewWorkspacePresentation,
  attributes = {},
}: {
  children: ReactNode
  ready(): boolean
  bridgeOverrides?: AcceptanceBridgeOverrides
  reviewWorkspacePort?: ReviewWorkspacePort
  reviewWorkspacePresentation?: {
    archiveSelection?: ReviewArchiveSelection
    historySelector?: ReviewHistorySelector
  }
  attributes?: Record<string, string>
}) {
  const bridge = useMemo(
    () =>
      createAcceptanceBridge({
        async projectSnapshot() {
          return null
        },
        ...bridgeOverrides,
      }),
    [bridgeOverrides],
  )
  const [settled, setSettled] = useState(false)
  const completed = useRef(false)
  useEffect(() => {
    let observer: MutationObserver | undefined
    let probeFrame: number | undefined
    let firstFrame: number | undefined
    let secondFrame: number | undefined
    const cancelStablePaint = () => {
      if (firstFrame !== undefined) cancelAnimationFrame(firstFrame)
      if (secondFrame !== undefined) cancelAnimationFrame(secondFrame)
      firstFrame = undefined
      secondFrame = undefined
    }
    const cancelProbe = () => {
      if (probeFrame !== undefined) cancelAnimationFrame(probeFrame)
      probeFrame = undefined
    }
    const scheduleProbe = () => {
      if (probeFrame !== undefined) return
      probeFrame = requestAnimationFrame(() => {
        probeFrame = undefined
        attempt()
      })
    }
    const invalidate = () => {
      cancelStablePaint()
      if (completed.current) {
        completed.current = false
        setSettled(false)
      }
    }
    const visuallyReady = () => workspaceVisualsSettled(document) && ready()
    const attempt = () => {
      const choose = document.querySelector<HTMLElement>(
        '.empty-project [aria-label="选择项目文件夹"], .empty-project button',
      )
      if (choose !== null) {
        if (choose.dataset.acceptanceAction !== 'open-project') {
          choose.dataset.acceptanceAction = 'open-project'
          choose.click()
        }
        return
      }
      if (!visuallyReady()) {
        invalidate()
        scheduleProbe()
        return
      }
      cancelProbe()
      if (completed.current || firstFrame !== undefined || secondFrame !== undefined) return
      firstFrame = requestAnimationFrame(() => {
        firstFrame = undefined
        if (!visuallyReady()) {
          invalidate()
          return
        }
        secondFrame = requestAnimationFrame(() => {
          secondFrame = undefined
          if (!visuallyReady()) {
            invalidate()
            return
          }
          completed.current = true
          setSettled(true)
        })
      })
    }
    observer = new MutationObserver(attempt)
    observer.observe(document.body, { attributes: true, childList: true, subtree: true })
    attempt()
    return () => {
      observer?.disconnect()
      cancelProbe()
      cancelStablePaint()
    }
  }, [ready])
  return (
    <div
      className="acceptance-scene-root"
      data-acceptance-scene-ready={settled ? 'true' : 'false'}
      style={{ width: '100%', height: '100%' }}
      {...attributes}
    >
      <App
        bridge={bridge as ViewerBridge}
        reviewWorkspacePort={reviewWorkspacePort}
        reviewWorkspacePresentation={reviewWorkspacePresentation}
      />
      {children}
    </div>
  )
}

export function workspaceVisualsSettled(root: Document | Element): boolean {
  const shell = root.querySelector('.viewer-shell')
  if (shell === null) return false
  if (
    shell.querySelector(
      '.folder-tree-skeleton-row, .folder-filmstrip-skeleton, [data-thumbnail-state="loading"]',
    ) !== null
  ) {
    return false
  }
  const thumbnails = [...shell.querySelectorAll<HTMLElement>('.aspect-thumbnail')]
  if (thumbnails.length === 0) {
    return (
      shell.querySelector(
        '.folder-overview, .content-browser, .image-preview .image-preview-image',
      ) !== null
    )
  }
  return thumbnails.every((thumbnail) => {
    if (thumbnail.dataset.thumbnailState === 'failed') {
      return (
        thumbnail.querySelector('.aspect-thumbnail-placeholder[aria-label="缩略图不可用"]') !== null
      )
    }
    return (
      thumbnail.dataset.thumbnailState === 'ready' &&
      thumbnail.querySelector('.aspect-thumbnail-placeholder') === null &&
      thumbnail.querySelector('img') !== null
    )
  })
}

export function setAcceptanceInputValue(input: HTMLInputElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set
  setter?.call(input, value)
  input.dispatchEvent(new Event('input', { bubbles: true }))
  input.dispatchEvent(new Event('change', { bubbles: true }))
}
