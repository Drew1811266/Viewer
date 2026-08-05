import { type ReactNode, useEffect, useMemo, useRef, useState } from 'react'
import App from '../../App'
import type { ViewerBridge } from '../../api/viewer'
import { type AcceptanceBridgeOverrides, createAcceptanceBridge } from '../acceptanceBridge'

const EMPTY_BRIDGE_OVERRIDES: AcceptanceBridgeOverrides = {}

export default function AcceptanceProductScene({
  children,
  ready,
  bridgeOverrides = EMPTY_BRIDGE_OVERRIDES,
  attributes = {},
}: {
  children: ReactNode
  ready(): boolean
  bridgeOverrides?: AcceptanceBridgeOverrides
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
    let firstFrame: number | undefined
    let secondFrame: number | undefined
    const attempt = () => {
      if (completed.current) return
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
      if (!workspaceVisualsSettled(document) || !ready()) return
      completed.current = true
      observer?.disconnect()
      firstFrame = requestAnimationFrame(() => {
        secondFrame = requestAnimationFrame(() => setSettled(true))
      })
    }
    observer = new MutationObserver(attempt)
    observer.observe(document.body, { attributes: true, childList: true, subtree: true })
    attempt()
    return () => {
      observer?.disconnect()
      if (firstFrame !== undefined) cancelAnimationFrame(firstFrame)
      if (secondFrame !== undefined) cancelAnimationFrame(secondFrame)
    }
  }, [ready])
  return (
    <div
      className="acceptance-scene-root"
      data-acceptance-scene-ready={settled ? 'true' : 'false'}
      style={{ width: '100%', height: '100%' }}
      {...attributes}
    >
      <App bridge={bridge as ViewerBridge} />
      {children}
    </div>
  )
}

export function workspaceVisualsSettled(root: Document | Element): boolean {
  const shell = root.querySelector('.viewer-shell')
  if (shell === null) return false
  const thumbnails = [...shell.querySelectorAll<HTMLElement>('.aspect-thumbnail')]
  if (thumbnails.length === 0) return false
  return thumbnails.every(
    (thumbnail) =>
      thumbnail.dataset.thumbnailState === 'ready' &&
      thumbnail.querySelector('.aspect-thumbnail-placeholder') === null &&
      thumbnail.querySelector('img') !== null,
  )
}

export function setAcceptanceInputValue(input: HTMLInputElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set
  setter?.call(input, value)
  input.dispatchEvent(new Event('input', { bubbles: true }))
  input.dispatchEvent(new Event('change', { bubbles: true }))
}
