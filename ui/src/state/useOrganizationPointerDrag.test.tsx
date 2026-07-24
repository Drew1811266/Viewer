import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { defined } from '../defined'
import {
  type OrganizationPointerInput,
  useOrganizationPointerDrag,
} from './useOrganizationPointerDrag'

interface FrameHarness {
  cancel: ReturnType<typeof vi.fn>
  flushNext: () => void
  pending: () => number
  request: ReturnType<typeof vi.fn>
}

interface CaptureHarness {
  node: HTMLElement
  release: ReturnType<typeof vi.fn>
  set: ReturnType<typeof vi.fn>
  setOwned: (owned: boolean) => void
}

const rect = (top = 0, bottom = 200): DOMRect =>
  ({
    bottom,
    height: bottom - top,
    left: 0,
    right: 200,
    top,
    width: 200,
    x: 0,
    y: top,
    toJSON: () => ({}),
  }) as DOMRect

function frames(): FrameHarness {
  let nextId = 1
  const callbacks = new Map<number, FrameRequestCallback>()
  const request = vi.fn((callback: FrameRequestCallback) => {
    const id = nextId++
    callbacks.set(id, callback)
    return id
  })
  const cancel = vi.fn((id: number) => callbacks.delete(id))
  vi.stubGlobal('requestAnimationFrame', request)
  vi.stubGlobal('cancelAnimationFrame', cancel)
  return {
    cancel,
    flushNext: () => {
      const entry = callbacks.entries().next().value as [number, FrameRequestCallback] | undefined
      if (!entry) throw new Error('No animation frame is pending')
      callbacks.delete(entry[0])
      entry[1](performance.now())
    },
    pending: () => callbacks.size,
    request,
  }
}

function captureNode(pointerId = 7): CaptureHarness {
  let owned = false
  const node = document.createElement('div')
  const set = vi.fn((nextPointerId: number) => {
    if (nextPointerId === pointerId) owned = true
  })
  const release = vi.fn((nextPointerId: number) => {
    if (nextPointerId === pointerId) owned = false
  })
  Object.defineProperties(node, {
    hasPointerCapture: {
      configurable: true,
      value: vi.fn((nextPointerId: number) => nextPointerId === pointerId && owned),
    },
    releasePointerCapture: { configurable: true, value: release },
    setPointerCapture: { configurable: true, value: set },
  })
  return { node, release, set, setOwned: (next) => (owned = next) }
}

function start(
  capture: CaptureHarness,
  overrides: Partial<Extract<OrganizationPointerInput, { type: 'start' }>> = {},
): Extract<OrganizationPointerInput, { type: 'start' }> {
  return {
    type: 'start',
    pointerId: 7,
    entityIds: ['entity-2', 'entity-1'],
    mode: 'move',
    clientX: 50,
    clientY: 50,
    captureNode: capture.node,
    ...overrides,
  }
}

