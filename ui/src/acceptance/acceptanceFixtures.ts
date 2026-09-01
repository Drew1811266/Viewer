import type {
  PreparedReviewCommand,
  ReviewArchivePlan,
  ReviewHistoryView,
  ReviewMigrationInspection,
  ReviewRestorePlan,
  ReviewTargetVersionKey,
  ReviewWorkspaceView,
} from '../api/reviewWorkspaceTypes'
import type {
  BrowserFile,
  ContentFolderCard,
  FolderTreeItem,
  FolderWorkspace,
  ImageRepresentation,
  ImageRepresentationRequest,
  ProjectSnapshot,
  ReviewCompletionProposal,
  ReviewConflictSnapshot,
  ReviewMemberSnapshot,
  ReviewSessionSnapshot,
  VideoCacheStats,
  VideoFile,
} from '../api/types'
import type { ViewerBridge } from '../api/viewer'

const FIXTURE_MODIFIED_NS = '1767600000000000000'
const IMAGE_WIDTH = 560
const IMAGE_HEIGHT = 373

export const ACCEPTANCE_VIDEO_FRAME_URL = '/商品-15.jpg'
export const ACCEPTANCE_VIDEO_CACHE_STATS: VideoCacheStats = {
  bytesUsed: 268_435_456,
  budgetBytes: 1_073_741_824,
  entryCount: 24,
}

export const ACCEPTANCE_VIDEO_FILES: VideoFile[] = [
  acceptanceVideo('acceptance-video-landscape', 'Lookbook-01.mp4', {
    durationUs: 92_400_000,
    displayWidth: 1_920,
    displayHeight: 1_080,
    coverUrl: '/商品-15.jpg',
  }),
  acceptanceVideo('acceptance-video-portrait', 'Campaign-portrait.mov', {
    durationUs: 48_200_000,
    displayWidth: 1_080,
    displayHeight: 1_920,
    rotationDegrees: 90,
    coverUrl: '/商品-16.jpg',
  }),
  acceptanceVideo('acceptance-video-unavailable', 'Archive-damaged.mkv', {
    durationUs: null,
    displayWidth: null,
    displayHeight: null,
    videoCodec: null,
    audioCodec: null,
    probeStatus: 'failed',
    failureKind: 'damaged',
    coverUrl: null,
  }),
]

export function acceptanceVideoCacheBridge(): Pick<
  ViewerBridge,
  'videoCacheStats' | 'videoCacheClear'
> {
  return {
    async videoCacheStats() {
      return ACCEPTANCE_VIDEO_CACHE_STATS
    },
    async videoCacheClear() {
      return { ...ACCEPTANCE_VIDEO_CACHE_STATS, bytesUsed: 0, entryCount: 0 }
    },
  }
}

export const ACCEPTANCE_PROJECT_SNAPSHOT: ProjectSnapshot = {
  projectId: 'acceptance-project',
  sessionId: 'acceptance-session',
  generation: 1,
  displayName: '测试图',
  access: 'read_write',
}

export const ACCEPTANCE_FILES: BrowserFile[] = Array.from({ length: 30 }, (_, offset) => {
  const number = String(offset + 1).padStart(2, '0')
  const name = `商品-${number}.jpg`
  return {
    entityId: `acceptance-image-${number}`,
    relativePath: `衣服/A01/${name}`,
    name,
    kind: 'jpeg',
    size: 32_768 + offset * 1_024,
    modifiedNs: String(BigInt(FIXTURE_MODIFIED_NS) + BigInt(offset)),
    marker: { reviewState: null, favorite: false },
    imageMetadata: { width: IMAGE_WIDTH, height: IMAGE_HEIGHT },
    imageUrl: null,
    videoMetadata: null,
  }
})

export const ACCEPTANCE_REVIEW_MEMBERS: ReviewMemberSnapshot[] = ACCEPTANCE_FILES.slice(0, 6).map(
  (file, index) => ({
    assetVersionId: `acceptance-asset-version-${index + 1}`,
    entityId: file.entityId,
    relativePath: file.relativePath,
    displayName: file.name,
    kind: 'image',
    feedbackItems: index < 2 ? 1 : 0,
  }),
)

