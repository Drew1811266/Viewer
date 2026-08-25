import type { ViewerController } from '../../state/useViewerController'

export type OrganizationCommands = Pick<
  ViewerController,
  | 'setSelectedEntityIds'
  | 'setReviewState'
  | 'toggleFavorite'
  | 'previewRename'
  | 'preflightFileCommand'
  | 'executeFileCommand'
  | 'cancelOperation'
  | 'loadOperationResults'
  | 'undoLastOperation'
  | 'consumeContextRepair'
  | 'beginFinderDrag'
>

export type OrganizationFileCommandKind = 'rename' | 'copy' | 'move' | 'trash'
