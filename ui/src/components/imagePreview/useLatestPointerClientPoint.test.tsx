import { fireEvent, render } from '@testing-library/react'
import type { MutableRefObject } from 'react'
import { describe, expect, it } from 'vitest'
import type { Point } from './imageGeometry'
import { useLatestPointerClientPoint } from './useLatestPointerClientPoint'

describe('useLatestPointerClientPoint', () => {
  it('tracks pointer activity without rerendering and stops after unmount', () => {
    let observed!: MutableRefObject<Point | null>
    let renderCount = 0

    function Harness() {
      renderCount += 1
      observed = useLatestPointerClientPoint()
      return null
    }

    const view = render(<Harness />)
    fireEvent.pointerMove(window, { clientX: 120, clientY: 80 })
    expect(observed.current).toEqual({ x: 120, y: 80 })
    expect(renderCount).toBe(1)

    fireEvent.pointerDown(window, { clientX: 250, clientY: 160 })
    expect(observed.current).toEqual({ x: 250, y: 160 })
    expect(renderCount).toBe(1)

    view.unmount()
    fireEvent.pointerMove(window, { clientX: 400, clientY: 300 })
    expect(observed.current).toEqual({ x: 250, y: 160 })
  })
})