export const ACCEPTANCE_REVIEW_CONFLICTS: ReviewConflictSnapshot[] = [
  {
    assetVersionId: ACCEPTANCE_REVIEW_MEMBERS[2]?.assetVersionId ?? 'acceptance-asset-version-3',
    relativePath: ACCEPTANCE_FILES[2]?.relativePath ?? '衣服/A01/商品-03.jpg',
    kind: 'content_changed',
  },
]

export function acceptanceReviewSnapshot(
  overrides: Partial<ReviewSessionSnapshot> = {},
): ReviewSessionSnapshot {
  const snapshot: ReviewSessionSnapshot = {
    phase: 'active',
    resume: null,
    reviewStreamId: 'acceptance-review-stream',
    reviewRoundId: 'acceptance-review-round',
    revision: 4,
    members: ACCEPTANCE_REVIEW_MEMBERS,
    feedback: [
      {
        feedbackId: 'acceptance-feedback-1',
        text: '降低商品表面的高光强度，并保持布料纹理清晰。',
        createdAtMs: 1_767_600_000_000,
        targetEntityIds: ACCEPTANCE_REVIEW_MEMBERS.slice(0, 2).flatMap((member) =>
          member.entityId === null ? [] : [member.entityId],
        ),
        targets: ACCEPTANCE_REVIEW_MEMBERS.slice(0, 2).map((member) => ({
          assetVersionId: member.assetVersionId,
          entityId: member.entityId,
          anchor: { kind: 'asset' as const },
        })),
        targetCount: 2,
      },
    ],
    restorableFeedbackId: null,
    unreviewable: [
      {
        assetVersionId:
          ACCEPTANCE_REVIEW_MEMBERS[5]?.assetVersionId ?? 'acceptance-asset-version-6',
        relativePath: ACCEPTANCE_FILES[5]?.relativePath ?? '衣服/A01/商品-06.jpg',
        failure: 'decode_failed',
      },
    ],
    conflicts: [],
    counts: { total: 6, feedbackItems: 1, revise: 2, unreviewable: 1, pass: 0 },
    error: null,
  }
  return { ...snapshot, ...overrides }
}

export function acceptanceReviewCompletion(
  conflicts: ReviewConflictSnapshot[] = [],
): ReviewCompletionProposal {
  return {
    proposalId: 72,
    summary: {
      reviewRoundId: 'acceptance-review-round',
      revision: 4,
      total: 6,
      revise: 2,
      unreviewable: 1,
      defaultPass: 3,
      feedback: [
        {
          feedbackId: 'acceptance-feedback-1',
          text: '降低商品表面的高光强度，并保持布料纹理清晰。',
          targetCount: 2,
        },
      ],
      conflicts,
      pending: [],
      canComplete: conflicts.length === 0,
    },
  }
}

export const ACCEPTANCE_TEXT_FILES: BrowserFile[] = [
  {
    entityId: 'acceptance-text-markdown',
    relativePath: '文档/sample.md',
    name: 'sample.md',
    kind: 'markdown',
    size: 1_824,
    modifiedNs: FIXTURE_MODIFIED_NS,
    marker: { reviewState: 'keep', favorite: false },
    imageMetadata: null,
    imageUrl: null,
    videoMetadata: null,
  },
  {
    entityId: 'acceptance-text-plain',
    relativePath: '文档/plain.txt',
    name: 'plain.txt',
    kind: 'text',
    size: 3_264,
    modifiedNs: FIXTURE_MODIFIED_NS,
    marker: { reviewState: null, favorite: true },
    imageMetadata: null,
    imageUrl: null,
    videoMetadata: null,
  },
  {
    entityId: 'acceptance-text-gb18030',
    relativePath: '文档/gb18030.txt',
    name: 'gb18030.txt',
    kind: 'text',
    size: 2_048,
    modifiedNs: FIXTURE_MODIFIED_NS,
    marker: { reviewState: null, favorite: false },
    imageMetadata: null,
    imageUrl: null,
    videoMetadata: null,
  },
  {
    entityId: 'acceptance-text-large',
    relativePath: '文档/large.txt',
    name: 'large.txt',
    kind: 'text',
    size: 10 * 1_024 * 1_024 + 512,
    modifiedNs: FIXTURE_MODIFIED_NS,
    marker: { reviewState: null, favorite: false },
    imageMetadata: null,
    imageUrl: null,
    videoMetadata: null,
  },
]