describe('useOrganizationPointerDrag', () => {
  let surface: HTMLElement
  let row: HTMLElement
  let hit: ReturnType<typeof vi.fn>
  let frameHarness: FrameHarness

  beforeEach(() => {
    surface = document.createElement('div')
    surface.dataset.organizationDropSurface = ''
    row = document.createElement('div')
    row.dataset.organizationFolderId = 'folder-target'
    const rowContent = document.createElement('span')
    row.append(rowContent)
    surface.append(row)
    document.body.append(surface)
    vi.spyOn(surface, 'getBoundingClientRect').mockReturnValue(rect())
    Object.defineProperty(surface, 'scrollBy', {
      configurable: true,
      value: vi.fn(),
    })
    hit = vi.fn(() => rowContent)
    Object.defineProperty(document, 'elementFromPoint', {
      configurable: true,
      value: hit,
    })
    frameHarness = frames()
  })

  afterEach(() => {
    document.body.replaceChildren()
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('keeps a move shorter than 4 px armed and does not submit on pointer up', () => {
    const onDrop = vi.fn()
    const isDropTargetValid = vi.fn(
      (_entityIds: readonly string[], _destinationId: string, _mode: 'move' | 'copy') => true,
    )
    const capture = captureNode()
    const { result } = renderHook(() =>
      useOrganizationPointerDrag({
        disabled: false,
        resetKey: 'generation-1',
        isDropTargetValid,
        onDrop,
      }),
    )

    act(() => {
      result.current.handlePointerInput(start(capture))
      result.current.handlePointerInput({
        type: 'move',
        pointerId: 7,
        clientX: 53.99,
        clientY: 50,
      })
    })
    expect(result.current.dragView).toBeNull()
    expect(result.current.dropTarget).toBeNull()
    expect(isDropTargetValid).not.toHaveBeenCalled()

    act(() =>
      result.current.handlePointerInput({
        type: 'end',
        pointerId: 7,
        clientX: 53.99,
        clientY: 50,
      }),
    )
    expect(onDrop).not.toHaveBeenCalled()
    expect(capture.release).toHaveBeenCalledOnce()
  })

  it('cancels the armed session when pointer capture throws and accepts the next start', () => {
    const onDrop = vi.fn()
    const { result } = renderHook(() =>
      useOrganizationPointerDrag({
        disabled: false,
        resetKey: 'generation-1',
        isDropTargetValid: () => true,
        onDrop,
      }),
    )
    const failedCapture = captureNode(7)
    failedCapture.set.mockImplementation(() => {
      throw new DOMException('capture unavailable')
    })

    act(() => {
      result.current.handlePointerInput(start(failedCapture, { entityIds: ['stale-entity'] }))
      result.current.handlePointerInput({
        type: 'move',
        pointerId: 7,
        clientX: 54,
        clientY: 50,
      })
      result.current.handlePointerInput({
        type: 'end',
        pointerId: 7,
        clientX: 54,
        clientY: 50,
      })
    })
    expect(onDrop).not.toHaveBeenCalled()

    const validCapture = captureNode(8)
    act(() => {
      result.current.handlePointerInput(
        start(validCapture, {
          pointerId: 8,
          entityIds: ['fresh-entity'],
        }),
      )
      result.current.handlePointerInput({
        type: 'move',
        pointerId: 8,
        clientX: 54,
        clientY: 50,
      })
      result.current.handlePointerInput({
        type: 'end',
        pointerId: 8,
        clientX: 54,
        clientY: 50,
      })
    })

    expect(validCapture.set).toHaveBeenCalledWith(8)
    expect(onDrop).toHaveBeenCalledOnce()
    expect(onDrop).toHaveBeenCalledWith(['fresh-entity'], 'folder-target', 'move')
  })

  it('activates at exactly 4 px and resolves the closest controlled folder row', () => {
    const capture = captureNode()
    const { result } = renderHook(() =>
      useOrganizationPointerDrag({
        disabled: false,
        resetKey: 'generation-1',
        isDropTargetValid: () => true,
        onDrop: vi.fn(),
      }),
    )

    act(() => {
      result.current.handlePointerInput(start(capture))
      result.current.handlePointerInput({
        type: 'move',
        pointerId: 7,
        clientX: 54,
        clientY: 50,
      })
    })

    expect(result.current.dragView).toEqual({
      clientX: 54,
      clientY: 50,
      itemCount: 2,
      mode: 'move',
    })
    expect(result.current.dropTarget).toEqual({
      entityId: 'folder-target',
      mode: 'move',
      valid: true,
    })
  })

  it('validates with the ordered entity and mode snapshot from pointer down', () => {
    const ids = ['second', 'first']
    const isDropTargetValid = vi.fn(
      (_entityIds: readonly string[], _destinationId: string, _mode: 'move' | 'copy') => true,
    )
    const capture = captureNode()
    const { result } = renderHook(() =>
      useOrganizationPointerDrag({
        disabled: false,
        resetKey: 'generation-1',
        isDropTargetValid,
        onDrop: vi.fn(),
      }),
    )

    act(() => result.current.handlePointerInput(start(capture, { entityIds: ids, mode: 'copy' })))
    ids.reverse()
    ids.push('late')
    act(() =>
      result.current.handlePointerInput({
        type: 'move',
        pointerId: 7,
        clientX: 54,
        clientY: 50,
      }),
    )

    expect(isDropTargetValid).toHaveBeenCalledWith(['second', 'first'], 'folder-target', 'copy')
    expect(isDropTargetValid.mock.calls[0]?.[0]).not.toBe(ids)
  })

  it('cleans up a valid drag before submitting it exactly once', () => {
    const capture = captureNode()
    const onDrop = vi.fn()
    const { result } = renderHook(() =>
      useOrganizationPointerDrag({
        disabled: false,
        resetKey: 'generation-1',
        isDropTargetValid: () => true,
        onDrop,
      }),
    )
    act(() => {
      result.current.handlePointerInput(start(capture))
      result.current.handlePointerInput({
        type: 'move',
        pointerId: 7,
        clientX: 54,
        clientY: 50,
      })
      result.current.handlePointerInput({
        type: 'end',
        pointerId: 7,
        clientX: 54,
        clientY: 50,
      })
      result.current.handlePointerInput({
        type: 'end',
        pointerId: 7,
        clientX: 54,
        clientY: 50,
      })
    })

    expect(result.current.dragView).toBeNull()
    expect(result.current.dropTarget).toBeNull()
    expect(capture.release).toHaveBeenCalledOnce()
    expect(onDrop).toHaveBeenCalledOnce()
    expect(capture.release.mock.invocationCallOrder[0]).toBeLessThan(
      defined(onDrop.mock.invocationCallOrder[0], 'Expected drop callback invocation order'),
    )
    expect(onDrop).toHaveBeenCalledWith(['entity-2', 'entity-1'], 'folder-target', 'move')
  })

  it('never submits invalid or absent targets', () => {
    const onDrop = vi.fn()
    const { result } = renderHook(() =>
      useOrganizationPointerDrag({
        disabled: false,
        resetKey: 'generation-1',
        isDropTargetValid: () => false,
        onDrop,
      }),
    )

    const invalidCapture = captureNode()
    act(() => {
      result.current.handlePointerInput(start(invalidCapture))
      result.current.handlePointerInput({
        type: 'move',
        pointerId: 7,
        clientX: 54,
        clientY: 50,
      })
    })
    expect(result.current.dropTarget?.valid).toBe(false)
    act(() =>
      result.current.handlePointerInput({
        type: 'end',
        pointerId: 7,
        clientX: 54,
        clientY: 50,
      }),
    )

    hit.mockReturnValue(null)
    const absentCapture = captureNode()
    act(() => {
      result.current.handlePointerInput(start(absentCapture))
      result.current.handlePointerInput({
        type: 'move',
        pointerId: 7,
        clientX: 54,
        clientY: 50,
      })
    })
    expect(result.current.dropTarget).toBeNull()
    act(() =>
      result.current.handlePointerInput({
        type: 'end',
        pointerId: 7,
        clientX: 54,
        clientY: 50,
      }),
    )
    expect(onDrop).not.toHaveBeenCalled()
  })

  it('cancels on Escape, pointercancel, blur, disabled, reset, and unmount', () => {
    const onDrop = vi.fn()
    const isDropTargetValid = vi.fn(() => true)
    const hook = renderHook(
      (props: { disabled: boolean; resetKey: string }) =>
        useOrganizationPointerDrag({ ...props, isDropTargetValid, onDrop }),
      { initialProps: { disabled: false, resetKey: 'generation-1' } },
    )
    const captures: CaptureHarness[] = []
    const activate = () => {
      const capture = captureNode()
      captures.push(capture)
      act(() => {
        hook.result.current.handlePointerInput(start(capture))
        hook.result.current.handlePointerInput({
          type: 'move',
          pointerId: 7,
          clientX: 54,
          clientY: 50,
        })
      })
      expect(hook.result.current.dragView).not.toBeNull()
    }

    activate()
    act(() => window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' })))
    activate()
    act(() => hook.result.current.handlePointerInput({ type: 'cancel', pointerId: 7 }))
    activate()
    act(() => window.dispatchEvent(new Event('blur')))
    activate()
    hook.rerender({ disabled: true, resetKey: 'generation-1' })
    hook.rerender({ disabled: false, resetKey: 'generation-1' })
    activate()
    hook.rerender({ disabled: false, resetKey: 'generation-2' })
    activate()
    hook.unmount()

    expect(onDrop).not.toHaveBeenCalled()
    for (const capture of captures) expect(capture.release).toHaveBeenCalledOnce()
  })

  it('ignores move, end, and cancel input from another pointer', () => {
    const onDrop = vi.fn()
    const capture = captureNode()
    const { result } = renderHook(() =>
      useOrganizationPointerDrag({
        disabled: false,
        resetKey: 'generation-1',
        isDropTargetValid: () => true,
        onDrop,
      }),
    )

    act(() => {
      result.current.handlePointerInput(start(capture))
      result.current.handlePointerInput({
        type: 'move',
        pointerId: 8,
        clientX: 80,
        clientY: 80,
      })
      result.current.handlePointerInput({
        type: 'end',
        pointerId: 8,
        clientX: 80,
        clientY: 80,
      })
      result.current.handlePointerInput({ type: 'cancel', pointerId: 8 })
    })
    expect(result.current.dragView).toBeNull()
    expect(capture.release).not.toHaveBeenCalled()

    act(() => {
      result.current.handlePointerInput({
        type: 'move',
        pointerId: 7,
        clientX: 54,
        clientY: 50,
      })
      result.current.handlePointerInput({
        type: 'end',
        pointerId: 7,
        clientX: 54,
        clientY: 50,
      })
    })
    expect(onDrop).toHaveBeenCalledOnce()
  })

  it('edge-scrolls at most 18 px per frame, re-hit-tests, and stops in the center', () => {
    vi.spyOn(surface, 'getBoundingClientRect').mockReturnValue(rect(10, 110))
    let scrollTop = 40
    Object.defineProperty(surface, 'scrollTop', {
      configurable: true,
      get: () => scrollTop,
      set: (value: number) => (scrollTop = value),
    })
    const secondRow = document.createElement('div')
    secondRow.dataset.organizationFolderId = 'folder-after-scroll'
    surface.append(secondRow)
    const scrollBy = vi.fn(({ top }: ScrollToOptions) => {
      scrollTop += top ?? 0
      hit.mockReturnValue(secondRow)
    })
    Object.defineProperty(surface, 'scrollBy', { configurable: true, value: scrollBy })
    const capture = captureNode()
    const { result } = renderHook(() =>
      useOrganizationPointerDrag({
        disabled: false,
        resetKey: 'generation-1',
        isDropTargetValid: () => true,
        onDrop: vi.fn(),
      }),
    )

    act(() => {
      result.current.handlePointerInput(start(capture))
      result.current.handlePointerInput({
        type: 'move',
        pointerId: 7,
        clientX: 50,
        clientY: 15,
      })
    })
    expect(frameHarness.pending()).toBe(1)
    act(() => frameHarness.flushNext())
    const firstDelta = (scrollBy.mock.calls[0]?.[0] as ScrollToOptions | undefined)?.top ?? 0
    expect(firstDelta).toBeLessThan(0)
    expect(Math.abs(firstDelta)).toBeLessThanOrEqual(18)
    expect(result.current.dropTarget?.entityId).toBe('folder-after-scroll')
    expect(frameHarness.pending()).toBe(1)

    act(() =>
      result.current.handlePointerInput({
        type: 'move',
        pointerId: 7,
        clientX: 50,
        clientY: 60,
      }),
    )
    expect(frameHarness.pending()).toBe(0)
    expect(frameHarness.cancel).toHaveBeenCalled()
  })

  it('stops edge RAF on cancel and when scrolling has reached a boundary', () => {
    vi.spyOn(surface, 'getBoundingClientRect').mockReturnValue(rect(10, 110))
    let scrollTop = 0
    Object.defineProperty(surface, 'scrollTop', {
      configurable: true,
      get: () => scrollTop,
      set: (value: number) => (scrollTop = value),
    })
    const scrollBy = vi.fn(({ top }: ScrollToOptions) => {
      scrollTop = Math.max(0, scrollTop + (top ?? 0))
    })
    Object.defineProperty(surface, 'scrollBy', { configurable: true, value: scrollBy })
    const capture = captureNode()
    const { result } = renderHook(() =>
      useOrganizationPointerDrag({
        disabled: false,
        resetKey: 'generation-1',
        isDropTargetValid: () => true,
        onDrop: vi.fn(),
      }),
    )

    act(() => {
      result.current.handlePointerInput(start(capture))
      result.current.handlePointerInput({
        type: 'move',
        pointerId: 7,
        clientX: 50,
        clientY: 15,
      })
    })
    act(() => frameHarness.flushNext())
    expect(scrollBy).toHaveBeenCalledOnce()
    expect(frameHarness.pending()).toBe(0)

    scrollTop = 40
    act(() =>
      result.current.handlePointerInput({
        type: 'move',
        pointerId: 7,
        clientX: 50,
        clientY: 105,
      }),
    )
    expect(frameHarness.pending()).toBe(1)
    act(() => result.current.cancel())
    expect(frameHarness.pending()).toBe(0)
    expect(frameHarness.cancel).toHaveBeenCalled()
  })

  it('uses rerendered validation and drop callbacks for an existing session', () => {
    const oldDrop = vi.fn()
    const newDrop = vi.fn()
    const oldValidation = vi.fn(() => false)
    const newValidation = vi.fn(() => true)
    const capture = captureNode()
    const hook = renderHook(
      (props: { isDropTargetValid: () => boolean; onDrop: () => void }) =>
        useOrganizationPointerDrag({
          disabled: false,
          resetKey: 'generation-1',
          ...props,
        }),
      {
        initialProps: {
          isDropTargetValid: oldValidation,
          onDrop: oldDrop,
        },
      },
    )

    act(() => hook.result.current.handlePointerInput(start(capture)))
    hook.rerender({ isDropTargetValid: newValidation, onDrop: newDrop })
    act(() => {
      hook.result.current.handlePointerInput({
        type: 'move',
        pointerId: 7,
        clientX: 54,
        clientY: 50,
      })
      hook.result.current.handlePointerInput({
        type: 'end',
        pointerId: 7,
        clientX: 54,
        clientY: 50,
      })
    })

    expect(oldValidation).not.toHaveBeenCalled()
    expect(newValidation).toHaveBeenCalledOnce()
    expect(oldDrop).not.toHaveBeenCalled()
    expect(newDrop).toHaveBeenCalledOnce()
  })

  it('rejects disabled, empty, duplicate, and concurrent starts without capture leaks', () => {
    const { result, rerender } = renderHook(
      ({ disabled }: { disabled: boolean }) =>
        useOrganizationPointerDrag({
          disabled,
          resetKey: 'generation-1',
          isDropTargetValid: () => true,
          onDrop: vi.fn(),
        }),
      { initialProps: { disabled: true } },
    )
    const disabledCapture = captureNode()
    act(() => result.current.handlePointerInput(start(disabledCapture)))
    expect(disabledCapture.set).not.toHaveBeenCalled()

    rerender({ disabled: false })
    const emptyCapture = captureNode()
    const duplicateCapture = captureNode()
    const activeCapture = captureNode()
    const concurrentCapture = captureNode()
    act(() => {
      result.current.handlePointerInput(start(emptyCapture, { entityIds: [] }))
      result.current.handlePointerInput(start(duplicateCapture, { entityIds: ['same', 'same'] }))
      result.current.handlePointerInput(start(activeCapture))
      result.current.handlePointerInput(start(concurrentCapture))
    })
    expect(emptyCapture.set).not.toHaveBeenCalled()
    expect(duplicateCapture.set).not.toHaveBeenCalled()
    expect(activeCapture.set).toHaveBeenCalledOnce()
    expect(concurrentCapture.set).not.toHaveBeenCalled()

    activeCapture.setOwned(false)
    act(() => result.current.cancel())
    expect(activeCapture.release).not.toHaveBeenCalled()
  })
})
