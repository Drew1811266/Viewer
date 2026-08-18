import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import VideoRateMenu from './VideoRateMenu'

const RATES = [0.5, 0.75, 1, 1.25, 1.5, 2] as const

describe('VideoRateMenu', () => {
  it('selects a supported rate from one rounded listbox and closes it', () => {
    const onSelect = vi.fn()
    render(<VideoRateMenu rate={1} rates={RATES} disabled={false} onSelect={onSelect} />)

    const trigger = screen.getByRole('button', { name: '播放速度，当前 1×' })
    expect(trigger).toHaveAttribute('aria-haspopup', 'listbox')
    expect(trigger).toHaveAttribute('aria-expanded', 'false')

    fireEvent.click(trigger)
    const listbox = screen.getByRole('listbox', { name: '选择播放速度' })
    expect(trigger).toHaveAttribute('aria-expanded', 'true')
    expect(
      within(listbox)
        .getAllByRole('option')
        .map((option) => option.textContent?.trim()),
    ).toEqual(['0.5×', '0.75×', '1×', '1.25×', '1.5×', '2×'])
    expect(within(listbox).getByRole('option', { name: '1×' })).toHaveAttribute(
      'aria-selected',
      'true',
    )

    fireEvent.click(within(listbox).getByRole('option', { name: '1.5×' }))

    expect(onSelect).toHaveBeenCalledOnce()
    expect(onSelect).toHaveBeenCalledWith(1.5)
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('listbox', { name: '选择播放速度' })).not.toBeInTheDocument()
  })

  it('moves through options with arrow keys and restores trigger focus on Escape', () => {
    render(<VideoRateMenu rate={1} rates={RATES} disabled={false} onSelect={() => undefined} />)
    const trigger = screen.getByRole('button', { name: '播放速度，当前 1×' })

    trigger.focus()
    fireEvent.keyDown(trigger, { key: 'ArrowDown' })
    expect(screen.getByRole('option', { name: '1.25×' })).toHaveFocus()

    fireEvent.keyDown(screen.getByRole('option', { name: '1.25×' }), { key: 'End' })
    expect(screen.getByRole('option', { name: '2×' })).toHaveFocus()

    fireEvent.keyDown(document, { key: 'Escape' })
    expect(screen.queryByRole('listbox', { name: '选择播放速度' })).not.toBeInTheDocument()
    expect(trigger).toHaveFocus()
  })
})
