import { useCallback, useEffect, useMemo, useState } from 'react'
import type { BrowserFile } from '../../api/types'
import {
  compareEntryAvailability,
  compareValidationMessage,
  validateCompareCandidates,
} from '../../state/comparePolicy'
import type { ViewerController } from '../../state/useViewerController'
import type { ViewerState } from '../../state/viewerState'
import { resolveCompareFiles } from './viewingModel'

type CompareCommands = Pick<ViewerController, 'setCompareEntityIds'>

export interface CompareCoordinator {
  compareFiles: BrowserFile[]
  compareOpen: boolean
  compareStatus: string | null
  enterCompare(files: readonly BrowserFile[], operationBusy: boolean): void
  changeComparedEntities(entityIds: string[]): void
  setCompareStatus(message: string | null): void
}

export function useCompareCoordinator({
  state,
  projectSessionId,
  commands,
  closePreview,
  openPreview,
}: {
  state: ViewerState
  projectSessionId: string
  commands: CompareCommands
  closePreview(): void
  openPreview(file: BrowserFile): void
}): CompareCoordinator {
  const [compareStatus, setCompareStatus] = useState<string | null>(null)
  const compareFiles = useMemo(
    () => resolveCompareFiles(state.workspace, state.compareEntityIds),
    [state.compareEntityIds, state.workspace],
  )
  const compareOpen = state.compareEntityIds.length >= 2 && compareFiles.length >= 2

  useEffect(() => setCompareStatus(null), [projectSessionId])

  const enterCompare = useCallback(
    (files: readonly BrowserFile[], operationBusy: boolean) => {
      const availability = compareEntryAvailability({
        workspace: state.workspace,
        searchResultsOpen: state.search.showResults,
        operationBusy,
      })
      if (availability !== 'available') {
        setCompareStatus(
          availability === 'busy'
            ? '请等待当前文件操作完成后再开始对比。'
            : '请先返回文件夹内容，再选择图片进行对比。',
        )
        return
      }
      const validation = validateCompareCandidates(files)
      if (!validation.ok) {
        setCompareStatus(compareValidationMessage(validation.reason))
        return
      }
      setCompareStatus(null)
      closePreview()
      commands.setCompareEntityIds(files.map((file) => file.entityId))
    },
    [closePreview, commands.setCompareEntityIds, state.search.showResults, state.workspace],
  )
  const changeComparedEntities = useCallback(
    (entityIds: string[]) => {
      setCompareStatus(null)
      if (entityIds.length >= 2) {
        commands.setCompareEntityIds(entityIds)
        return
      }
      commands.setCompareEntityIds([])
      if (entityIds.length !== 1 || state.workspace?.workspace !== 'content') return
      const survivor = state.workspace.images.find((file) => file.entityId === entityIds[0])
      if (survivor !== undefined) openPreview(survivor)
    },
    [commands.setCompareEntityIds, openPreview, state.workspace],
  )

  useEffect(() => {
    if (state.compareEntityIds.length === 0) return
    const liveIds = compareFiles.map((file) => file.entityId)
    if (liveIds.length !== state.compareEntityIds.length || liveIds.length < 2) {
      changeComparedEntities(liveIds)
    }
  }, [changeComparedEntities, compareFiles, state.compareEntityIds])

  useEffect(() => {
    if (compareOpen && state.search.showResults) changeComparedEntities([])
  }, [changeComparedEntities, compareOpen, state.search.showResults])

  return {
    compareFiles,
    compareOpen,
    compareStatus,
    enterCompare,
    changeComparedEntities,
    setCompareStatus,
  }
}
