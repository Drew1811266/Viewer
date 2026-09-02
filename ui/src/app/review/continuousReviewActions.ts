import type {
  ReviewArchivePlan,
  ReviewArchiveSelection,
  ReviewHistoryRef,
  ReviewHistorySelector,
  ReviewMigrationBinding,
  ReviewMigrationInspection,
  ReviewMigrationPlan,
  ReviewRestoreDecision,
  ReviewRestorePlan,
  ReviewSourceBindingDecision,
  ReviewTargetVersionKey,
  ReviewWorkspaceCommand,
  ReviewWorkspaceError,
  ReviewWorkspacePort,
  ReviewWorkspaceSessionRequest,
} from '../../api/reviewWorkspaceTypes'
import type { ReviewAnchor } from '../../api/types'
import { type ContinuousReviewSnapshot, reviewWorkspaceError } from './continuousReviewModel'

export interface ContinuousReviewActionDriver {
  getSnapshot(): ContinuousReviewSnapshot
  version(): number
  query<T>(key: string, operation: (scope: ReviewWorkspaceSessionRequest) => Promise<T>): Promise<T>
  submit(command: ReviewWorkspaceCommand, expectedSnapshotId: string | null): Promise<void>
  reject(error: ReviewWorkspaceError): Promise<never>
}

