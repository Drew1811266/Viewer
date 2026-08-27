import { act, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import AcceptanceApp, { type AcceptanceSceneRegistry } from './AcceptanceApp'
import type { AcceptanceRequest } from './acceptanceRequest'

const request: AcceptanceRequest = {
  id: 'PRE-01',
  viewport: '1024x720',
  width: 1024,
  height: 720,
}

describe('Viewer visual acceptance root protocol', () => {
  const frames: Array<{ id: number; callback: FrameRequestCallback; cancelled: boolean }> = []
  let nextFrameId = 1

  beforeEach(() => {
    frames.length = 0
    nextFrameId = 1
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      const id = nextFrameId
      nextFrameId += 1
      frames.push({ id, callback, cancelled: false })
      return id
    })
    vi.stubGlobal('cancelAnimationFrame', (id: number) => {
      const frame = frames.find((candidate) => candidate.id === id)
      if (frame !== undefined) frame.cancelled = true
    })
  })

  afterEach(() => vi.unstubAllGlobals())

  it('becomes ready only after the requested formal scene has painted twice', () => {
    const registry: AcceptanceSceneRegistry = {
      'PRE-01': () => <section aria-label="formal preview">Preview</section>,
    }
    const { container } = render(<AcceptanceApp request={request} sceneRegistry={registry} />)
    const root = container.querySelector<HTMLElement>('[data-acceptance-id="PRE-01"]')

    expect(root).toHaveAttribute('data-acceptance-viewport', '1024x720')
    expect(root).toHaveAttribute('data-acceptance-status', 'pending')
    expect(root).toHaveStyle({ width: '1024px', height: '720px' })
    expect(screen.getByRole('region', { name: 'formal preview' })).toBeVisible()

    act(() => runNextFrame(frames, 0))
    expect(root).toHaveAttribute('data-acceptance-status', 'pending')
    act(() => runNextFrame(frames, 16))
    expect(root).toHaveAttribute('data-acceptance-status', 'ready')
  })

  it('waits for a scene readiness boundary before counting the two stable paint frames', async () => {
    function PendingScene() {
      return (
        <section aria-label="async formal workspace" data-acceptance-scene-ready="false">
          Workspace
        </section>
      )
    }
    const registry: AcceptanceSceneRegistry = { 'PRE-01': PendingScene }
    const { container } = render(<AcceptanceApp request={request} sceneRegistry={registry} />)
    const root = container.querySelector<HTMLElement>('[data-acceptance-id="PRE-01"]')
    const scene = screen.getByRole('region', { name: 'async formal workspace' })

    expect(root).toHaveAttribute('data-acceptance-status', 'pending')
    expect(frames).toHaveLength(0)

    await act(async () => {
      scene.setAttribute('data-acceptance-scene-ready', 'true')
      await Promise.resolve()
    })
    expect(frames).toHaveLength(1)
    act(() => runNextFrame(frames, 0))
    expect(root).toHaveAttribute('data-acceptance-status', 'pending')
    act(() => runNextFrame(frames, 16))
    expect(root).toHaveAttribute('data-acceptance-status', 'ready')
  })

  it('restarts its stable paint frames when the scene readiness boundary reopens', async () => {
    function PendingScene() {
      return (
        <section aria-label="late thumbnail workspace" data-acceptance-scene-ready="false">
          Workspace
        </section>
      )
    }
    const registry: AcceptanceSceneRegistry = { 'PRE-01': PendingScene }
    const { container } = render(<AcceptanceApp request={request} sceneRegistry={registry} />)
    const root = container.querySelector<HTMLElement>('[data-acceptance-id="PRE-01"]')
    const scene = screen.getByRole('region', { name: 'late thumbnail workspace' })

    await setSceneReadiness(scene, 'true')
    act(() => runNextFrame(frames, 0))
    await setSceneReadiness(scene, 'false')
    act(() => runNextFrame(frames, 16))
    expect(root).toHaveAttribute('data-acceptance-status', 'pending')

    await setSceneReadiness(scene, 'true')
    act(() => runNextFrame(frames, 32))
    act(() => runNextFrame(frames, 48))
    expect(root).toHaveAttribute('data-acceptance-status', 'ready')
  })

  it('reports one bounded scene failure outside the screenshot root without retrying', () => {
    const errorLog = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const registry: AcceptanceSceneRegistry = {
      'PRE-01': () => {
        throw new Error('preview fixture failed')
      },
    }
    const { container } = render(<AcceptanceApp request={request} sceneRegistry={registry} />)

    expect(container.querySelector('[data-acceptance-frame]')).toHaveAttribute(
      'data-acceptance-status',
      'error',
    )
    expect(screen.getByRole('alert')).toHaveTextContent('preview fixture failed')
    act(() => runNextFrame(frames, 0))
    act(() => runNextFrame(frames, 16))
    expect(container.querySelector('[data-acceptance-frame]')).toHaveAttribute(
      'data-acceptance-status',
      'error',
    )
    expect(screen.getAllByRole('alert')).toHaveLength(1)
    errorLog.mockRestore()
  })

  it.each(['A11Y-05', 'RVW-15', 'RVW-24'])('uses a half-size logical frame for %s', (id) => {
    const zoomRequest: AcceptanceRequest = {
      id,
      viewport: '1024x720',
      width: 1024,
      height: 720,
    }
    const registry: AcceptanceSceneRegistry = { [id]: () => <section>Zoom</section> }
    const { container } = render(<AcceptanceApp request={zoomRequest} sceneRegistry={registry} />)

    expect(container.querySelector('[data-acceptance-frame]')).toHaveStyle({
      width: '512px',
      height: '360px',
    })
  })
  it('preserves the uncatalogued video feasibility probe', () => {
    const probe = { ...request, id: 'VIDEO-FEASIBILITY' }
    render(
      <AcceptanceApp
        request={probe}
        sceneRegistry={{ 'VIDEO-FEASIBILITY': () => <section>Probe</section> }}
      />,
    )
    expect(screen.getByText('Probe')).toBeVisible()
  })
})

function runNextFrame(
  frames: Array<{ callback: FrameRequestCallback; cancelled: boolean }>,
  timestamp: number,
) {
  const frame = frames.shift()
  if (frame !== undefined && !frame.cancelled) frame.callback(timestamp)
}

async function setSceneReadiness(scene: HTMLElement, value: 'true' | 'false') {
  await act(async () => {
    scene.setAttribute('data-acceptance-scene-ready', value)
    await Promise.resolve()
  })
}
