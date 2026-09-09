import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())
const listen = vi.hoisted(() => vi.fn())
const onDragDropEvent = vi.hoisted(() => vi.fn())

vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen }))
vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: () => ({ onDragDropEvent }),
}))

import { tauriViewerBridge } from './viewer'

beforeEach(() => {
  invoke.mockReset()
  listen.mockReset()
  onDragDropEvent.mockReset()
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('tauriViewerBridge', () => {
  it('exposes one typed image render command and one semantic event transport', async () => {
    const handler = vi.fn()
    const unlisten = vi.fn()
    let receive: ((event: { payload: unknown }) => void) | undefined
    listen.mockImplementation(async (_event, eventHandler) => {
      receive = eventHandler
      return unlisten
    })
    const command = {
      sessionId: 41,
      assetGeneration: 1,
      sceneRevision: 0,
      commandId: 1,
      command: {
        type: 'open' as const,
        entityId: '00000000-0000-4000-8000-000000000007',
      },
    }

    await tauriViewerBridge.imageRenderCommand(command)
    const stop = await tauriViewerBridge.listenImageRender(handler)

    expect(invoke).toHaveBeenCalledWith('image_render_command', { command })
    expect(listen).toHaveBeenCalledWith('viewer://image-render', expect.any(Function))
    receive?.({
      payload: {
        type: 'ready',
        sessionId: 41,
        assetGeneration: 1,
        width: 900,
        height: 600,
      },
    })
    expect(handler).toHaveBeenCalledWith({
      type: 'ready',
      sessionId: '41',
      assetGeneration: 1,
      width: 900,
      height: 600,
    })
    stop()
    expect(unlisten).toHaveBeenCalledOnce()
  })

  it('maps every review operation and the progress event to the native contract', async () => {
    const unlisten = vi.fn()
    let receive: ((event: { payload: unknown }) => void) | undefined
    listen.mockImplementation(async (_event, handler) => {
      receive = handler
      return unlisten
    })
    const progress = vi.fn()
    const session = { sessionId: 'session-1', generation: 7 }
    const guard = { ...session, reviewRoundId: 'round-1', expectedRevision: 4 }

    await tauriViewerBridge.reviewStatus(session)
    await tauriViewerBridge.reviewPreviewStart({
      ...session,
      scope: { kind: 'selection', entityIds: ['image-1', 'video-1'] },
    })
    await tauriViewerBridge.reviewStart({ ...session, proposalId: 11 })
    await tauriViewerBridge.reviewStartWithFeedback({
      ...session,
      proposalId: 11,
      text: '修正衣领边缘',
      targets: [
        {
          entityId: 'image-1',
          anchor: { kind: 'image_rect', x: 0.2, y: 0.1, width: 0.3, height: 0.2 },
        },
      ],
    })
    await tauriViewerBridge.reviewResume(session)
    await tauriViewerBridge.reviewAddFeedback({
      ...guard,
      text: '降低高光强度',
      targets: [{ entityId: 'image-1', anchor: { kind: 'asset' } }],
    })
    await tauriViewerBridge.reviewUpdateFeedback({
      ...guard,
      feedbackId: 'feedback-1',
      text: '进一步降低高光强度',
      targets: [{ entityId: 'image-1', anchor: { kind: 'asset' } }],
    })
    await tauriViewerBridge.reviewUpdateFeedbackText({
      ...guard,
      feedbackId: 'feedback-1',
      text: '只调整文字',
    })
    await tauriViewerBridge.reviewReplaceFeedbackAnchor({
      ...guard,
      feedbackId: 'feedback-1',
      target: {
        entityId: 'image-1',
        anchor: {
          kind: 'image_stroke',
          points: [
            { x: 0.2, y: 0.3 },
            { x: 0.7, y: 0.6 },
          ],
        },
      },
    })
    await tauriViewerBridge.reviewReplaceFeedbackAnchor({
      ...guard,
      feedbackId: 'feedback-1',
      target: {
        entityId: 'image-1',
        anchor: {
          kind: 'image_arrow',
          tail: { x: 0.2, y: 0.3 },
          head: { x: 0.7, y: 0.6 },
        },
      },
    })
    await tauriViewerBridge.reviewDeleteFeedback({ ...guard, feedbackId: 'feedback-1' })
    await tauriViewerBridge.reviewRestoreDeletedFeedback({ ...guard, feedbackId: 'feedback-1' })
    await tauriViewerBridge.reviewCompletionSummary(guard)
    await tauriViewerBridge.reviewComplete({ ...guard, proposalId: 12 })
    await tauriViewerBridge.reviewAbandon(guard)
    await tauriViewerBridge.reviewCancelTask(session)
    const stop = await tauriViewerBridge.listenReviewProgress(progress)

    expect(invoke.mock.calls).toEqual([
      ['review_status', { request: session }],
      [
        'review_preview_start',
        {
          request: {
            ...session,
            scope: { kind: 'selection', entityIds: ['image-1', 'video-1'] },
          },
        },
      ],
      ['review_start', { request: { ...session, proposalId: 11 } }],
      [
        'review_start_with_feedback',
        {
          request: {
            ...session,
            proposalId: 11,
            text: '修正衣领边缘',
            targets: [
              {
                entityId: 'image-1',
                anchor: { kind: 'image_rect', x: 0.2, y: 0.1, width: 0.3, height: 0.2 },
              },
            ],
          },
        },
      ],
      ['review_resume', { request: session }],
      [
        'review_add_feedback',
        {
          request: {
            ...guard,
            text: '降低高光强度',
            targets: [{ entityId: 'image-1', anchor: { kind: 'asset' } }],
          },
        },
      ],
      [
        'review_update_feedback',
        {
          request: {
            ...guard,
            feedbackId: 'feedback-1',
            text: '进一步降低高光强度',
            targets: [{ entityId: 'image-1', anchor: { kind: 'asset' } }],
          },
        },
      ],
      [
        'review_update_feedback_text',
        {
          request: { ...guard, feedbackId: 'feedback-1', text: '只调整文字' },
        },
      ],
      [
        'review_replace_feedback_anchor',
        {
          request: {
            ...guard,
            feedbackId: 'feedback-1',
            target: {
              entityId: 'image-1',
              anchor: {
                kind: 'image_stroke',
                points: [
                  { x: 0.2, y: 0.3 },
                  { x: 0.7, y: 0.6 },
                ],
              },
            },
          },
        },
      ],
      [
        'review_replace_feedback_anchor',
        {
          request: {
            ...guard,
            feedbackId: 'feedback-1',
            target: {
              entityId: 'image-1',
              anchor: {
                kind: 'image_arrow',
                tail: { x: 0.2, y: 0.3 },
                head: { x: 0.7, y: 0.6 },
              },
            },
          },
        },
      ],
      ['review_delete_feedback', { request: { ...guard, feedbackId: 'feedback-1' } }],
      ['review_restore_deleted_feedback', { request: { ...guard, feedbackId: 'feedback-1' } }],
      ['review_completion_summary', { request: guard }],
      ['review_complete', { request: { ...guard, proposalId: 12 } }],
      ['review_abandon', { request: guard }],
      ['review_cancel_task', { request: session }],
    ])
    const reviewPayloads = invoke.mock.calls
      .filter(([command]) => String(command).startsWith('review_'))
      .map(([, payload]) => payload)
    expect(JSON.stringify(reviewPayloads)).not.toMatch(
      /sourcePath|artifactPath|assetVersion|digest|\/Users\//,
    )
    expect(listen).toHaveBeenCalledWith('viewer://review-progress', expect.any(Function))

    const event = {
      sessionId: 'session-1',
      generation: 7,
      taskKind: 'start',
      completed: 2,
      total: 3,
      cancellable: true,
    }
    receive?.({ payload: event })
    expect(progress).toHaveBeenCalledWith(event)
    stop()
    expect(unlisten).toHaveBeenCalledOnce()
  })

  it('maps every video operation and event to its one matching native contract', async () => {
    const handler = vi.fn()

    await tauriViewerBridge.videoOpen({
      attemptId: '00000000-0000-4000-8000-000000000001',
      entityId: 'video-1',
    })
    await tauriViewerBridge.videoCancelOpen({
      attemptId: '00000000-0000-4000-8000-000000000001',
    })
    await tauriViewerBridge.videoClose({ generation: 8 })
    await tauriViewerBridge.videoPlay({ generation: 8 })
    await tauriViewerBridge.videoPause({ generation: 8 })
    await tauriViewerBridge.videoSeek({
      generation: 8,
      requestId: 12,
      timeUs: 2_500_000,
      intent: 'preview',
    })
    await tauriViewerBridge.videoStep({ generation: 8, direction: 'forward' })
    await tauriViewerBridge.videoSetVolume({ generation: 8, volumePercent: 64 })
    await tauriViewerBridge.videoSetMuted({ generation: 8, muted: true })
    await tauriViewerBridge.videoSetRate({ generation: 8, rate: 'one_and_half' })
    await tauriViewerBridge.videoSetFullscreen({ generation: 8, fullscreen: true })
    await tauriViewerBridge.videoRequestThumbnail({
      generation: 8,
      requestId: 'thumbnail-1',
      timeUs: 3_000_000,
    })
    await tauriViewerBridge.videoRequestCover('video-1')
    await tauriViewerBridge.videoCacheStats()
    await tauriViewerBridge.videoCacheClear()
    await tauriViewerBridge.listenVideo(handler)

    expect(invoke.mock.calls).toEqual([
      [
        'video_open',
        {
          request: {
            attemptId: '00000000-0000-4000-8000-000000000001',
            entityId: 'video-1',
          },
        },
      ],
      ['video_cancel_open', { request: { attemptId: '00000000-0000-4000-8000-000000000001' } }],
      ['video_close', { request: { generation: 8 } }],
      ['video_play', { request: { generation: 8 } }],
      ['video_pause', { request: { generation: 8 } }],
      [
        'video_seek',
        {
          request: { generation: 8, requestId: 12, timeUs: 2_500_000, intent: 'preview' },
        },
      ],
      ['video_step', { request: { generation: 8, direction: 'forward' } }],
      ['video_set_volume', { request: { generation: 8, volumePercent: 64 } }],
      ['video_set_muted', { request: { generation: 8, muted: true } }],
      ['video_set_rate', { request: { generation: 8, rate: 'one_and_half' } }],
      ['video_set_fullscreen', { request: { generation: 8, fullscreen: true } }],
      [
        'video_request_thumbnail',
        { request: { generation: 8, requestId: 'thumbnail-1', timeUs: 3_000_000 } },
      ],
      ['video_request_cover', { entityId: 'video-1' }],
      ['video_cache_stats'],
      ['video_cache_clear'],
    ])
    expect('videoSetSurfaceRect' in tauriViewerBridge).toBe(false)
    expect(listen).toHaveBeenCalledWith('viewer://video-event', expect.any(Function))
  })

  it('forwards the complete native project drag lifecycle', async () => {
    let receive:
      | ((event: {
          payload:
            | { type: 'enter'; paths: string[]; position: { x: number; y: number } }
            | { type: 'over'; position: { x: number; y: number } }
            | { type: 'drop'; paths: string[]; position: { x: number; y: number } }
            | { type: 'leave' }
        }) => void)
      | undefined
    const unlisten = vi.fn()
    onDragDropEvent.mockImplementation(async (handler) => {
      receive = handler
      return unlisten
    })
    const handler = vi.fn()

    await tauriViewerBridge.listenProjectDropEvents(handler)
    receive?.({ payload: { type: 'enter', paths: ['/fixture/project'], position: { x: 1, y: 2 } } })
    receive?.({ payload: { type: 'over', position: { x: 2, y: 3 } } })
    receive?.({ payload: { type: 'drop', paths: ['/fixture/project'], position: { x: 3, y: 4 } } })
    receive?.({ payload: { type: 'leave' } })

    expect(handler.mock.calls.map(([event]) => event)).toEqual([
      { type: 'enter', paths: ['/fixture/project'] },
      { type: 'over' },
      { type: 'drop', paths: ['/fixture/project'] },
      { type: 'leave' },
    ])
  })

  it('gets the viewer settings without arguments', async () => {
    await tauriViewerBridge.getViewerSettings()

    expect(invoke).toHaveBeenCalledWith('get_viewer_settings')
  })

  it('updates the complete viewer settings atomically', async () => {
    await tauriViewerBridge.updateViewerSettings({
      thumbnailDensity: 'large',
      magnifier: { shape: 'rounded_rectangle', magnification: 2, area: 'medium' },
    })

    expect(invoke).toHaveBeenCalledWith('update_viewer_settings', {
      settings: {
        thumbnailDensity: 'large',
        magnifier: { shape: 'rounded_rectangle', magnification: 2, area: 'medium' },
      },
    })
  })

  it('rejects an aborted image request immediately and cancels its native request', async () => {
    vi.stubGlobal('crypto', { randomUUID: () => 'image-request-1' })
    const nativeRequest = new Promise(() => undefined)
    invoke.mockImplementation((command: string) => {
      if (command === 'request_image_representation') return nativeRequest
      if (command === 'cancel_image_request') return Promise.resolve(true)
      throw new Error(`Unexpected command: ${command}`)
    })
    const controller = new AbortController()

    const result = tauriViewerBridge.requestImage(
      {
        entityId: 'image-1',
        representation: { kind: 'original100_percent' },
      },
      controller.signal,
    )
    controller.abort()

    await expect(result).rejects.toMatchObject({ name: 'AbortError' })
    expect(invoke).toHaveBeenCalledWith('request_image_representation', {
      entityId: 'image-1',
      representation: { kind: 'original100_percent' },
      requestId: 'image-request-1',
    })
    expect(invoke).toHaveBeenCalledWith('cancel_image_request', {
      requestId: 'image-request-1',
    })
  })

  it('does not invoke native image rendering when the signal is already aborted', async () => {
    const controller = new AbortController()
    controller.abort()

    const result = tauriViewerBridge.requestImage(
      {
        entityId: 'image-1',
        representation: { kind: 'original100_percent' },
      },
      controller.signal,
    )

    await expect(result).rejects.toMatchObject({ name: 'AbortError' })
    expect(invoke).not.toHaveBeenCalled()
  })
})
