import { useLayoutEffect, useState } from 'react'
import type {
  ReviewArchiveSelection,
  ReviewHistorySelector,
  ReviewUsageImportPreview,
} from '../../api/reviewWorkspaceTypes'
import type { ProjectAccess } from '../../app/review/reviewModel'
import type { ContinuousReviewCoordinator } from '../../app/review/useContinuousReviewCoordinator'
import type { ReviewSessionCoordinator } from '../../app/review/useReviewSessionCoordinator'
import ViewerButton from '../ui/ViewerButton'
import ReviewArchiveDialog from './ReviewArchiveDialog'
import ReviewContextBar from './ReviewContextBar'
import ReviewHistoryPanel from './ReviewHistoryPanel'
import ReviewMigrationDialog from './ReviewMigrationDialog'
import ReviewRecoveryNotice from './ReviewRecoveryNotice'
import ReviewSourceConfirmation from './ReviewSourceConfirmation'
import ReviewUsageImport from './ReviewUsageImport'
import { useContinuousArchivePreview } from './useContinuousArchivePreview'
import { useContinuousHistoryReview } from './useContinuousHistoryReview'

interface ContinuousReviewWorkspaceLayerProps {
  review: ReviewSessionCoordinator
  coordinator: ContinuousReviewCoordinator
  selectedEntityIds: string[]
  projectAccess: ProjectAccess
  contextBarHidden: boolean
  archiveSelection?: ReviewArchiveSelection
  historySelector?: ReviewHistorySelector
  onReturnToMembers(entityIds: string[]): void
}

export default function ContinuousReviewWorkspaceLayer({
  review,
  coordinator,
  selectedEntityIds,
  projectAccess,
  contextBarHidden,
  archiveSelection,
  historySelector,
  onReturnToMembers,
}: ContinuousReviewWorkspaceLayerProps) {
  const [usageOpen, setUsageOpen] = useState(false)
  const [usagePreview, setUsagePreview] = useState<ReviewUsageImportPreview | null>(null)
  const [migrationDismissed, setMigrationDismissed] = useState(false)
  const adoptedSelection = usageArchiveSelection(coordinator, usagePreview)
  const archive = useContinuousArchivePreview(
    coordinator,
    archiveSelection ?? adoptedSelection ?? undefined,
  )
  const history = useContinuousHistoryReview(coordinator, selectedEntityIds, historySelector)

  useLayoutEffect(() => {
    setUsageOpen(false)
    setUsagePreview(null)
    setMigrationDismissed(false)
  }, [coordinator.workbenchSessionKey])

  const migration = coordinator.view?.migration ?? null
  return (
    <>
      <ReviewRecoveryNotice
        review={review}
        projectAccess={projectAccess}
        continuousReview={coordinator}
      />
      {migration !== null && !migrationDismissed && (
        <ReviewMigrationDialog
          inspection={migration}
          onConfirm={(plan) => void coordinator.migrate(plan)}
          onCancel={() => setMigrationDismissed(true)}
        />
      )}
      {migration !== null && migrationDismissed && (
        <section className="review-local-notice" data-tone="warning" role="status">
          <div>
            <strong>旧评审记录尚未迁移</strong>
            <span>旧数据保持不变；迁移前不会启用持续评审写入。</span>
          </div>
          <ViewerButton onClick={() => setMigrationDismissed(false)}>查看迁移选项</ViewerButton>
        </section>
      )}
      {migration === null && coordinator.state.kind === 'ready' && !contextBarHidden && (
        <ReviewContextBar
          protocol="continuous"
          snapshot={review.snapshot}
          inspectorOpen={false}
          onReturnToMembers={() => onReturnToMembers(currentEntityIds(coordinator))}
          onToggleInspector={() => undefined}
          onPrepareCompletion={() => undefined}
          onRequestAbandon={() => undefined}
          onArchive={() => void archive.open()}
          onHistory={history.available ? () => void history.open() : undefined}
          onUsageImport={
            coordinator.view?.capabilities.usageImport ? () => setUsageOpen(true) : undefined
          }
          continuousFeedbackCount={coordinator.view?.current?.state.feedback.length}
          archiveDisabled={
            coordinator.currentSnapshotId === null || archive.busy || history.transactionBusy
          }
          historyDisabled={history.transactionBusy}
          archiveNotice={history.error?.message ?? archive.notice ?? history.notice ?? null}
        />
      )}
      {migration === null && (
        <ContinuousDialogs archive={archive} history={history} coordinator={coordinator} />
      )}
      {usageOpen && (
        <ReviewUsageImport
          onSelect={coordinator.selectUsage}
          onCancel={() => setUsageOpen(false)}
          onConfirm={(preview) =>
            coordinator.adoptUsage(preview.declaration.id).then(() => {
              setUsagePreview(preview)
              setUsageOpen(false)
            })
          }
        />
      )}
    </>
  )
}

function ContinuousDialogs({
  archive,
  history,
  coordinator,
}: {
  archive: ReturnType<typeof useContinuousArchivePreview>
  history: ReturnType<typeof useContinuousHistoryReview>
  coordinator: ContinuousReviewCoordinator
}) {
  return (
    <>
      {archive.dialog !== null && (
        <ReviewArchiveDialog
          preview={archive.dialog.preview}
          selection={archive.dialog.selection}
          busy={archive.busy}
          error={archive.error}
          onSelectionChange={(selection) => void archive.changeSelection(selection)}
          onConfirm={() => void archive.confirm()}
          onCancel={archive.cancel}
        />
      )}
      {history.panel !== null && history.source === null && (
        <ReviewHistoryPanel
          history={history.panel.history}
          historyRef={history.panel.historyRef}
          historyRefs={history.panel.historyRefs}
          restorePlan={history.panel.restorePreview?.plan ?? null}
          currentFeedback={coordinator.view?.current?.state.feedback ?? []}
          evidence={history.panel.evidence}
          busy={history.busy}
          canClose={history.canClose}
          error={history.error}
          onClose={history.close}
          onContinue={(reference) => void history.continueHistorical(reference)}
          onRestore={(decisions) => void history.restore(decisions)}
          onInvalidateRestorePreview={history.invalidateRestorePreview}
          onRequestEvidence={(assetVersionId, role) =>
            void history.requestEvidence(assetVersionId, role)
          }
        />
      )}
      {history.source !== null && (
        <ReviewSourceConfirmation
          oldAsset={history.source.oldAsset}
          candidates={history.source.candidates}
          originalAnchor={history.source.originalAnchor}
          targetKey={history.source.targetKey}
          busy={history.busy}
          error={history.error}
          onCancel={history.closeSource}
          onConfirm={(decision) => void history.confirmSource(decision)}
        />
      )}
    </>
  )
}

function currentEntityIds(coordinator: ContinuousReviewCoordinator): string[] {
  return (
    coordinator.view?.current?.state.assets.flatMap((asset) =>
      asset.sourceEntityId === null ? [] : [asset.sourceEntityId],
    ) ?? []
  )
}

function usageArchiveSelection(
  coordinator: ContinuousReviewCoordinator,
  preview: ReviewUsageImportPreview | null,
): ReviewArchiveSelection | null {
  const expectedSnapshotId = coordinator.currentSnapshotId
  if (preview === null || expectedSnapshotId === null) return null
  return {
    expectedSnapshotId,
    groups: [
      {
        basis: {
          kind: 'known',
          snapshot: structuredClone(preview.declaration.basis),
          source: { kind: 'agent_declared', usageId: preview.declaration.id },
        },
        targets: structuredClone(preview.declaration.targets),
      },
    ],
  }
}