export const ACCEPTANCE_UNSUPPORTED_FILE: BrowserFile = {
  entityId: 'acceptance-unsupported',
  relativePath: '其它/unsupported.bin',
  name: 'unsupported.bin',
  kind: 'other',
  size: 768,
  modifiedNs: FIXTURE_MODIFIED_NS,
  marker: { reviewState: null, favorite: false },
  imageMetadata: null,
  imageUrl: null,
  videoMetadata: null,
}

export const ACCEPTANCE_FOLDER_TREE: FolderTreeItem[] = [
  {
    entityId: 'acceptance-folder-clothes',
    parentEntityId: null,
    relativePath: '衣服',
    name: '衣服',
    marker: { reviewState: null, favorite: false },
  },
  {
    entityId: 'acceptance-folder-a01',
    parentEntityId: 'acceptance-folder-clothes',
    relativePath: '衣服/A01',
    name: 'A01',
    marker: { reviewState: null, favorite: false },
  },
  ...['A02', 'A03', 'A04'].map(
    (name) =>
      ({
        entityId: `acceptance-folder-${name.toLowerCase()}`,
        parentEntityId: 'acceptance-folder-clothes',
        relativePath: `衣服/${name}`,
        name,
        marker: { reviewState: null, favorite: false },
      }) satisfies FolderTreeItem,
  ),
  {
    entityId: 'acceptance-folder-destination-root',
    parentEntityId: null,
    relativePath: '目标',
    name: '目标',
    marker: { reviewState: null, favorite: false },
  },
  {
    entityId: 'acceptance-folder-destination',
    parentEntityId: 'acceptance-folder-destination-root',
    relativePath: '目标/Destination',
    name: 'Destination',
    marker: { reviewState: null, favorite: false },
  },
  {
    entityId: 'acceptance-folder-empty-root',
    parentEntityId: null,
    relativePath: '空目录',
    name: '空目录',
    marker: { reviewState: null, favorite: false },
  },
  {
    entityId: 'acceptance-folder-empty',
    parentEntityId: 'acceptance-folder-empty-root',
    relativePath: '空目录/Empty',
    name: 'Empty',
    marker: { reviewState: null, favorite: false },
  },
  {
    entityId: 'acceptance-folder-documents',
    parentEntityId: null,
    relativePath: '文档',
    name: '文档',
    marker: { reviewState: null, favorite: false },
  },
]

export const ACCEPTANCE_CONTENT_FOLDERS: ContentFolderCard[] = ['A01', 'A02', 'A03', 'A04'].map(
  (name, index) => ({
    entityId: `acceptance-folder-${name.toLowerCase()}`,
    relativePath: `衣服/${name}`,
    name,
    marker: { reviewState: null, favorite: index === 1 },
    imageCount: index === 0 ? ACCEPTANCE_FILES.length : 6 + index * 3,
    videoCount: 0,
    otherFileCount: index === 0 ? ACCEPTANCE_TEXT_FILES.length + 1 : 0,
    reviewProgress: {
      total: index === 0 ? ACCEPTANCE_FILES.length : 6 + index * 3,
      keep: index + 1,
      pending: index,
      reject: 0,
      unmarked: index === 0 ? ACCEPTANCE_FILES.length - 1 : 5 + index,
      favorite: index === 1 ? 1 : 0,
    },
    representativeImages: ACCEPTANCE_FILES.slice(index * 4, index * 4 + 6),
  }),
)

export function acceptanceWorkspace(
  entityId: string | null,
  showingAggregate = false,
): FolderWorkspace {
  if (entityId === 'acceptance-folder-empty') return { workspace: 'empty' }
  if (entityId === null) {
    return {
      workspace: 'category',
      folders: [
        folderCard('acceptance-folder-clothes', '衣服', '衣服', ACCEPTANCE_FILES.slice(0, 6)),
        folderCard(
          'acceptance-folder-documents',
          '文档',
          '文档',
          ACCEPTANCE_FILES.slice(6, 10),
          ACCEPTANCE_TEXT_FILES.length,
        ),
        folderCard(
          'acceptance-folder-destination-root',
          '目标',
          '目标',
          ACCEPTANCE_FILES.slice(10, 13),
        ),
      ],
    }
  }
  if (entityId === 'acceptance-folder-clothes' && !showingAggregate) {
    return { workspace: 'category', folders: ACCEPTANCE_CONTENT_FOLDERS }
  }
  if (entityId.startsWith('acceptance-folder-a') || showingAggregate) {
    return {
      workspace: 'content',
      images: ACCEPTANCE_FILES,
      videos: [],
      otherFiles: [...ACCEPTANCE_TEXT_FILES, ACCEPTANCE_UNSUPPORTED_FILE],
    }
  }
  return { workspace: 'empty' }
}

