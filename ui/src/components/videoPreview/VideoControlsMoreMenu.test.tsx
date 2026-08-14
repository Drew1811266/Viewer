import { fireEvent, render, screen, within } from '@testing-library/react'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import type { VideoControlCommands, VideoControlViewState } from './VideoControls'
import VideoControlsMoreMenu from './VideoControlsMoreMenu'

describe('VideoControlsMoreMenu', () => {
  it('keeps secondary playback controls in one named compact panel', () => {
    const commands = compactCommands()
    renderMenu({ commands })

    const trigger = screen.getByRole('button', { name: '更多播放控制' })
    expect(trigger).toHaveAttribute('aria-expanded', 'false')
    fireEvent.click(trigger)

    const panel = screen.getByRole('region', { name: '更多播放控制' })
    expect(trigger).toHaveAttribute('aria-expanded', 'true')
    expect(within(panel).getByRole('button', { name: '上一帧' })).toBeVisible()
    expect(within(panel).getByRole('button', { name: '下一帧' })).toBeVisible()
    expect(within(panel).getByRole('slider', { name: '音量' })).toHaveValue('72')
    expect(within(panel).getByRole('combobox', { name: '播放速度' })).toHaveValue('1.25')

    fireEvent.click(within(panel).getByRole('button', { name: '上一帧' }))
    fireEvent.click(within(panel).getByRole('button', { name: '下一帧' }))
    fireEvent.change(within(panel).getByRole('slider', { name: '音量' }), {
      target: { value: '46' },
    })
    fireEvent.change(within(panel).getByRole('combobox', { name: '播放速度' }), {
      target: { value: '1.5' },
    })

    expect(commands.step.mock.calls).toEqual([['backward'], ['forward']])
    expect(commands.setVolume).toHaveBeenCalledWith(46)
    expect(commands.setRate).toHaveBeenCalledWith(1.5)
  })

  it('holds idle visibility while open and restores focus on Escape', () => {
    const onAdjustingChange = vi.fn()
    renderMenu({ onAdjustingChange })
    const trigger = screen.getByRole('button', { name: '更多播放控制' })

    fireEvent.click(trigger)
    expect(onAdjustingChange).toHaveBeenLastCalledWith(true)
    fireEvent.keyDown(document, { key: 'Escape' })

    expect(trigger).toHaveAttribute('aria-expanded', 'false')
    expect(trigger).toHaveFocus()
    expect(onAdjustingChange).toHaveBeenLastCalledWith(false)
  })
})

function renderMenu({
  commands = compactCommands(),
  onAdjustingChange = vi.fn(),
}: {
  commands?: ReturnType<typeof compactCommands>
  onAdjustingChange?: (adjusting: boolean) => void
} = {}) {
  function MenuHarness() {
    const [open, setOpen] = useState(false)
    return (
      <VideoControlsMoreMenu
        open={open}
        onOpenChange={setOpen}
        disabled={false}
        view={{ volumePercent: 72, rate: 1.25 }}
        commands={commands}
        rates={[0.5, 0.75, 1, 1.25, 1.5, 2]}
        onActivity={() => undefined}
        onAdjustingChange={onAdjustingChange}
      />
    )
  }
  return render(<MenuHarness />)
}

function compactCommands() {
  return {
    step: vi.fn<VideoControlCommands['step']>().mockResolvedValue(undefined),
    setVolume: vi.fn<VideoControlCommands['setVolume']>().mockResolvedValue(undefined),
    setRate: vi.fn<VideoControlCommands['setRate']>().mockResolvedValue(undefined),
  } satisfies Pick<VideoControlCommands, 'step' | 'setVolume' | 'setRate'>
}

const _viewContract: Pick<VideoControlViewState, 'volumePercent' | 'rate'> = {
  volumePercent: 72,
  rate: 1.25,
}
void _viewContract
