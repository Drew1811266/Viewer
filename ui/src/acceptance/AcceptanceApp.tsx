import {
  Component,
  type ComponentType,
  type ErrorInfo,
  type ReactNode,
  useEffect,
  useState,
} from 'react'
import type { AcceptanceRequest } from './acceptanceRequest'

export interface AcceptanceSceneProps {
  request: AcceptanceRequest
}

export type AcceptanceSceneRegistry = Partial<Record<string, ComponentType<AcceptanceSceneProps>>>

interface AcceptanceAppProps {
  request: AcceptanceRequest
  sceneRegistry: AcceptanceSceneRegistry
}

type AcceptanceStatus = 'pending' | 'ready' | 'error'

export default function AcceptanceApp({ request, sceneRegistry }: AcceptanceAppProps) {
  const [status, setStatus] = useState<AcceptanceStatus>('pending')
  const [error, setError] = useState<string | null>(null)
  const Scene = sceneRegistry[request.id] ?? missingScene(request.id)
  const browserZoom = request.id === 'A11Y-05' || request.id === 'RVW-15' ? 2 : 1

  useEffect(() => {
    if (status !== 'pending') return
    const frame = document.querySelector<HTMLElement>(
      `[data-acceptance-id="${CSS.escape(request.id)}"]`,
    )
    const cancellation: { first?: number; second?: number } = {}
    let observer: MutationObserver | undefined
    const scheduleStablePaint = () => {
      if (frame?.querySelector('[data-acceptance-scene-ready="false"]') !== null) return false
      observer?.disconnect()
      cancellation.first = requestAnimationFrame(() => {
        cancellation.second = requestAnimationFrame(() => setStatus('ready'))
      })
      return true
    }
    if (!scheduleStablePaint() && frame !== null) {
      observer = new MutationObserver(scheduleStablePaint)
      observer.observe(frame, { attributes: true, childList: true, subtree: true })
    }
    return () => {
      observer?.disconnect()
      if (cancellation.first !== undefined) cancelAnimationFrame(cancellation.first)
      if (cancellation.second !== undefined) cancelAnimationFrame(cancellation.second)
    }
  }, [request.id, request.viewport, status])

  function fail(caught: Error) {
    setError(caught.message)
    setStatus('error')
  }

  return (
    <>
      <main
        data-acceptance-frame
        data-acceptance-id={request.id}
        data-acceptance-viewport={request.viewport}
        data-acceptance-status={status}
        style={{
          width: request.width / browserZoom,
          height: request.height / browserZoom,
          overflow: 'hidden',
        }}
      >
        <AcceptanceSceneBoundary key={request.id} onError={fail}>
          <Scene request={request} />
        </AcceptanceSceneBoundary>
      </main>
      {error !== null && (
        <aside data-acceptance-diagnostic role="alert">
          {error}
        </aside>
      )}
    </>
  )
}

function missingScene(id: string): ComponentType<AcceptanceSceneProps> {
  return function MissingAcceptanceScene() {
    throw new Error(`Missing Viewer acceptance scene: ${id}`)
  }
}

interface BoundaryProps {
  children: ReactNode
  onError(error: Error): void
}

interface BoundaryState {
  failed: boolean
}

class AcceptanceSceneBoundary extends Component<BoundaryProps, BoundaryState> {
  state: BoundaryState = { failed: false }

  static getDerivedStateFromError(): BoundaryState {
    return { failed: true }
  }

  componentDidCatch(error: Error, _errorInfo: ErrorInfo) {
    this.props.onError(error)
  }

  render() {
    return this.state.failed ? null : this.props.children
  }
}
