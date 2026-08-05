import { useCallback, useEffect, useRef } from 'react'
import type {
  BrowserFile,
  ImageRepresentationRequest,
  SelectionInfo,
  TextEncoding,
  TextPreview as TextPreviewDto,
} from '../../api/types'
import CompareWorkspace from '../../components/CompareWorkspace'
import ImagePreview from '../../components/ImagePreview'
import InfoOverlay from '../../components/InfoOverlay'
import RadialFileMenu from '../../components/RadialFileMenu'
import { buildRadialMenuModel, type RadialMenuContext } from '../../components/radialMenuModel'
import TextPreview from '../../components/TextPreview'
import UnsupportedFilePreview from '../../components/UnsupportedFilePreview'
import { defined } from '../../defined'
import type { AcceptanceSceneRegistry } from '../AcceptanceApp'
import {
  ACCEPTANCE_FILES,
  ACCEPTANCE_TEXT_FILES,
  ACCEPTANCE_UNSUPPORTED_FILE,
  imageRepresentation,
} from '../acceptanceFixtures'
import type { AcceptanceRequest } from '../acceptanceRequest'
import AcceptanceProductScene from './sceneHarness'

const PREVIEW_FILE = namedFile(ACCEPTANCE_FILES, '商品-02.jpg')
const MARKDOWN_FILE = namedFile(ACCEPTANCE_TEXT_FILES, 'sample.md')
const PLAIN_FILE = namedFile(ACCEPTANCE_TEXT_FILES, 'plain.txt')
const GB18030_FILE = namedFile(ACCEPTANCE_TEXT_FILES, 'gb18030.txt')
const LARGE_TEXT_FILE = namedFile(ACCEPTANCE_TEXT_FILES, 'large.txt')

const BASE_RADIAL_CONTEXT: RadialMenuContext = {
  selectedCount: 3,
  selectedImageCount: 3,
  previewEnabled: true,
  readOnly: false,
  busy: false,
  compareContextAvailable: true,
  commonReview: null,
  commonFavorite: false,
}

export const VIEWING_SCENES: AcceptanceSceneRegistry = {
  'RAD-01': (props) => <RadialScene {...props} state="click" />,
  'RAD-02': (props) => <RadialScene {...props} state="gesture" />,
  'RAD-03': (props) => <RadialScene {...props} state="mark" />,
  'RAD-04': (props) => <RadialScene {...props} state="organize" />,
  'RAD-05': (props) => <RadialScene {...props} state="disabled" />,
  'RAD-06': (props) => <RadialScene {...props} state="readonly" />,
  'RAD-07': (props) => <RadialScene {...props} state="keyboard" />,
  'PRE-01': (props) => <PreviewScene {...props} state="fit" />,
  'PRE-02': (props) => <PreviewScene {...props} state="original" />,
  'PRE-03': (props) => <PreviewScene {...props} state="zoom" />,
  'PRE-04': (props) => <PreviewScene {...props} state="rotate" />,
  'PRE-05': (props) => <PreviewScene {...props} state="loading" />,
  'PRE-06': (props) => <PreviewScene {...props} state="error" />,
  'PRE-07': (props) => <PreviewScene {...props} state="navigation" />,
  'COM-01': () => <CompareScene count={2} />,
  'COM-02': () => <CompareScene count={3} />,
  'COM-03': () => <CompareScene count={4} />,
  'COM-04': () => <CompareScene count={20} />,
  'DOC-01': () => <TextScene state="markdown" />,
  'DOC-02': () => <TextScene state="plain" />,
  'DOC-03': () => <TextScene state="encoding" />,
  'DOC-04': () => <TextScene state="truncated" />,
  'DOC-05': () => <TextScene state="dual" />,
  'DOC-06': () => <UnsupportedFilePreview file={ACCEPTANCE_UNSUPPORTED_FILE} onClose={noOp} />,
  'DOC-07': () => (
    <UnsupportedFilePreview file={ACCEPTANCE_UNSUPPORTED_FILE} unavailable onClose={noOp} />
  ),
  'INF-01': () => <InfoScene aggregate={false} />,
  'INF-02': () => <InfoScene aggregate />,
}

type RadialState = 'click' | 'gesture' | 'mark' | 'organize' | 'disabled' | 'readonly' | 'keyboard'

