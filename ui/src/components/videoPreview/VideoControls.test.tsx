import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import VideoControls, {
  VIDEO_RATES,
  type VideoControlCommands,
  type VideoControlViewState,
} from './VideoControls'

describe('VideoControls', () => {
  it('offers the approved controls, Chinese names, shortcuts, and exact rates', () => {
    const commands = controlCommands()
    renderControls({ commands })
    const controls = screen.getByRole('group', { name: '视频播放控制' })

    expect(within(controls).getByRole('button', { name: '播放' })).toHaveAttribute(
      'aria-keyshortcuts',
      'Space',
    )
    expect(within(controls).getByRole('button', { name: '播放' })).toHaveClass(
      'video-controls__play',
    )
    expect(within(controls).getByRole('button', { name: '上一帧' })).toHaveAttribute(
      'aria-keyshortcuts',
      'ArrowLeft',
    )
    expect(within(controls).getByRole('button', { name: '下一帧' })).toHaveAttribute(
      'aria-keyshortcuts',
      'ArrowRight',
    )
    expect(within(controls).getByRole('button', { name: '静音' })).toHaveAttribute(
      'aria-keyshortcuts',
      'M',
    )
    expect(within(controls).getByRole('button', { name: '进入全屏' })).toHaveAttribute(
      'aria-keyshortcuts',
      'F',
    )
    expect(VIDEO_RATES).toEqual([0.5, 0.75, 1, 1.25, 1.5, 2])
    expect(
      within(controls)
        .getAllByRole('option')
        .map((option) => option.getAttribute('value')),
    ).toEqual(['0.5', '0.75', '1', '1.25', '1.5', '2'])
  })

  it('maps every control to one generation-bound command', () => {
    const commands = controlCommands()
    renderControls({ commands })

    fireEvent.click(screen.getByRole('button', { name: '播放' }))
    fireEvent.click(screen.getByRole('button', { name: '上一帧' }))
    fireEvent.click(screen.getByRole('button', { name: '下一帧' }))
    fireEvent.click(screen.getByRole('button', { name: '静音' }))
    fireEvent.change(screen.getByRole('slider', { name: '音量' }), { target: { value: '64' } })
    fireEvent.change(screen.getByRole('combobox', { name: '播放速度' }), {
      target: { value: '1.5' },
    })
    fireEvent.click(screen.getByRole('button', { name: '进入全屏' }))

    expect(commands.togglePlayback).toHaveBeenCalledOnce()
    expect(commands.step.mock.calls).toEqual([['backward'], ['forward']])
    expect(commands.toggleMuted).toHaveBeenCalledOnce()
    expect(commands.setVolume).toHaveBeenCalledWith(64)
    expect(commands.setRate).toHaveBeenCalledWith(1.5)
    expect(commands.toggleFullscreen).toHaveBeenCalledOnce()
  })

  it('renders ended playback as paused at the final time', () => {
    renderControls({
      view: { ...CONTROL_VIEW, phase: 'ended', timeUs: 7_000_000, durationUs: 8_000_000 },
    })

    expect(screen.getByRole('button', { name: '播放' })).toBeVisible()
    expect(screen.getByRole('slider', { name: '视频时间轴' })).toHaveAttribute('aria-valuenow', '8')
    expect(screen.getByText('00:08 / 00:08')).toBeVisible()
  })

  it('marks hidden and reduced-motion state without removing focused controls', () => {
    renderControls({ visible: false, reducedMotion: true })
    const controls = screen.getByRole('group', { name: '视频播放控制' })
    expect(controls).toHaveAttribute('data-visible', 'false')
    expect(controls).toHaveAttribute('data-reduced-motion', 'true')
    expect(screen.getByRole('button', { name: '播放' })).toBeInTheDocument()
  })
})

const CONTROL_VIEW: VideoControlViewState = {
  generation: 7,
  phase: 'paused',
  timeUs: 2_000_000,
  durationUs: 8_000_000,
  volumePercent: 100,
  muted: false,
  rate: 1,
  fullscreen: false,
  error: null,
  timelineThumbnail: null,
}

function renderControls({
  view = CONTROL_VIEW,
  commands = controlCommands(),
  visible = true,
  reducedMotion = false,
}: {
  view?: VideoControlViewState
  commands?: ReturnType<typeof controlCommands>
  visible?: boolean
  reducedMotion?: boolean
} = {}) {
  return render(
    <VideoControls
      view={view}
      commands={commands}
      visible={visible}
      reducedMotion={reducedMotion}
      onActivity={() => undefined}
      onSeekingChange={() => undefined}
      onAdjustingChange={() => undefined}
    />,
  )
}

function controlCommands() {
  return {
    play: vi.fn<VideoControlCommands['play']>().mockResolvedValue(undefined),
    pause: vi.fn<VideoControlCommands['pause']>().mockResolvedValue(undefined),
    togglePlayback: vi.fn<VideoControlCommands['togglePlayback']>().mockResolvedValue(undefined),
    step: vi.fn<VideoControlCommands['step']>().mockResolvedValue(undefined),
    seek: vi.fn<VideoControlCommands['seek']>().mockResolvedValue(undefined),
    setVolume: vi.fn<VideoControlCommands['setVolume']>().mockResolvedValue(undefined),
    setMuted: vi.fn<VideoControlCommands['setMuted']>().mockResolvedValue(undefined),
    toggleMuted: vi.fn<VideoControlCommands['toggleMuted']>().mockResolvedValue(undefined),
    setRate: vi.fn<VideoControlCommands['setRate']>().mockResolvedValue(undefined),
    setFullscreen: vi.fn<VideoControlCommands['setFullscreen']>().mockResolvedValue(undefined),
    toggleFullscreen: vi
      .fn<VideoControlCommands['toggleFullscreen']>()
      .mockResolvedValue(undefined),
    requestThumbnail: vi
      .fn<VideoControlCommands['requestThumbnail']>()
      .mockResolvedValue(undefined),
  }
}
