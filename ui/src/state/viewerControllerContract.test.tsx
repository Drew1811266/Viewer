import { describe, expectTypeOf, it } from 'vitest'
import type { ViewerController } from './useViewerController'

type ExpectedCommands = Pick<
  ViewerController,
  | 'openProject'
  | 'closeProject'
  | 'reselectProject'
  | 'selectFolder'
  | 'showAllDescendants'
  | 'cancelTask'
  | 'setSearchText'
  | 'setSearchScope'
  | 'setSearchFilters'
  | 'setSearchSort'
  | 'setSearchLayout'
  | 'removeSearchFilter'
  | 'clearSearchFilters'
  | 'setVisibleSearchHits'
  | 'setSearchPage'
  | 'returnToFolderContext'
  | 'setSelectedEntityIds'
  | 'setReviewState'
  | 'toggleFavorite'
  | 'previewRename'
  | 'preflightFileCommand'
  | 'executeFileCommand'
  | 'cancelOperation'
  | 'loadOperationResults'
  | 'undoLastOperation'
  | 'beginFinderDrag'
  | 'setPreviewEntityId'
  | 'setCompareEntityIds'
  | 'consumeContextRepair'
  | 'clearCloseBlocked'
  | 'openPermissionSettings'
>

describe('ViewerController facade', () => {
  it('retains the state and command contract used by App', () => {
    expectTypeOf<ViewerController>().toHaveProperty('state')
    expectTypeOf<ViewerController>().toMatchTypeOf<
      ExpectedCommands & { state: ViewerController['state'] }
    >()
  })
})