function folderCard(
  entityId: string,
  relativePath: string,
  name: string,
  representativeImages: BrowserFile[],
  otherFileCount = 0,
): ContentFolderCard {
  return {
    entityId,
    relativePath,
    name,
    marker: { reviewState: null, favorite: false },
    imageCount: representativeImages.length,
    videoCount: 0,
    otherFileCount,
    reviewProgress: {
      total: representativeImages.length + otherFileCount,
      keep: 0,
      pending: 0,
      reject: 0,
      unmarked: representativeImages.length + otherFileCount,
      favorite: 0,
    },
    representativeImages,
  }
}

export function imageRepresentation(
  file: BrowserFile,
  kind: ImageRepresentationRequest['kind'],
): ImageRepresentation {
  if (file.kind !== 'jpeg' && file.kind !== 'png') {
    throw new Error(`Acceptance image representation requires a previewable image: ${file.name}`)
  }
  return {
    cacheKey: `${file.entityId}:${kind}`,
    url: `/${encodeURIComponent(file.name)}?representation=${kind}`,
    width: file.imageMetadata?.width ?? IMAGE_WIDTH,
    height: file.imageMetadata?.height ?? IMAGE_HEIGHT,
    backend: 'image_io',
  }
}

export function acceptanceFile(entityId: string): BrowserFile {
  const file = [
    ...ACCEPTANCE_FILES,
    ...ACCEPTANCE_VIDEO_FILES,
    ...ACCEPTANCE_TEXT_FILES,
    ACCEPTANCE_UNSUPPORTED_FILE,
  ].find((candidate) => candidate.entityId === entityId)
  if (file === undefined) throw new Error(`Unknown Viewer acceptance file: ${entityId}`)
  return file
}

function acceptanceVideo(
  entityId: string,
  name: string,
  metadata: Partial<VideoFile['videoMetadata']>,
): VideoFile {
  return {
    entityId,
    relativePath: `视频/${name}`,
    name,
    kind: 'video',
    size: 24_000_000,
    modifiedNs: FIXTURE_MODIFIED_NS,
    marker: { reviewState: null, favorite: false },
    imageMetadata: null,
    imageUrl: null,
    videoMetadata: {
      durationUs: 92_400_000,
      displayWidth: 1_920,
      displayHeight: 1_080,
      rotationDegrees: 0,
      frameRateMillihertz: 30_000,
      videoCodec: 'h264',
      audioCodec: 'aac',
      probeStatus: 'ready',
      failureKind: null,
      coverUrl: ACCEPTANCE_VIDEO_FRAME_URL,
      ...metadata,
    },
  }
}

export const ACCEPTANCE_CONTINUOUS_TARGET: ReviewTargetVersionKey = {
  feedbackId: 'acceptance-continuous-feedback',
  textRevisionId: 'acceptance-continuous-text-c',
  targetId: 'acceptance-continuous-target-1',
  targetRevisionId: 'acceptance-continuous-target-revision-c',
}

export const ACCEPTANCE_CONTINUOUS_OLD_TARGET: ReviewTargetVersionKey = {
  ...ACCEPTANCE_CONTINUOUS_TARGET,
  textRevisionId: 'acceptance-continuous-text-b',
  targetRevisionId: 'acceptance-continuous-target-revision-b',
}

const continuousAsset = {
  id: 'acceptance-continuous-asset-1',
  sourceEntityId: ACCEPTANCE_FILES[0]?.entityId ?? 'acceptance-image-01',
  relativePath: ACCEPTANCE_FILES[0]?.relativePath ?? '衣服/A01/商品-01.jpg',
  evidence: { sizeBytes: 32768, modifiedNs: FIXTURE_MODIFIED_NS, blake3: '11'.repeat(32) },
  media: { kind: 'image' as const, width: IMAGE_WIDTH, height: IMAGE_HEIGHT },
  producerAssetId: null,
  parentAssetVersionId: null,
}

const replacedContinuousAsset = {
  ...continuousAsset,
  id: 'acceptance-continuous-asset-2',
  sourceEntityId: ACCEPTANCE_FILES[1]?.entityId ?? 'acceptance-image-02',
  relativePath: ACCEPTANCE_FILES[1]?.relativePath ?? '衣服/A01/商品-02.jpg',
  parentAssetVersionId: continuousAsset.id,
}

