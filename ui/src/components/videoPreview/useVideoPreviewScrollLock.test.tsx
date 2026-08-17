import { render } from '@testing-library/react'
import { useRef } from 'react'
import { describe, expect, it } from 'vitest'
import { useVideoPreviewScrollLock } from './useVideoPreviewScrollLock'

describe('useVideoPreviewScrollLock', () => {
  it('cancels horizontal and vertical wheel gestures without consuming pointer input', () => {
    const { getByTestId } = render(<ScrollLockHarness />)
    const root = getByTestId('scroll-lock-root')
    const wheel = new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      deltaX: 48,
      deltaY: 64,
    })

    expect(root.dispatchEvent(wheel)).toBe(false)
    expect(wheel.defaultPrevented).toBe(true)

    const pointer = new PointerEvent('pointerdown', { bubbles: true, cancelable: true })
    expect(root.dispatchEvent(pointer)).toBe(true)
  })
})

function ScrollLockHarness() {
  const root = useRef<HTMLElement>(null)
  useVideoPreviewScrollLock(root)
  return <section ref={root} data-testid="scroll-lock-root" />
}