function RadialScene({ request, state }: { request: AcceptanceRequest; state: RadialState }) {
  const readOnly = state === 'readonly'
  const selectionCount = state === 'disabled' ? 1 : 3
  const context: RadialMenuContext = {
    ...BASE_RADIAL_CONTEXT,
    selectedCount: selectionCount,
    selectedImageCount: selectionCount,
    readOnly,
  }
  const acted = useRef(false)
  useEffect(() => {
    if (acted.current) return
    acted.current = true
    if (state === 'mark' || state === 'keyboard') menuItem('标记')?.click()
    if (state === 'organize') menuItem('整理')?.click()
    if (state === 'disabled') menuItem('并排对比')?.focus()
  }, [state])
  useWorkspaceSelection(selectionCount)
  const ready = useCallback(
    () =>
      workspaceThumbnailsReady() &&
      document.querySelectorAll('.image-cell[aria-selected="true"]').length === selectionCount &&
      document.querySelector('[role="menu"][aria-label="文件操作"]') !== null,
    [selectionCount],
  )
  return (
    <AcceptanceProductScene ready={ready}>
      <RadialFileMenu
        origin={{ x: 220 + (request.width - 220) / 2, y: request.height / 2 }}
        viewport={{ width: request.width, height: request.height }}
        pointerId={state === 'gesture' ? 47 : null}
        selectionCount={selectionCount}
        readOnly={readOnly}
        model={buildRadialMenuModel(context)}
        onAction={noOp}
        onClose={noOp}
      />
    </AcceptanceProductScene>
  )
}

function InfoScene({ aggregate }: { aggregate: boolean }) {
  useWorkspaceSelection(aggregate ? 2 : 1, aggregate ? undefined : PREVIEW_FILE.name)
  const ready = useCallback(() => workspaceThumbnailsReady() && infoInspectorRendered(document), [])
  return (
    <AcceptanceProductScene ready={ready}>
      <InfoOverlay
        files={aggregate ? [PREVIEW_FILE, PLAIN_FILE] : [PREVIEW_FILE]}
        selectionInfo={aggregate ? AGGREGATE_SELECTION_INFO : undefined}
        dimensions={
          aggregate
            ? {}
            : {
                [PREVIEW_FILE.entityId]: PREVIEW_FILE.imageMetadata ?? undefined,
              }
        }
        onClose={noOp}
      />
    </AcceptanceProductScene>
  )
}

export function infoInspectorRendered(root: Document | Element): boolean {
  return root.querySelector('aside.viewer-inspector[aria-label="文件信息"]') !== null
}

function useWorkspaceSelection(count: number, preferredName?: string) {
  const completed = useRef(false)
  useEffect(() => {
    const select = () => {
      const cells = [...document.querySelectorAll<HTMLElement>('.image-cell')]
      if (completed.current) return
      if (cells.length === 0) {
        const folder = document.querySelector<HTMLElement>(
          '[role="treeitem"][aria-label="衣服/A01"]',
        )
        if (folder !== null && folder.dataset.acceptanceAction !== 'open-content-folder') {
          folder.dataset.acceptanceAction = 'open-content-folder'
          folder.click()
        } else {
          const disclosure = document.querySelector<HTMLElement>('[aria-label="展开 衣服"]')
          if (disclosure !== null && disclosure.dataset.acceptanceAction !== 'expand-clothes') {
            disclosure.dataset.acceptanceAction = 'expand-clothes'
            disclosure.click()
          }
        }
        return
      }
      if (!workspaceThumbnailsReady()) return
      const preferred =
        preferredName === undefined
          ? []
          : cells.filter((cell) => cell.getAttribute('aria-label') === preferredName)
      const candidates = [...preferred, ...cells.filter((cell) => !preferred.includes(cell))]
      if (candidates.length < count) return
      const selected = cells.filter((cell) => cell.getAttribute('aria-selected') === 'true')
      if (selected.length === count) {
        completed.current = true
        return
      }
      const next =
        selected.length === 0
          ? candidates[0]
          : candidates.find((cell) => cell.getAttribute('aria-selected') !== 'true')
      next?.dispatchEvent(
        new MouseEvent('click', {
          bubbles: true,
          cancelable: true,
          metaKey: selected.length > 0,
        }),
      )
    }
    const observer = new MutationObserver(select)
    observer.observe(document.body, { attributes: true, childList: true, subtree: true })
    select()
    return () => observer.disconnect()
  }, [count, preferredName])
}

function workspaceThumbnailsReady(): boolean {
  const thumbnails = [...document.querySelectorAll<HTMLElement>('.aspect-thumbnail')]
  return (
    thumbnails.length > 0 &&
    thumbnails.every((thumbnail) => thumbnail.dataset.thumbnailState === 'ready')
  )
}

type PreviewState = 'fit' | 'original' | 'zoom' | 'rotate' | 'loading' | 'error' | 'navigation'

function PreviewScene({ state }: { request: AcceptanceRequest; state: PreviewState }) {
  const acted = useRef(false)
  useEffect(() => {
    if (acted.current) return
    acted.current = true
    if (state === 'original') namedButton('按 100% 显示')?.click()
    if (state === 'zoom') {
      namedButton('放大')?.click()
      namedButton('放大')?.click()
    }
    if (state === 'rotate') namedButton('顺时针旋转')?.click()
  }, [state])
  const requestImage =
    state === 'loading' ? neverImage : state === 'error' ? failedImage : requestAcceptanceImage
  return (
    <ImagePreview
      file={PREVIEW_FILE}
      files={ACCEPTANCE_FILES}
      requestImage={requestImage}
      onNavigate={noOp}
      onClose={noOp}
    />
  )
}

