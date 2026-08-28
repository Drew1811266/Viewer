import { beforeEach, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import type { PreparedReviewCommand, ReviewWorkspaceError } from './reviewWorkspaceTypes'
import { tauriReviewWorkspaceBridge } from './viewer'

beforeEach(() => invoke.mockReset())
const session = { sessionId: '00000000-0000-0000-0000-000000000001', generation: 2 }
const envelope: PreparedReviewCommand = {
  context: { projectId: 'project', streamId: 'stream', production: null },
  commandId: 'command',
  expectedSnapshotId: null,
  payloadDigest: 'ab'.repeat(32),
  generated: {
    snapshotId: 'snapshot',
    feedbackId: 'feedback',
    textRevisionId: 'text',
    archiveId: 'archive',
    targets: [{ targetId: 'target', targetRevisionId: 'revision' }],
    migration: [],
    createdAtMs: 1234,
  },
  usageSelections: [
    {
      id: 'usage',
      candidate: {
        canonicalDigest: 'cd'.repeat(32),
        sourceDigest: 'ef'.repeat(32),
        source: 'producer/usage.json',
      },
    },
  ],
  command: {
    kind: 'save_feedback',
    feedbackId: null,
    text: '袖口收紧，保留材质',
    targets: [{ kind: 'add', assetVersionId: 'asset', anchor: { kind: 'asset' } }],
  },
}

it('maps all eleven continuous review operations without dropping the full retry envelope', async () => {
  const prepared = {
    ...session,
    commandId: envelope.commandId,
    expectedSnapshotId: null,
    command: envelope.command,
  }
  const apply = { ...session, envelope }
  const selection = { expectedSnapshotId: 'snapshot', groups: [] }
  const selector = { kind: 'archive' as const, archiveId: 'archive' }
  const assets = { ...session, entityIds: ['entity'] }
  const archive = { ...session, selection }
  const restore = { ...session, archiveId: 'archive', decisions: [] }
  const history = { ...session, selector }
  const usage = { ...session, entityId: 'usage-file' }
  const evidence = { ...history, assetVersionId: 'asset', role: 'base' as const }
  invoke.mockResolvedValueOnce(envelope)
  expect(await tauriReviewWorkspaceBridge.prepareCommand(prepared)).toEqual(envelope)
  await tauriReviewWorkspaceBridge.getWorkspace(session)
  await tauriReviewWorkspaceBridge.prepareAssets(assets)
  await tauriReviewWorkspaceBridge.applyCommand(apply)
  await tauriReviewWorkspaceBridge.previewArchive(archive)
  await tauriReviewWorkspaceBridge.previewRestore(restore)
  await tauriReviewWorkspaceBridge.getHistory(history)
  await tauriReviewWorkspaceBridge.inspectUsage(usage)
  await tauriReviewWorkspaceBridge.inspectMigration(session)
  await tauriReviewWorkspaceBridge.getEvidence(evidence)
  await tauriReviewWorkspaceBridge.cancelTask(session)
  expect(invoke.mock.calls).toEqual([
    ['prepare_review_command', { request: prepared }],
    ['get_review_workspace', { request: session }],
    ['prepare_review_assets', { request: assets }],
    ['apply_review_command', { request: apply }],
    ['preview_review_archive', { request: archive }],
    ['preview_review_restore', { request: restore }],
    ['get_review_history', { request: history }],
    ['inspect_review_usage', { request: usage }],
    ['inspect_review_migration', { request: session }],
    ['get_review_evidence', { request: evidence }],
    ['cancel_review_workspace_task', { request: session }],
  ])
})

it('preserves a committed receipt on rejection so callers do not manufacture a replacement command', async () => {
  const failure: ReviewWorkspaceError = {
    code: 'committed_view_unavailable',
    message: '已提交，刷新失败',
    retryable: true,
    committedReceipt: {
      commandId: 'command',
      payloadDigest: envelope.payloadDigest,
      snapshot: { snapshotId: 'snapshot', blake3: 'cd'.repeat(32) },
    },
  }
  invoke.mockRejectedValueOnce(failure)
  await expect(tauriReviewWorkspaceBridge.applyCommand({ ...session, envelope })).rejects.toEqual(
    failure,
  )
  expect(invoke).toHaveBeenCalledTimes(1)
})
