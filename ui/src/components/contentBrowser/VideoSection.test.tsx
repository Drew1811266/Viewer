import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { VideoFile } from '../../api/types'
import { VideoSection } from './VideoSection'

function video(): VideoFile {
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
      durationUs: 5_000_000,
      displayWidth: 1280,
      displayHeight: 720,
      rotationDegrees: 0,
      frameRateMillihertz: 24_000,
      videoCodec: 'h264',
      audioCodec: null,
      probeStatus: 'ready',
      failureKind: null,
      coverUrl: null,
    },
  }
}

describe('VideoSection', () => {
  it('omits its disclosure when no videos exist', () => {
    render(
      <VideoSection
        videos={[]}
        expanded
        onExpandedChange={vi.fn()}
        selection={new Set()}
        activeId={null}
        onOpen={vi.fn()}
      />,
    )

    expect(screen.queryByText(/视频 ·/)).not.toBeInTheDocument()
  })

  it('renders stable selected cards under an expanded disclosure and opens by entity id', () => {
    const onExpandedChange = vi.fn()
    const onOpen = vi.fn()
    render(
      <VideoSection
        videos={[video()]}
        expanded
        onExpandedChange={onExpandedChange}
        selection={new Set(['video-1'])}
        activeId="video-1"
        onOpen={onOpen}
      />,
    )

    expect(screen.getByText('视频 · 1')).toBeVisible()
    const disclosure = screen.getByRole('button', { name: '收起视频' })
    expect(disclosure).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('option', { name: 'clip.mp4' })).toHaveAttribute(
      'aria-selected',
      'true',
    )
    fireEvent.doubleClick(screen.getByRole('option', { name: 'clip.mp4' }))
    expect(onOpen).toHaveBeenCalledWith('video-1')

    fireEvent.click(disclosure)
    expect(onExpandedChange).toHaveBeenCalledWith(false)
  })
})
