import type {
  BrowserFile,
  ContentFolderCard,
  FolderTreeItem,
  FolderWorkspace,
  ImageRepresentation,
  ImageRepresentationRequest,
  ProjectSnapshot,
} from '../api/types'

const FIXTURE_MODIFIED_NS = '1767600000000000000'
const IMAGE_WIDTH = 560
const IMAGE_HEIGHT = 373

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
  }
})

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
    url: `/${encodeURIComponent(file.name)}`,
    width: file.imageMetadata?.width ?? IMAGE_WIDTH,
    height: file.imageMetadata?.height ?? IMAGE_HEIGHT,
    backend: 'image_io',
  }
}

export function acceptanceFile(entityId: string): BrowserFile {
  const file = [...ACCEPTANCE_FILES, ...ACCEPTANCE_TEXT_FILES, ACCEPTANCE_UNSUPPORTED_FILE].find(
    (candidate) => candidate.entityId === entityId,
  )
  if (file === undefined) throw new Error(`Unknown Viewer acceptance file: ${entityId}`)
  return file
}