export function acceptanceContinuousReviewView(id: string): ReviewWorkspaceView {
  const emptyCurrent = id === 'RVW-32'
  const migration = id === 'RVW-31' ? acceptanceContinuousMigration() : null
  return {
    streamId: 'acceptance-continuous-stream',
    historySelectors: [],
    current:
      migration !== null
        ? null
        : {
            reference: { snapshotId: 'acceptance-snapshot-c', blake3: 'ab'.repeat(32) },
            production: null,
            state: {
              projectId: ACCEPTANCE_PROJECT_SNAPSHOT.projectId,
              streamId: 'acceptance-continuous-stream',
              snapshotId: 'acceptance-snapshot-c',
              parent: { snapshotId: 'acceptance-snapshot-b', blake3: 'bc'.repeat(32) },
              assets: emptyCurrent ? [] : [continuousAsset, replacedContinuousAsset],
              feedback: emptyCurrent
                ? []
                : [
                    {
                      id: ACCEPTANCE_CONTINUOUS_TARGET.feedbackId,
                      textRevisionId: ACCEPTANCE_CONTINUOUS_TARGET.textRevisionId,
                      text:
                        id === 'RVW-29'
                          ? '图二的新意见，不可覆盖'
                          : '袖口收紧，保留褶皱；新图继续独立评审。',
                      createdAtMs: 1_767_600_000_000,
                      targets: [
                        {
                          id: ACCEPTANCE_CONTINUOUS_TARGET.targetId,
                          revisionId: ACCEPTANCE_CONTINUOUS_TARGET.targetRevisionId,
                          assetVersionId: continuousAsset.id,
                          anchor: { kind: 'image_rect', x: 0.2, y: 0.2, width: 0.3, height: 0.3 },
                          availability:
                            id === 'RVW-28'
                              ? { kind: 'needs_confirmation', reasons: ['source_changed'] }
                              : { kind: 'ready' },
                        },
                      ],
                      historyRef: null,
                    },
                  ],
            },
            commandId: 'acceptance-command-c',
            payloadDigest: 'cd'.repeat(32),
            changes: [],
            evidence: [],
          },
    sourceChecks:
      id === 'RVW-28'
        ? [
            {
              assetVersionId: continuousAsset.id,
              checkedAtMs: 1_767_600_001_000,
              status: 'changed',
            },
          ]
        : [],
    projection: {
      actionable: emptyCurrent || id === 'RVW-28' ? [] : [ACCEPTANCE_CONTINUOUS_TARGET.targetId],
      needsConfirmation: id === 'RVW-28' ? [ACCEPTANCE_CONTINUOUS_TARGET.targetId] : [],
    },
    recovery: [],
    migration,
    capabilities: { continuousEditing: true, usageImport: true, migration: migration !== null },
  }
}

export function acceptanceContinuousArchivePlan(id: string): ReviewArchivePlan {
  const basis = id === 'RVW-27' ? ACCEPTANCE_CONTINUOUS_OLD_TARGET : ACCEPTANCE_CONTINUOUS_TARGET
  const groups = [
    {
      basis: { kind: 'unknown' as const },
      targets:
        id === 'RVW-26'
          ? [
              basis,
              {
                feedbackId: 'acceptance-continuous-feedback-shared',
                textRevisionId: 'acceptance-continuous-text-shared',
                targetId: 'acceptance-continuous-target-2',
                targetRevisionId: 'acceptance-continuous-target-revision-2',
              },
            ]
          : [basis],
    },
  ]
  return {
    expectedSnapshotId: 'acceptance-snapshot-c',
    groups,
    removed: id === 'RVW-27' ? [] : [basis],
    retained:
      id === 'RVW-27'
        ? [{ basis, current: ACCEPTANCE_CONTINUOUS_TARGET, disposition: 'retain_later_edit' }]
        : [],
    alreadyCovered: [],
  }
}