function CompareScene({ count }: { count: number }) {
  return (
    <CompareWorkspace
      files={ACCEPTANCE_FILES.slice(0, count)}
      readOnly={false}
      requestImage={requestAcceptanceImage}
      onEntityIdsChange={noOp}
      onSetReview={noOp}
      onToggleFavorite={noOp}
      onStatus={noOp}
    />
  )
}

type TextState = 'markdown' | 'plain' | 'encoding' | 'truncated' | 'dual'

function TextScene({ state }: { state: TextState }) {
  if (state === 'markdown') {
    return (
      <TextPreview
        files={[MARKDOWN_FILE]}
        requestPreview={requestMarkdown}
        openExternalLink={openExternalLink}
        onClose={noOp}
      />
    )
  }
  if (state === 'encoding') {
    return (
      <TextPreview
        files={[GB18030_FILE]}
        requestPreview={requestEncodingRequired}
        openExternalLink={openExternalLink}
        onClose={noOp}
      />
    )
  }
  if (state === 'truncated') {
    return (
      <TextPreview
        files={[LARGE_TEXT_FILE]}
        requestPreview={requestTruncatedText}
        openExternalLink={openExternalLink}
        onClose={noOp}
      />
    )
  }
  if (state === 'dual') {
    return (
      <TextPreview
        files={[MARKDOWN_FILE, PLAIN_FILE]}
        requestPreview={requestTextByKind}
        openExternalLink={openExternalLink}
        onClose={noOp}
      />
    )
  }
  return (
    <TextPreview
      files={[PLAIN_FILE]}
      requestPreview={requestPlainText}
      openExternalLink={openExternalLink}
      onClose={noOp}
    />
  )
}

async function requestAcceptanceImage(
  file: BrowserFile,
  request: ImageRepresentationRequest,
  signal?: AbortSignal,
) {
  if (signal?.aborted) throw new DOMException('Image request aborted', 'AbortError')
  return imageRepresentation(file, request.kind)
}

function neverImage() {
  return new Promise<never>(() => undefined)
}

async function failedImage(): Promise<never> {
  throw new Error('Acceptance image fixture is unavailable')
}

async function requestMarkdown(file: BrowserFile): Promise<TextPreviewDto> {
  return {
    entityId: file.entityId,
    format: 'markdown',
    plainText: null,
    markdownHtml:
      '<h1>Viewer 视觉验收</h1><p>这是经过净化的 Markdown 内容。</p><p><a href="https://example.com">外部链接</a></p>',
    encoding: 'utf8',
    truncated: false,
  }
}

async function requestPlainText(file: BrowserFile): Promise<TextPreviewDto> {
  return {
    entityId: file.entityId,
    format: 'plain_text',
    plainText: 'Viewer 确定性的纯文本预览。\n文本可以选择、复制并独立滚动。',
    markdownHtml: null,
    encoding: 'utf8',
    truncated: false,
  }
}

function requestEncodingRequired(
  _file: BrowserFile,
  _encoding?: TextEncoding,
): Promise<TextPreviewDto> {
  return Promise.reject({
    code: 'text_encoding_required',
    userMessage: '无法自动识别文本编码。',
  })
}

async function requestTruncatedText(file: BrowserFile): Promise<TextPreviewDto> {
  return {
    ...(await requestPlainText(file)),
    plainText: '大型文本预览内容。\n'.repeat(40),
    truncated: true,
  }
}

function requestTextByKind(file: BrowserFile): Promise<TextPreviewDto> {
  return file.kind === 'markdown' ? requestMarkdown(file) : requestPlainText(file)
}

async function openExternalLink() {}

const AGGREGATE_SELECTION_INFO: SelectionInfo = {
  relativePaths: [PREVIEW_FILE.relativePath, PLAIN_FILE.relativePath, '衣服/A01'],
  totalSize: PREVIEW_FILE.size + PLAIN_FILE.size,
  types: { folders: 1, images: 1, otherFiles: 1 },
  commonReview: { state: 'mixed' },
  commonFavorite: { state: 'common', value: false },
}

function namedFile(files: BrowserFile[], name: string): BrowserFile {
  return defined(
    files.find((file) => file.name === name),
    `Missing Viewer acceptance fixture: ${name}`,
  )
}

function menuItem(name: string): HTMLButtonElement | undefined {
  return [...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')].find(
    (item) => item.textContent?.trim() === name,
  )
}

function namedButton(name: string): HTMLButtonElement | null {
  return document.querySelector<HTMLButtonElement>(`button[aria-label="${name}"]`)
}

function noOp() {}
