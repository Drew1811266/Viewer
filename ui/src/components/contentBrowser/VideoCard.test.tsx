import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { VideoFile } from '../../api/types'
import { VideoCard } from './VideoCard'

function video(overrides: Partial<VideoFile> = {}): VideoFile {
  return {
    entityId: 'video-1',
    relativePath: 'id-001/clip.mp4',
    name: 'clip.mp4',
    kind: 'video',
    size: 120,
    modifiedNs: '1',
    marker: { reviewState: null, favorite: false },
    imageMetadata: null,
    imageUrl: null,
    videoMetadata: {
      durationUs: 62_400_000,
      displayWidth: 1920,
      displayHeight: 1080,
      rotationDegrees: 0,
      frameRateMillihertz: 29_970,
      videoCodec: 'h264',
      audioCodec: 'aac',
      probeStatus: 'ready',
      failureKind: null,
      coverUrl: 'viewer-image://localhost/session/cover',
    },
    ...overrides,
  }
}

describe('VideoCard', () => {
  it('shows only a positive review feedback count', () => {
    const rendered = render(
      <VideoCard
        video={video()}
        selected={false}
        active={false}
        onOpen={vi.fn()}
        feedbackCount={3}
      />,
    )

    expect(screen.getByText('返工 · 3 条')).toBeVisible()
    rendered.rerender(
      <VideoCard
        video={video()}
        selected={false}
        active={false}
        onOpen={vi.fn()}
        feedbackCount={0}
      />,
    )
    expect(screen.queryByText(/返工/)).not.toBeInTheDocument()
  })

  it('keeps a 16:9 stage while its cover fades in and formats duration from microseconds', () => {
    render(<VideoCard video={video()} selected={false} active={false} onOpen={vi.fn()} />)

    const option = screen.getByRole('option', { name: 'clip.mp4' })
    const stage = option.querySelector<HTMLElement>('.video-card-cover-stage')
    const cover = option.querySelector<HTMLImageElement>('.video-card-cover')
    expect(stage).not.toBeNull()
    expect(stage).toHaveStyle({ aspectRatio: '16 / 9' })
    expect(screen.getByText('1:02')).toBeVisible()
    expect(cover).not.toHaveClass('video-card-cover--loaded')

    fireEvent.load(cover as HTMLImageElement)

    expect(option.querySelector('.video-card-cover-stage')).toBe(stage)
    expect(cover).toHaveClass('video-card-cover--loaded')
  })

  it('exposes selected/focus state, failed availability, and opens the entity on double click', () => {
    const onOpen = vi.fn()
    const failed = video({
      videoMetadata: {
        ...video().videoMetadata,
        probeStatus: 'failed',
        failureKind: 'damaged',
        coverUrl: null,
      },
    })
    render(<VideoCard video={failed} selected active onOpen={onOpen} />)

    const option = screen.getByRole('option', { name: 'clip.mp4' })
    expect(option).toHaveAttribute('aria-selected', 'true')
    expect(option).toHaveAttribute('data-active', 'true')
    expect(screen.getByText('不可用')).toBeVisible()

    fireEvent.doubleClick(option)
    expect(onOpen).toHaveBeenCalledWith('video-1')
  })

  it('keeps a playable video available when only its cover thumbnail fails', () => {
    const onOpen = vi.fn()
    render(<VideoCard video={video()} selected={false} active={false} onOpen={onOpen} />)

    const option = screen.getByRole('option', { name: 'clip.mp4' })
    const stage = option.querySelector<HTMLElement>('.video-card-cover-stage')
    const cover = option.querySelector<HTMLImageElement>('.video-card-cover')
    fireEvent.error(cover as HTMLImageElement)

    expect(stage).toHaveAttribute('data-thumbnail-state', 'failed')
    expect(stage).toHaveStyle({ aspectRatio: '16 / 9' })
    expect(screen.getByLabelText('视频缩略图不可用')).toBeVisible()
    expect(screen.queryByText('不可用')).not.toBeInTheDocument()
    fireEvent.doubleClick(option)
    expect(onOpen).toHaveBeenCalledWith('video-1')
  })

  it('requests and displays a generated cover when ready metadata has no cover URL', async () => {
    const requestCover = vi
      .fn<(entityId: string) => Promise<string>>()
      .mockResolvedValue('viewer-image://localhost/session/generated-cover')
    render(
      <VideoCard
        video={video({
          videoMetadata: { ...video().videoMetadata, coverUrl: null },
        })}
        selected={false}
        active={false}
        onOpen={vi.fn()}
        requestCover={requestCover}
      />,
    )

    await waitFor(() => expect(requestCover).toHaveBeenCalledExactlyOnceWith('video-1'))
    expect(
      screen.getByRole('option', { name: 'clip.mp4' }).querySelector('.video-card-cover'),
    ).toHaveAttribute('src', 'viewer-image://localhost/session/generated-cover')
  })
})
