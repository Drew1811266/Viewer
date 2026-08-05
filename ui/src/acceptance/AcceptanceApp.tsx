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

  useEffect(() => {
    if (status !== 'pending') return
    const firstFrame = requestAnimationFrame(() => {
      const secondFrame = requestAnimationFrame(() => setStatus('ready'))
      cancellation.second = secondFrame
    })
    const cancellation: { second?: number } = {}
    return () => {
      cancelAnimationFrame(firstFrame)
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
        style={{ width: request.width, height: request.height, overflow: 'hidden' }}
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
