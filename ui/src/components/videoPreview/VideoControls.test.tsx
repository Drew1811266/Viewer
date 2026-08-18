import { fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import VideoControls, {
  VIDEO_RATES,
  type VideoControlCommands,
  type VideoControlViewState,
} from './VideoControls'

describe('VideoControls', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

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

  it('exposes the current playback state on the primary transport control', () => {
    const rendered = renderControls()
    expect(screen.getByRole('button', { name: '播放' })).toHaveAttribute('aria-pressed', 'false')

    rendered.rerender(
      <VideoControls
        view={{ ...CONTROL_VIEW, phase: 'playing' }}
        commands={controlCommands()}
        visible
        reducedMotion={false}
        onActivity={() => undefined}
        onSeekingChange={() => undefined}
        onAdjustingChange={() => undefined}
      />,
    )

    expect(screen.getByRole('button', { name: '暂停' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('marks hidden and reduced-motion state without removing focused controls', () => {
    renderControls({ visible: false, reducedMotion: true })
    const controls = screen.getByRole('group', { name: '视频播放控制' })
    expect(controls).toHaveAttribute('data-visible', 'false')
    expect(controls).toHaveAttribute('data-reduced-motion', 'true')
    expect(screen.getByRole('button', { name: '播放' })).toBeInTheDocument()
  })

  it('keeps all controls inline in the wide player', () => {
    stubCompactViewport(false)
    renderControls()

    expect(screen.queryByRole('button', { name: '更多播放控制' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '上一帧' })).toBeVisible()
    expect(screen.getByRole('button', { name: '下一帧' })).toBeVisible()
    expect(screen.getByRole('slider', { name: '音量' })).toBeVisible()
    expect(screen.getByRole('combobox', { name: '播放速度' })).toBeVisible()
  })

  it('keeps primary controls visible and moves secondary controls into More when compact', () => {
    stubCompactViewport(true)
    renderControls()

    expect(screen.getByRole('button', { name: '播放' })).toBeVisible()
    expect(screen.getByRole('button', { name: '静音' })).toBeVisible()
    expect(screen.getByRole('button', { name: '进入全屏' })).toBeVisible()
    expect(screen.queryByRole('button', { name: '上一帧' })).not.toBeInTheDocument()
    expect(screen.queryByRole('slider', { name: '音量' })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '更多播放控制' }))
    const panel = screen.getByRole('region', { name: '更多播放控制' })
    expect(within(panel).getByRole('button', { name: '上一帧' })).toBeVisible()
    expect(within(panel).getByRole('button', { name: '下一帧' })).toBeVisible()
    expect(within(panel).getByRole('slider', { name: '音量' })).toBeVisible()
    expect(within(panel).getByRole('combobox', { name: '播放速度' })).toBeVisible()
  })

  it('keeps transport, timeline, and settings as three non-overlapping columns', () => {
    stubCompactViewport(true)
    renderControls()

    const controls = screen.getByRole('group', { name: '视频播放控制' })
    const directChildren = Array.from(controls.children)

    expect(directChildren.map((child) => child.className)).toEqual([
      'video-controls__transport',
      'video-controls__timeline',
      'video-controls__settings',
    ])
    expect(directChildren[1]).toContainElement(screen.getByRole('slider', { name: '视频时间轴' }))
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
    previewSeek: vi.fn<VideoControlCommands['previewSeek']>().mockResolvedValue(undefined),
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

function stubCompactViewport(compact: boolean) {
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({
      matches: compact,
      media: '(max-width: 1099px)',
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  )
}
