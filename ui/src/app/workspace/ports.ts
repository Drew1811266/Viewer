import type { ViewerBridge } from '../../api/viewer'

export type PreviewDataPort = Pick<
  ViewerBridge,
  'queryFolder' | 'requestImage' | 'previewText' | 'openExternalLink' | 'videoRequestCover'
>

export type VideoPlaybackPort = Pick<
  ViewerBridge,
  | 'listenVideo'
  | 'videoClose'
  | 'videoCancelOpen'
  | 'videoOpen'
  | 'videoPause'
  | 'videoPlay'
  | 'videoRequestCover'
  | 'videoRequestThumbnail'
  | 'videoSeek'
  | 'videoSetFullscreen'
  | 'videoSetMuted'
  | 'videoSetRate'
  | 'videoSetVolume'
  | 'videoStep'
>

export type SettingsPort = Pick<ViewerBridge, 'videoCacheStats' | 'videoCacheClear'>

export type EmptyProjectPort = Pick<
  ViewerBridge,
  'chooseProject' | 'openProject' | 'listenProjectDropEvents'
>

export type WorkspaceShellPort = Pick<ViewerBridge, 'revealProjectInFileManager'>

export interface WorkspacePorts {
  preview: PreviewDataPort
  playback: VideoPlaybackPort
  settings: SettingsPort
  emptyProject: EmptyProjectPort
  shell: WorkspaceShellPort
}

export function createWorkspacePorts(bridge: ViewerBridge): WorkspacePorts {
  return {
    preview: {
      queryFolder: bridge.queryFolder,
      requestImage: bridge.requestImage,
      previewText: bridge.previewText,
      openExternalLink: bridge.openExternalLink,
      videoRequestCover: bridge.videoRequestCover,
    },
    playback: {
      listenVideo: bridge.listenVideo,
      videoClose: bridge.videoClose,
      videoCancelOpen: bridge.videoCancelOpen,
      videoOpen: bridge.videoOpen,
      videoPause: bridge.videoPause,
      videoPlay: bridge.videoPlay,
      videoRequestCover: bridge.videoRequestCover,
      videoRequestThumbnail: bridge.videoRequestThumbnail,
      videoSeek: bridge.videoSeek,
      videoSetFullscreen: bridge.videoSetFullscreen,
      videoSetMuted: bridge.videoSetMuted,
      videoSetRate: bridge.videoSetRate,
      videoSetVolume: bridge.videoSetVolume,
      videoStep: bridge.videoStep,
    },
    settings: {
      videoCacheStats: bridge.videoCacheStats,
      videoCacheClear: bridge.videoCacheClear,
    },
    emptyProject: {
      chooseProject: bridge.chooseProject,
      openProject: bridge.openProject,
      listenProjectDropEvents: bridge.listenProjectDropEvents,
    },
    shell: {
      revealProjectInFileManager: bridge.revealProjectInFileManager,
    },
  }
}
