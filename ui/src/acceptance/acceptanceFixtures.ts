import type {
  BrowserFile,
  FolderTreeItem,
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
  {
    entityId: 'acceptance-folder-documents',
    parentEntityId: null,
    relativePath: '文档',
    name: '文档',
    marker: { reviewState: null, favorite: false },
  },
]

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
