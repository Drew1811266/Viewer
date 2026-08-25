import { describe, expect, expectTypeOf, it } from 'vitest'
import type { BrowserFile } from '../../api/types'
import { tauriViewerBridge } from '../../api/viewer'
import type { OpenPreviewIntent, WorkspaceIntent } from './intents'
import { createWorkspacePorts } from './ports'

function exhaust(intent: WorkspaceIntent): string {
  switch (intent.kind) {
    case 'open-preview':
      return intent.file.entityId
    case 'enter-compare':
      return String(intent.files.length)
    case 'start-rename':
      return String(intent.files.length)
    case 'show-operation-results':
      return intent.batchId
    case 'open-settings':
      return intent.kind
    case 'open-info':
      return intent.kind
    case 'close-project':
      return intent.kind
  }
}

describe('workspace contracts', () => {
  it('keeps preview intents as the exact open-preview member of the closed intent union', () => {
    expectTypeOf<OpenPreviewIntent>().toEqualTypeOf<
      Extract<WorkspaceIntent, { kind: 'open-preview' }>
    >()
    expectTypeOf<OpenPreviewIntent['file']>().toEqualTypeOf<BrowserFile>()
    expect(exhaust({ kind: 'open-settings' })).toBe('open-settings')
  })

  it('creates explicit narrow ports without forwarding the full bridge object', () => {
    const ports = createWorkspacePorts(tauriViewerBridge)

    expect(Object.keys(ports.preview).sort()).toEqual([
      'openExternalLink',
      'previewText',
      'queryFolder',
      'requestImage',
      'videoRequestCover',
    ])
    expect('executeFileCommand' in ports.preview).toBe(false)
    expect(Object.keys(ports.playback).sort()).toEqual([
      'listenVideo',
      'videoCancelOpen',
      'videoClose',
      'videoOpen',
      'videoPause',
      'videoPlay',
      'videoRequestCover',
      'videoRequestThumbnail',
      'videoSeek',
      'videoSetFullscreen',
      'videoSetMuted',
      'videoSetRate',
      'videoSetVolume',
      'videoStep',
    ])
    expect(Object.keys(ports.settings).sort()).toEqual(['videoCacheClear', 'videoCacheStats'])
    expect(Object.keys(ports.emptyProject).sort()).toEqual([
      'chooseProject',
      'listenProjectDropEvents',
      'openProject',
    ])
    expect('closeProject' in ports.emptyProject).toBe(false)
    expect(Object.keys(ports.shell)).toEqual(['revealProjectInFileManager'])
  })
})
