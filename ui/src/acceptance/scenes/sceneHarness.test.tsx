import { act, render } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import AcceptanceProductScene, { workspaceVisualsSettled } from './sceneHarness'

vi.mock('../../App', () => ({
  default: () => (
    <main className="viewer-shell">
      <section className="content-browser" />
    </main>
  ),
}))

describe('acceptance product scene readiness', () => {
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

  it('holds readiness when thumbnail work begins during its stable paint frames', async () => {
    const { container } = render(
      <AcceptanceProductScene ready={() => document.querySelector('.task-bar') === null}>
        {null}
      </AcceptanceProductScene>,
    )
    const root = container.querySelector<HTMLElement>('.acceptance-scene-root')

    await act(async () => {
      await Promise.resolve()
    })

    expect(root).toHaveAttribute('data-acceptance-scene-ready', 'false')
    expect(document.querySelector('.viewer-shell')).not.toBeNull()
    expect(workspaceVisualsSettled(document)).toBe(true)
    expect(frames).toHaveLength(1)

    const task = document.createElement('aside')
    task.className = 'task-bar'
    await act(async () => {
      document.body.append(task)
      await Promise.resolve()
    })

    act(() => runNextFrame(frames, 0))
    act(() => runNextFrame(frames, 16))
    expect(root).toHaveAttribute('data-acceptance-scene-ready', 'false')

    await act(async () => {
      task.remove()
      await Promise.resolve()
    })
    act(() => runNextFrame(frames, 32))
    act(() => runNextFrame(frames, 48))
    act(() => runNextFrame(frames, 64))
    expect(root).toHaveAttribute('data-acceptance-scene-ready', 'true')
  })

  it('rechecks asynchronous readiness without requiring a DOM mutation', async () => {
    let resourceReady = false
    const { container } = render(
      <AcceptanceProductScene ready={() => resourceReady}>{null}</AcceptanceProductScene>,
    )
    const root = container.querySelector<HTMLElement>('.acceptance-scene-root')

    await act(async () => {
      await Promise.resolve()
    })
    expect(frames).toHaveLength(1)

    resourceReady = true
    act(() => runNextFrame(frames, 0))
    act(() => runNextFrame(frames, 16))
    act(() => runNextFrame(frames, 32))
    expect(root).toHaveAttribute('data-acceptance-scene-ready', 'true')
  })

  it('waits for real thumbnail content instead of capturing workspace skeletons', () => {
    const root = document.createElement('div')
    root.innerHTML = `
      <main class="viewer-shell">
        <span class="aspect-thumbnail" data-thumbnail-state="loading">
          <span class="aspect-thumbnail-placeholder" aria-label="缩略图加载中"></span>
        </span>
      </main>
    `
    expect(workspaceVisualsSettled(root)).toBe(false)

    root.innerHTML = `
      <main class="viewer-shell">
        <span class="aspect-thumbnail" data-thumbnail-state="ready"><img alt="" /></span>
      </main>
    `
    expect(workspaceVisualsSettled(root)).toBe(true)
  })

  it('accepts completed empty overviews and terminal thumbnail failures', () => {
    const root = document.createElement('div')
    root.innerHTML = `
      <main class="viewer-shell">
        <section class="folder-overview"><div class="folder-filmstrip-list"></div></section>
      </main>
    `
    expect(workspaceVisualsSettled(root)).toBe(true)

    root.innerHTML = `
      <main class="viewer-shell">
        <section class="content-browser">
          <span class="aspect-thumbnail" data-thumbnail-state="failed">
            <span class="aspect-thumbnail-placeholder" aria-label="缩略图不可用"></span>
          </span>
        </section>
      </main>
    `
    expect(workspaceVisualsSettled(root)).toBe(true)
  })

  it('accepts the real review workbench after its image has rendered', () => {
    const root = document.createElement('div')
    root.innerHTML = `
      <main class="viewer-shell">
        <section class="image-preview">
          <img class="image-preview-image" alt="商品-01.jpg" />
        </section>
      </main>
    `

    expect(workspaceVisualsSettled(root)).toBe(true)
  })

  it('does not accept an overview while its rows are still skeletons', () => {
    const root = document.createElement('div')
    root.innerHTML = `
      <main class="viewer-shell">
        <section class="folder-overview">
          <div class="folder-filmstrip-skeleton"></div>
        </section>
      </main>
    `
    expect(workspaceVisualsSettled(root)).toBe(false)
  })
})

function runNextFrame(
  frames: Array<{ callback: FrameRequestCallback; cancelled: boolean }>,
  timestamp: number,
) {
  const frame = frames.shift()
  if (frame !== undefined && !frame.cancelled) frame.callback(timestamp)
}
