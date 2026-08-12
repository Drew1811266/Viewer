import { fireEvent, render, screen } from '@testing-library/react'
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
})
