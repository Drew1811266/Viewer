import type { BrowserFile } from '../../api/types'

export type WorkspaceIntent =
  | {
      kind: 'open-preview'
      file: BrowserFile
      files: readonly BrowserFile[] | null
      folderOverviewIdentity: string | null
    }
  | { kind: 'enter-compare'; files: readonly BrowserFile[] }
  | { kind: 'start-rename'; files: readonly BrowserFile[] }
  | { kind: 'show-operation-results'; batchId: string }
  | { kind: 'open-settings' }
  | { kind: 'open-info' }
  | { kind: 'close-project' }

export type OpenPreviewIntent = Extract<WorkspaceIntent, { kind: 'open-preview' }>

export type WorkspaceIntentSink = (intent: WorkspaceIntent) => void
