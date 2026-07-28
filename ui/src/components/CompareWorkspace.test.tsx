import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { BrowserFile, ImageRepresentation, ImageRepresentationRequest } from '../api/types'
import { defined } from '../defined'
import CompareWorkspace from './CompareWorkspace'

const files = ['a', 'b', 'c', 'd'].map((id) => image(id, `${id}.jpg`))

describe('CompareWorkspace', () => {
  it('rejects invalid cardinality and non-image candidates with safe feedback', () => {
    const one = renderWorkspace({ files: files.slice(0, 1) })
    expect(screen.getByRole('alert')).toHaveTextContent('请选择 2–20 张 JPG 或 PNG')
    one.unmount()

    renderWorkspace({
      files: [
        defined(files[0], 'Expected first comparison fixture'),
        { ...defined(files[1], 'Expected second comparison fixture'), kind: 'text' },
      ],
    })
    expect(screen.getByRole('alert')).toHaveTextContent('请选择 2–20 张 JPG 或 PNG')
  })

  it('uses two-column, asymmetric-three and four-grid layouts', () => {
    for (const [count, layout] of [
      [2, 'two_columns'],
      [3, 'three_asymmetric'],
      [4, 'four_grid'],
    ] as const) {
      const rendered = renderWorkspace({ files: files.slice(0, count) })
      expect(screen.getByRole('region', { name: '图片对比' })).toHaveAttribute(
        'data-layout',
        layout,
      )
      rendered.unmount()
    }
  })

  it('synchronizes by default, retains independent transforms and keeps rotation pane-local', () => {
    renderWorkspace()
    fireEvent.click(screen.getByRole('button', { name: '放大当前对比' }))
    expect(pane('a')).toHaveAttribute('data-scale', '1.25')
    expect(pane('b')).toHaveAttribute('data-scale', '1.25')

    fireEvent.click(screen.getByRole('button', { name: '切换为独立变换' }))
    fireEvent.focus(pane('b'))
    fireEvent.click(screen.getByRole('button', { name: '放大当前对比' }))
    expect(pane('a')).toHaveAttribute('data-scale', '1.25')
    expect(pane('b')).toHaveAttribute('data-scale', '1.5625')
    fireEvent.click(screen.getByRole('button', { name: '顺时针旋转当前图片' }))
    expect(pane('a')).toHaveAttribute('data-rotation', '0')
    expect(pane('b')).toHaveAttribute('data-rotation', '90')
  })

  it('preserves surviving pane transforms on removal and exits at one or zero panes', () => {
    const changed = vi.fn()
    const rendered = renderWorkspace({ files, onEntityIdsChange: changed })
    fireEvent.click(screen.getByRole('button', { name: '切换为独立变换' }))
    fireEvent.focus(pane('b'))
    fireEvent.click(screen.getByRole('button', { name: '放大当前对比' }))
    fireEvent.click(screen.getByRole('button', { name: '移除 d.jpg' }))
    expect(changed).toHaveBeenLastCalledWith(['a', 'b', 'c'])
    expect(pane('b')).toHaveAttribute('data-scale', '1.25')
    expect(screen.getByRole('region', { name: '图片对比' })).toHaveAttribute(
      'data-layout',
      'three_asymmetric',
    )
    rendered.unmount()

    renderWorkspace({ onEntityIdsChange: changed })
    fireEvent.click(screen.getByRole('button', { name: '移除 a.jpg' }))
    expect(changed).toHaveBeenLastCalledWith(['b'])
    fireEvent.click(screen.getByRole('button', { name: '关闭对比' }))
    expect(changed).toHaveBeenLastCalledWith([])
  })

  it('supports focused keyboard exit and never disables compare in read-only mode', () => {
    const changed = vi.fn()
    renderWorkspace({ readOnly: true, onEntityIdsChange: changed })
    const workspace = screen.getByRole('region', { name: '图片对比' })
    workspace.focus()
    fireEvent.keyDown(workspace, { key: 'Escape' })
    expect(changed).toHaveBeenCalledWith([])
    expect(screen.getByRole('button', { name: 'a.jpg 标记为保留' })).toBeDisabled()
  })

  it('requests 100% only for the active pane and reuses resident fit proxies', async () => {
    const requestImage = vi.fn(
      (_file: BrowserFile, _request: ImageRepresentationRequest) =>
        new Promise<ImageRepresentation>(() => undefined),
    )
    renderWorkspace({ requestImage })
    fireEvent.focus(pane('b'))
    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    await waitFor(() =>
      expect(requestImage).toHaveBeenCalledWith(files[1], {
        kind: 'original100_percent',
      }),
    )
    expect(
      requestImage.mock.calls.filter(([, request]) => request.kind === 'original100_percent'),
    ).toHaveLength(1)

    fireEvent.click(screen.getByRole('button', { name: '适应窗口' }))
    expect(
      requestImage.mock.calls.filter(([, request]) => request.kind === 'fit_preview'),
    ).toHaveLength(2)
  })

  it('recomputes actual pixels when 100% is requested before dimensions arrive', async () => {
    const requestImage = vi.fn((_file: BrowserFile, request: ImageRepresentationRequest) =>
      request.kind === 'original100_percent'
        ? Promise.resolve({
            cacheKey: 'original-a',
            url: 'viewer-image://localhost/original-a',
            width: 4_000,
            height: 3_000,
            backend: 'image_io' as const,
          })
        : new Promise<ImageRepresentation>(() => undefined),
    )
    renderWorkspace({ requestImage })
    fireEvent.click(screen.getByRole('button', { name: '100%' }))

    await waitFor(() => expect(pane('a')).toHaveAttribute('data-scale', '6.25'))
  })

  it('returns the transform to fit when an original exceeds the budget', async () => {
    const status = vi.fn()
    const requestImage = vi.fn((_file: BrowserFile, request: ImageRepresentationRequest) =>
      request.kind === 'original100_percent'
        ? Promise.reject({ code: 'image_budget_exceeded' })
        : Promise.resolve({
            cacheKey: 'proxy',
            url: 'viewer-image://localhost/proxy',
            width: 800,
            height: 600,
            backend: 'image_io' as const,
          }),
    )
    renderWorkspace({ requestImage, onStatus: status })
    await screen.findByRole('img', { name: 'a.jpg' })
    fireEvent.click(screen.getByRole('button', { name: '100%' }))

    await waitFor(() => expect(pane('a')).toHaveAttribute('data-scale', '1'))
    expect(status).toHaveBeenCalledWith('原图超出安全预览限制，已继续使用适窗代理。')
  })

  it('serializes rapid original switches and keeps every pane proxy resident', async () => {
    const firstOriginal = deferred<ImageRepresentation>()
    const secondOriginal = deferred<ImageRepresentation>()
    let originalCalls = 0
    const requestImage = vi.fn((file: BrowserFile, request: ImageRepresentationRequest) => {
      if (request.kind === 'original100_percent') {
        originalCalls += 1
        return originalCalls === 1 ? firstOriginal.promise : secondOriginal.promise
      }
      return Promise.resolve({
        cacheKey: `proxy-${file.entityId}`,
        url: `viewer-image://localhost/proxy-${file.entityId}`,
        width: 800,
        height: 600,
        backend: 'image_io' as const,
      })
    })
    renderWorkspace({ requestImage })
    await screen.findByRole('img', { name: 'a.jpg' })
    await screen.findByRole('img', { name: 'b.jpg' })

    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    await waitFor(() => expect(originalCalls).toBe(1))
    fireEvent.focus(pane('b'))
    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    expect(originalCalls).toBe(1)
    expect(screen.getByRole('img', { name: 'a.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/proxy-a',
    )

    await act(async () => firstOriginal.resolve(imageRepresentation('original-a')))
    await waitFor(() => expect(originalCalls).toBe(2))
    await act(async () => secondOriginal.resolve(imageRepresentation('original-b')))
    expect(await screen.findByRole('img', { name: 'b.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/original-b',
    )
  })

  it('drops a stale queued original before it reaches the native image lane', async () => {
    const running = deferred<ImageRepresentation>()
    const latest = deferred<ImageRepresentation>()
    const originalEntityIds: string[] = []
    const requestImage = vi.fn((file: BrowserFile, request: ImageRepresentationRequest) => {
      if (request.kind === 'original100_percent') {
        originalEntityIds.push(file.entityId)
        return originalEntityIds.length === 1 ? running.promise : latest.promise
      }
      return Promise.resolve({
        cacheKey: `proxy-${file.entityId}`,
        url: `viewer-image://localhost/proxy-${file.entityId}`,
        width: 800,
        height: 600,
        backend: 'image_io' as const,
      })
    })
    renderWorkspace({ requestImage })
    await screen.findByRole('img', { name: 'a.jpg' })
    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    await waitFor(() => expect(originalEntityIds).toEqual(['a']))
    fireEvent.focus(pane('b'))
    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    fireEvent.focus(pane('a'))
    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    expect(originalEntityIds).toEqual(['a'])

    await act(async () => running.resolve(imageRepresentation('stale-a')))
    await waitFor(() => expect(originalEntityIds).toEqual(['a', 'a']))
    await act(async () => latest.resolve(imageRepresentation('latest-a')))
    expect(await screen.findByRole('img', { name: 'a.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/latest-a',
    )
  })

  it('repairs externally removed panes without resetting surviving transforms', async () => {
    const changed = vi.fn()
    const rendered = renderWorkspace({ files: files.slice(0, 3), onEntityIdsChange: changed })
    fireEvent.click(screen.getByRole('button', { name: '切换为独立变换' }))
    fireEvent.focus(pane('b'))
    fireEvent.click(screen.getByRole('button', { name: '放大当前对比' }))

    rendered.rerender(workspace({ files: files.slice(0, 2), onEntityIdsChange: changed }))
    await waitFor(() =>
      expect(screen.getByRole('region', { name: '图片对比' })).toHaveAttribute(
        'data-layout',
        'two_columns',
      ),
    )
    expect(pane('b')).toHaveAttribute('data-scale', '1.25')

    rendered.rerender(workspace({ files: files.slice(0, 1), onEntityIdsChange: changed }))
    await waitFor(() => expect(changed).toHaveBeenLastCalledWith(['a']))
  })
})

function pane(id: string) {
  return screen.getByRole('article', { name: `对比 ${id}.jpg` })
}

function renderWorkspace(overrides: Partial<React.ComponentProps<typeof CompareWorkspace>> = {}) {
  return render(workspace(overrides))
}

function workspace(overrides: Partial<React.ComponentProps<typeof CompareWorkspace>> = {}) {
  return (
    <CompareWorkspace
      files={files.slice(0, 2)}
      readOnly={false}
      requestImage={vi.fn(() => new Promise<ImageRepresentation>(() => undefined))}
      onEntityIdsChange={vi.fn()}
      onSetReview={vi.fn()}
      onToggleFavorite={vi.fn()}
      onStatus={vi.fn()}
      {...overrides}
    />
  )
}

function image(entityId: string, name: string): BrowserFile {
  return {
    entityId,
    relativePath: `id/${name}`,
    name,
    kind: 'jpeg',
    size: 100,
    modifiedNs: '1',
    marker: { reviewState: null, favorite: false },
    imageMetadata: { width: 4_000, height: 3_000 },
    imageUrl: null,
  }
}

function imageRepresentation(token: string): ImageRepresentation {
  return {
    cacheKey: token,
    url: `viewer-image://localhost/${token}`,
    width: 4_000,
    height: 3_000,
    backend: 'image_io',
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((next) => {
    resolve = next
  })
  return { promise, resolve }
}
