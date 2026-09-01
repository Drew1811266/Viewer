import { vi } from 'vitest'
import type {
  PreparedReviewCommand,
  PrepareReviewCommandRequest,
  ReviewApplyResult,
  ReviewArchivePlan,
  ReviewArchiveSelection,
  ReviewMigrationInspection,
  ReviewRecoveryDraft,
  ReviewRestorePlan,
  ReviewWorkspaceError,
  ReviewWorkspacePort,
  ReviewWorkspaceView,
} from '../../api/reviewWorkspaceTypes'

export const context = { projectId: 'project-1', streamId: 'stream-1', production: null }
export const session = { sessionId: 'session-1', generation: 1 }
export const targetKey = {
  feedbackId: 'feedback-1',
  textRevisionId: 'text-1',
  targetId: 'target-1',
  targetRevisionId: 'revision-1',
}

export function archiveSelection(snapshotId = 'base'): ReviewArchiveSelection {
  return {
    expectedSnapshotId: snapshotId,
    groups: [{ basis: { kind: 'unknown' }, targets: [targetKey] }],
  }
}
export function archivePlan(selection = archiveSelection()): ReviewArchivePlan {
  return {
    ...selection,
    removed: selection.groups.flatMap((group) => group.targets),
    retained: [],
    alreadyCovered: [],
  }
}
export function restorePlan(snapshotId = 'base'): ReviewRestorePlan {
  return {
    expectedSnapshotId: snapshotId,
    restored: [],
    conflicts: [],
    coverageReversals: [],
    requiresSourceCheck: [],
  }
}
export function migrationInspection(): ReviewMigrationInspection {
  return {
    legacyProtocol: 'viewer.review/2',
    indexDigest: '12'.repeat(32),
    inspectionDigest: '34'.repeat(32),
    legacyRecords: [],
    activeDraft: null,
    completedCandidates: [],
    limitations: ['usage_unconfirmed'],
  }
}
export function recoveryDraft(commandId = 'different-command'): ReviewRecoveryDraft {
  return {
    streamId: 'stream-1',
    commandId,
    expectedSnapshotId: 'base',
    payloadDigest: 'ab'.repeat(32),
    failure: 'commit_unknown',
    editorInput: {
      migration: null,
      selections: [],
      text: '未决输入',
      feedbackId: null,
      targets: [],
      historyRef: null,
    },
  }
}
export const input = {
  contextKey: 'image-1',
  feedbackId: null,
  text: '袖口收紧，保留褶皱',
  targets: [
    { kind: 'add' as const, assetVersionId: 'asset-1', anchor: { kind: 'asset' as const } },
  ],
}

export function workspace(snapshotId: string | null = null): ReviewWorkspaceView {
  return {
    streamId: 'stream-1',
    historySelectors: [],
    current:
      snapshotId === null
        ? null
        : {
            reference: { snapshotId, blake3: 'ab'.repeat(32) },
            production: null,
            state: {
              projectId: context.projectId,
              streamId: context.streamId,
              snapshotId,
              parent: null,
              assets: [],
              feedback: [],
            },
            commandId: 'previous-command',
            payloadDigest: 'cd'.repeat(32),
            changes: [],
            evidence: [],
          },
    sourceChecks: [],
    projection: { actionable: [], needsConfirmation: [] },
    recovery: [],
    migration: null,
    capabilities: { continuousEditing: true, usageImport: true, migration: false },
  }
}

export function prepared(request: PrepareReviewCommandRequest): PreparedReviewCommand {
  return {
    context,
    commandId: request.commandId,
    expectedSnapshotId: request.expectedSnapshotId,
    command: request.command,
    payloadDigest: 'ef'.repeat(32),
    generated: {
      snapshotId: 'saved-snapshot',
      feedbackId: 'saved-feedback',
      textRevisionId: 'saved-text',
      archiveId: 'saved-archive',
      targets: [{ targetId: 'saved-target', targetRevisionId: 'saved-revision' }],
      migration: [],
      createdAtMs: 1234,
    },
    usageSelections: [
      {
        id: 'usage-1',
        candidate: {
          canonicalDigest: '12'.repeat(32),
          sourceDigest: '34'.repeat(32),
          source: 'producer/usage.json',
        },
      },
    ],
  }
}

export function applied(
  envelope: PreparedReviewCommand,
  view = workspace('saved-snapshot'),
): ReviewApplyResult {
  return {
    receipt: {
      commandId: envelope.commandId,
      payloadDigest: envelope.payloadDigest,
      snapshot: { snapshotId: envelope.generated.snapshotId, blake3: '56'.repeat(32) },
    },
    view,
  }
}

export function failure(
  code: ReviewWorkspaceError['code'],
  retryable = true,
): ReviewWorkspaceError {
  return { code, message: code, retryable, committedReceipt: null }
}

// Only the desktop boundary is replaced. The coordinator and its input/state logic are real.
export function reviewPort(view = workspace()): ReviewWorkspacePort {
  const unsupported = async () => {
    throw new Error('Unexpected desktop operation in this test')
  }
  return {
    getWorkspace: vi.fn().mockResolvedValue(view),
    prepareAssets: vi.fn(unsupported),
    prepareCommand: vi.fn(async (request) => prepared(request)),
    applyCommand: vi.fn(async ({ envelope }) => applied(envelope)),
    previewArchive: vi.fn(unsupported),
    previewRestore: vi.fn(unsupported),
    getHistory: vi.fn(unsupported),
    inspectUsage: vi.fn(unsupported),
    selectUsage: vi.fn(unsupported),
    inspectMigration: vi.fn(unsupported),
    getEvidence: vi.fn(unsupported),
    cancelTask: vi.fn().mockResolvedValue(0),
  }
}

export function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((yes, no) => {
    resolve = yes
    reject = no
  })
  return { promise, resolve, reject }
}