/** Preview authority is local to one coordinator, not reconstructed from caller-provided DTOs. */
export function continuousReviewActions(
  port: ReviewWorkspacePort,
  driver: ContinuousReviewActionDriver,
) {
  let archive: {
    version: number
    selection: ReviewArchiveSelection
    plan: ReviewArchivePlan | null
  } | null = null
  let restore: {
    version: number
    archiveId: string
    decisions: ReviewRestoreDecision[]
    plan: ReviewRestorePlan | null
  } | null = null
  let archiving: Promise<void> | null = null
  let restoring: Promise<void> | null = null
  let migration: { version: number; inspection: ReviewMigrationInspection | null } | null = null

  const noDraft = () =>
    driver.getSnapshot().editorInput.contextKey === null
      ? null
      : reviewWorkspaceError('needs_confirmation', '请先保存或放弃未提交输入', false)
  const previewRequired = () =>
    reviewWorkspaceError('preview_required', '请重新查看并确认操作范围', false)
  const currentId = () => driver.getSnapshot().view?.current?.authoring.head.snapshotId ?? null

  return {
    previewArchive: async (selection: ReviewArchiveSelection): Promise<ReviewArchivePlan> => {
      const frozen = structuredClone(selection)
      const version = driver.version()
      const request = { version, selection: frozen, plan: null as ReviewArchivePlan | null }
      archive = request
      const plan = await driver.query('archive', async (scope) => {
        // Even a locally rejected request supersedes an older in-flight preview.
        const blocked = noDraft()
        if (blocked) throw blocked
        if (frozen.expectedSnapshotId !== currentId())
          throw reviewWorkspaceError('stale_snapshot', '意见已变化，请刷新后重新预览', false)
        return port.previewArchive({ ...scope, selection: frozen })
      })
      if (archive !== request || version !== driver.version())
        throw reviewWorkspaceError('cancelled', '预览已失效，请重新获取', true)
      if (plan.expectedSnapshotId !== frozen.expectedSnapshotId)
        return driver.reject(
          reviewWorkspaceError('stale_snapshot', '预览不属于当前显示版本', false),
        )
      request.plan = structuredClone(plan)
      return plan
    },
    commitArchive: (): Promise<void> => {
      if (archiving !== null) return archiving
      if (archive === null || archive.plan === null || archive.version !== driver.version())
        return driver.reject(previewRequired())
      const blocked = noDraft()
      if (blocked) return driver.reject(blocked)
      const pending = driver.submit(
        { kind: 'archive', ...archive.selection },
        archive.plan.expectedSnapshotId,
      )
      archiving = pending
      pending.then(
        () => {
          archiving = null
          archive = null
        },
        () => {
          archiving = null
        },
      )
      return pending
    },
    previewRestore: async (
      archiveId: string,
      decisions: ReviewRestoreDecision[],
    ): Promise<ReviewRestorePlan> => {
      const expected = currentId()
      const version = driver.version()
      const frozen = structuredClone(decisions)
      const request = {
        version,
        archiveId,
        decisions: frozen,
        plan: null as ReviewRestorePlan | null,
      }
      restore = request
      const plan = await driver.query('restore', async (scope) => {
        const blocked = noDraft()
        if (blocked) throw blocked
        return port.previewRestore({ ...scope, archiveId, decisions: frozen })
      })
      if (restore !== request || version !== driver.version())
        throw reviewWorkspaceError('cancelled', '预览已失效，请重新获取', true)
      if (plan.expectedSnapshotId !== expected)
        return driver.reject(
          reviewWorkspaceError('stale_snapshot', '意见已变化，请刷新后重新预览', false),
        )
      request.plan = structuredClone(plan)
      return plan
    },
    restore: (): Promise<void> => {
      if (restoring !== null) return restoring
      if (restore === null || restore.plan === null || restore.version !== driver.version())
        return driver.reject(previewRequired())
      if (restore.plan.conflicts.length > 0)
        return driver.reject(
          reviewWorkspaceError('needs_confirmation', '请先逐项确认恢复冲突', false),
        )
      const pending = driver.submit(
        { kind: 'restore', archiveId: restore.archiveId, decisions: restore.decisions },
        restore.plan.expectedSnapshotId,
      )
      restoring = pending
      pending.then(
        () => {
          restoring = null
          restore = null
        },
        () => {
          restoring = null
        },
      )
      return pending
    },
    withdrawTargets: (targets: ReviewTargetVersionKey[]) =>
      driver.submit({ kind: 'withdraw', targets }, currentId()),
    continueHistorical: (historyRef: ReviewHistoryRef, bindings: ReviewSourceBindingDecision[]) =>
      driver.submit({ kind: 'continue_historical', historyRef, bindings }, currentId()),
    continueLegacy: (historyRef: ReviewHistoryRef, bindings: ReviewMigrationBinding[]) =>
      driver.submit({ kind: 'continue_legacy', historyRef, bindings }, currentId()),
    confirmSource: (decision: ReviewSourceBindingDecision) =>
      driver.submit({ kind: 'confirm_source', ...decision }, currentId()),
    confirmApplicability: (
      key: ReviewTargetVersionKey,
      assetVersionId: string,
      anchor: ReviewAnchor,
    ) => driver.submit({ kind: 'confirm_applicability', key, assetVersionId, anchor }, currentId()),
    adoptUsage: (declarationId: string) =>
      driver.submit({ kind: 'adopt_usage', declarationId }, currentId()),
    inspectUsage: (entityId: string) =>
      driver.query('usage', (scope) => port.inspectUsage({ ...scope, entityId })),
    selectUsage: () => driver.query('usage-selection', (scope) => port.selectUsage(scope)),
    inspectMigration: async () => {
      migration = null
      const version = driver.version()
      const inspection = await driver.query('migration', (scope) => port.inspectMigration(scope))
      migration = { version, inspection: structuredClone(inspection) }
      return inspection
    },
    migrate: (plan: ReviewMigrationPlan) => {
      const inspection =
        migration?.version === driver.version()
          ? migration.inspection
          : driver.getSnapshot().view?.migration
      if (!inspection || inspection.inspectionDigest !== plan.inspectionDigest)
        return driver.reject(previewRequired())
      return driver.submit({ kind: 'migrate', ...plan }, currentId())
    },
    getHistory: (selector: ReviewHistorySelector) => {
      const frozen = structuredClone(selector)
      return driver.query('history', (scope) => port.getHistory({ ...scope, selector: frozen }))
    },
    getEvidence: (
      selector: ReviewHistorySelector,
      assetVersionId: string,
      role: 'base' | 'annotated',
    ) => {
      const frozen = structuredClone(selector)
      return driver.query(`evidence:${role}`, (scope) =>
        port.getEvidence({ ...scope, selector: frozen, assetVersionId, role }),
      )
    },
    prepareAssets: (entityIds: string[]) => {
      const frozen = [...entityIds]
      return driver.query('assets', (scope) => port.prepareAssets({ ...scope, entityIds: frozen }))
    },
  }
}