export function acceptanceContinuousHistory(): ReviewHistoryView {
  return {
    selector: { kind: 'archive', archiveId: 'acceptance-archive-b' },
    entries: [
      {
        snapshot: { snapshotId: 'acceptance-snapshot-b', blake3: 'bc'.repeat(32) },
        feedback: [
          {
            id: ACCEPTANCE_CONTINUOUS_OLD_TARGET.feedbackId,
            textRevisionId: ACCEPTANCE_CONTINUOUS_OLD_TARGET.textRevisionId,
            text: '袖口收紧',
            createdAtMs: 1_767_599_000_000,
            targets: [
              {
                id: ACCEPTANCE_CONTINUOUS_OLD_TARGET.targetId,
                revisionId: ACCEPTANCE_CONTINUOUS_OLD_TARGET.targetRevisionId,
                assetVersionId: continuousAsset.id,
                anchor: { kind: 'image_rect', x: 0.2, y: 0.2, width: 0.3, height: 0.3 },
                availability: { kind: 'ready' },
              },
            ],
            historyRef: null,
          },
        ],
        assets: [continuousAsset],
        evidence: [
          {
            assetVersionId: continuousAsset.id,
            capability: {
              kind: 'image',
              base: { blake3: '21'.repeat(32), sizeBytes: 1234, width: 560, height: 373 },
              annotated: {
                blake3: '22'.repeat(32),
                sizeBytes: 1456,
                width: 560,
                height: 373,
              },
              annotations: [{ ordinal: 1, key: ACCEPTANCE_CONTINUOUS_OLD_TARGET }],
            },
          },
        ],
        selected: [ACCEPTANCE_CONTINUOUS_OLD_TARGET],
      },
    ],
    legacy: null,
    limitations: ['background_only'],
    restoreActions: [ACCEPTANCE_CONTINUOUS_OLD_TARGET],
  }
}

export function acceptanceContinuousRestorePlan(): ReviewRestorePlan {
  return {
    expectedSnapshotId: 'acceptance-snapshot-c',
    restored: [],
    conflicts: [ACCEPTANCE_CONTINUOUS_OLD_TARGET],
    coverageReversals: [],
    requiresSourceCheck: [],
  }
}

export function acceptanceContinuousMigration(): ReviewMigrationInspection {
  const completed = {
    projectId: ACCEPTANCE_PROJECT_SNAPSHOT.projectId,
    reviewStreamId: 'acceptance-continuous-stream',
    reviewRoundId: 'acceptance-legacy-round',
    production: null,
    previousCompletedRoundId: null,
    createdAtMs: 1_767_500_000_000,
    completedAtMs: 1_767_500_001_000,
    assets: [continuousAsset],
    feedback: [
      {
        id: 'acceptance-legacy-feedback',
        text: '旧记录只作为历史保留',
        createdAtMs: 1_767_500_000_000,
        targets: [{ assetVersionId: continuousAsset.id, anchor: { kind: 'asset' as const } }],
      },
    ],
    outcomes: [
      {
        assetVersionId: continuousAsset.id,
        kind: 'revise' as const,
        feedbackIds: ['acceptance-legacy-feedback'],
        failure: null,
      },
    ],
  }
  return {
    legacyProtocol: 'viewer.review/1',
    indexDigest: '31'.repeat(32),
    inspectionDigest: '32'.repeat(32),
    legacyRecords: [
      {
        streamId: 'acceptance-continuous-stream',
        roundId: completed.reviewRoundId,
        protocol: 'viewer.review/1',
        isDraft: false,
        blake3: '33'.repeat(32),
      },
    ],
    activeDraft: null,
    completedCandidates: [completed],
    limitations: ['background_only', 'legacy_evidence_absent', 'usage_unconfirmed'],
  }
}

export function acceptancePreparedContinuousCommand(
  command: PreparedReviewCommand['command'],
): PreparedReviewCommand {
  return {
    context: {
      projectId: ACCEPTANCE_PROJECT_SNAPSHOT.projectId,
      streamId: 'acceptance-continuous-stream',
      production: null,
    },
    commandId: 'acceptance-prepared-command',
    expectedSnapshotId: 'acceptance-snapshot-c',
    payloadDigest: '41'.repeat(32),
    generated: {
      snapshotId: 'acceptance-snapshot-d',
      feedbackId: 'acceptance-feedback-d',
      textRevisionId: 'acceptance-text-d',
      archiveId: 'acceptance-archive-d',
      targets: [],
      migration: [],
      createdAtMs: 1_767_600_002_000,
    },
    usageSelections: [],
    command,
  }
}

export const ACCEPTANCE_CONTINUOUS_ASSETS = [continuousAsset, replacedContinuousAsset] as const
