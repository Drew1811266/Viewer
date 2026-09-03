import type {
  ReviewAnchor,
  ReviewFeedbackTargetInput,
  ReviewMemberSnapshot,
  ReviewSessionSnapshot,
  ViewerCommandError,
} from '../../api/types'
import type { AcceptanceBridgeOverrides } from '../acceptanceBridge'
import {
  ACCEPTANCE_FILES,
  ACCEPTANCE_REVIEW_MEMBERS,
  acceptanceReviewSnapshot,
} from '../acceptanceFixtures'

const OPINIONS = [
  '领口需要收窄，保留原有材质。',
  '右袖边缘有伪影，请重画这一段。',
  '左侧接缝需要拉直。',
  '裤脚颜色请与衣身统一。',
  '右下区域需要改为更平滑的椭圆轮廓。',
]
const ANCHORS: ReviewAnchor[] = [
  { kind: 'image_rect', x: 0.4, y: 0.06, width: 0.2, height: 0.18 },
  {
    kind: 'image_stroke',
    points: [
      { x: 0.71, y: 0.32 },
      { x: 0.75, y: 0.42 },
      { x: 0.79, y: 0.57 },
      { x: 0.75, y: 0.64 },
      { x: 0.7, y: 0.52 },
    ],
  },
  { kind: 'image_point', x: 0.36, y: 0.57 },
  {
    kind: 'image_arrow',
    tail: { x: 0.5, y: 0.78 },
    head: { x: 0.66, y: 0.62 },
  },
  { kind: 'image_ellipse', x: 0.76, y: 0.7, width: 0.2, height: 0.22 },
]
const FIRST_ENTITY = 'acceptance-image-01'
const FIRST_VERSION = 'acceptance-asset-version-1'
const UNSAVED_TEXT = '请保留这里的布料纹理，再修正边缘。'
// Observe the snapshot returned by the real coordinator's acceptance bridge call.
// A dispatched pointer gesture alone is not evidence of a committed replacement.
let brushRedrawSnapshot: ReviewSessionSnapshot | null = null

function annotatedSnapshot(count = 4): ReviewSessionSnapshot {
  return acceptanceReviewSnapshot({
    members: ACCEPTANCE_REVIEW_MEMBERS.map((member, index) => ({
      ...member,
      feedbackItems: index === 0 ? count : 0,
    })),
    feedback: OPINIONS.slice(0, count).map((text, index) => ({
      feedbackId: `acceptance-annotation-${index + 1}`,
      text,
      createdAtMs: 1_767_600_000_000 + index,
      targetEntityIds: [FIRST_ENTITY],
      targetCount: 1,
      targets: [
        {
          assetVersionId: FIRST_VERSION,
          entityId: FIRST_ENTITY,
          anchor: ANCHORS[index] ?? { kind: 'asset' },
        },
      ],
    })),
    unreviewable: [],
    counts: { total: 6, feedbackItems: count, revise: count > 0 ? 1 : 0, unreviewable: 0, pass: 0 },
  })
}

/** In-memory bridge only; all UI actions still go through the real App/coordinator. */
export function workbenchBridgeOverrides(id: string): AcceptanceBridgeOverrides | null {
  if (!(id in REVIEW_WORKBENCH_SCENES)) return null
  const members: ReadonlyArray<ReviewMemberSnapshot> =
    id === 'RVW-16'
      ? ACCEPTANCE_FILES.map((file, index) => ({
          assetVersionId: `acceptance-asset-version-${index + 1}`,
          entityId: file.entityId,
          relativePath: file.relativePath,
          displayName: file.name,
          kind: 'image',
          feedbackItems: 0,
        }))
      : ACCEPTANCE_REVIEW_MEMBERS
  let snapshot =
    id === 'RVW-16'
      ? acceptanceReviewSnapshot({
          phase: 'idle',
          reviewStreamId: null,
          reviewRoundId: null,
          revision: 0,
          members: [],
          feedback: [],
          unreviewable: [],
          counts: { total: 0, feedbackItems: 0, revise: 0, unreviewable: 0, pass: 0 },
        })
      : annotatedSnapshot(isExtendedMarkupScene(id) ? 5 : 4)
  if (id === 'RVW-20') brushRedrawSnapshot = snapshot
  function save(text: string, targets: ReadonlyArray<ReviewFeedbackTargetInput>) {
    const first = targets[0]
    if (first === undefined || !text.trim()) throw new Error('Invalid acceptance annotation')
    const nextCount = snapshot.feedback.length + 1
    snapshot = {
      ...annotatedSnapshot(0),
      revision: snapshot.revision + 1,
      feedback: [
        ...snapshot.feedback,
        {
          feedbackId: `acceptance-annotation-${nextCount}`,
          text,
          createdAtMs: 1_767_600_000_000 + nextCount - 1,
          targetEntityIds: [first.entityId],
          targetCount: 1,
          targets: [
            { assetVersionId: FIRST_VERSION, entityId: first.entityId, anchor: first.anchor },
          ],
        },
      ],
      members: members.map((member, index) => ({
        ...member,
        feedbackItems: index === 0 ? nextCount : 0,
      })),
      counts: {
        total: members.length,
        feedbackItems: nextCount,
        revise: 1,
        unreviewable: 0,
        pass: 0,
      },
    }
    return snapshot
  }
  return {
    async reviewStatus() {
      return snapshot
    },
    async reviewStartWithFeedback({ text, targets }) {
      return save(text, targets)
    },
    async reviewAddFeedback({ text, targets }) {
      if (id === 'RVW-18')
        throw {
          code: 'review_save_failed',
          category: 'environment',
          userMessage: '意见尚未保存，请重试。',
          retryable: true,
          taskId: null,
          itemId: null,
        } satisfies ViewerCommandError
      return save(text, targets)
    },
    async reviewReplaceFeedbackAnchor({ feedbackId, target }) {
      snapshot = {
        ...snapshot,
        revision: snapshot.revision + 1,
        feedback: snapshot.feedback.map((feedback) =>
          feedback.feedbackId !== feedbackId
            ? feedback
            : {
                ...feedback,
                targets: feedback.targets.map((old) => ({ ...old, anchor: target.anchor })),
              },
        ),
      }
      if (id === 'RVW-20') brushRedrawSnapshot = snapshot
      return snapshot
    },
  }
}

export const REVIEW_WORKBENCH_SCENES: Readonly<Record<string, () => boolean>> = {
  'RVW-16': () => firstAnnotation(),
  'RVW-17': () => fourAnnotationsReady(),
  'RVW-18': () => draftReady('error'),
  'RVW-19': () => rectangleEditReady(),
  'RVW-20': () => brushRedrawReady(),
  'RVW-21': () =>
    previewReady() &&
    document.querySelector('.review-outside-scope-notice') !== null &&
    document.querySelector('.annotation-toolbar') === null,
  'RVW-22': () => draftReady('leave'),
  'RVW-23': () => {
    const badges = document.querySelectorAll('.image-cell .review-feedback-count-badge')
    return (
      document.querySelector('.image-preview') === null &&
      badges.length === 1 &&
      badges[0]?.textContent?.trim() === '返工 · 4 条' &&
      noPendingWork()
    )
  },
  'RVW-24': () => fourAnnotationsReady(),
  'RVW-33': () => compositeMagnifierReady(),
  'RVW-34': () => markupMenuReady('wide'),
  'RVW-35': () => markupMenuReady('compact'),
  'RVW-36': () => selectedExtendedShapeReady('image_arrow', 2),
  'RVW-37': () => selectedExtendedShapeReady('image_ellipse', 4),
}

function isExtendedMarkupScene(id: string) {
  return id === 'RVW-34' || id === 'RVW-35' || id === 'RVW-36' || id === 'RVW-37'
}

function noPendingWork() {
  return (
    document.querySelector('.task-bar, [aria-busy="true"], [data-thumbnail-state="loading"]') ===
    null
  )
}

function previewReady() {
  const image = document.querySelector<HTMLImageElement>(
    '.image-preview-image[data-visible="true"]',
  )
  return (
    image?.complete === true && image.naturalWidth > 0 && noPendingWork() && workbenchToolbarFits()
  )
}

export function workbenchToolbarFits(): boolean {
  const toolbar = document.querySelector<HTMLElement>('.image-preview > .viewer-toolbar')
  if (toolbar === null) return false
  const bounds = toolbar.getBoundingClientRect()
  const controls = [...toolbar.querySelectorAll('button')].map((button) =>
    button.getBoundingClientRect(),
  )
  return (
    controls.length > 0 &&
    controls.every(
      (rect, index) =>
        rect.width > 0 &&
        rect.height > 0 &&
        rect.left >= bounds.left - 1 &&
        rect.right <= bounds.right + 1 &&
        rect.top >= bounds.top - 1 &&
        rect.bottom <= bounds.bottom + 1 &&
        controls
          .slice(index + 1)
          .every(
            (other) =>
              rect.right <= other.left + 1 ||
              other.right <= rect.left + 1 ||
              rect.bottom <= other.top + 1 ||
              other.bottom <= rect.top + 1,
          ),
    )
  )
}

function fourAnnotationsReady() {
  const stage = document.querySelector('.image-preview-stage')?.getBoundingClientRect()
  const navigation = document.querySelector('.preview-navigation-float')?.getBoundingClientRect()
  return (
    previewReady() &&
    stage !== undefined &&
    navigation !== undefined &&
    navigation.left >= stage.left &&
    navigation.right <= stage.right &&
    document.querySelectorAll('.annotation-marker').length === 4 &&
    document.querySelector('.review-feedback-rail') !== null
  )
}

function compositeMagnifierReady() {
  if (!fourAnnotationsReady()) return false
  const magnifierButton = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
    (button) => button.getAttribute('aria-label') === '放大镜',
  )
  if (magnifierButton?.getAttribute('aria-pressed') !== 'true') {
    clickEnabled('放大镜', 'enable-composite-magnifier')
    return false
  }

  const stage = document.querySelector<HTMLElement>('.image-preview-stage')
  const image = document.querySelector<HTMLImageElement>(
    '.image-preview-image[data-visible="true"]',
  )
  if (stage === null || image === null) return false
  if (stage.dataset.acceptanceMagnifierPointer !== 'true') {
    const bounds = image.getBoundingClientRect()
    if (bounds.width <= 0 || bounds.height <= 0) return false
    stage.dataset.acceptanceMagnifierPointer = 'true'
    stage.dispatchEvent(
      new PointerEvent('pointermove', {
        bubbles: true,
        pointerId: 33,
        clientX: bounds.left + bounds.width * 0.57,
        clientY: bounds.top + bounds.height * 0.09,
      }),
    )
    return false
  }

  return (
    document.querySelector('.image-magnifier[data-visible="true"]') !== null &&
    document.querySelector('.image-magnifier__overlay[data-has-content="true"]') !== null &&
    document.querySelectorAll('.annotation-marker').length === 4 &&
    document.querySelector(
      '.task-bar, .inline-feedback-editor, .review-feedback-rail__error, [aria-busy="true"]',
    ) === null
  )
}

function allMarkupReady() {
  if (!previewReady()) return false
  const anchors = [...document.querySelectorAll<HTMLElement>('[data-anchor-kind]')]
  const kinds = new Set(anchors.map((anchor) => anchor.dataset.anchorKind))
  return (
    anchors.length === 5 &&
    ['image_point', 'image_arrow', 'image_stroke', 'image_rect', 'image_ellipse'].every((kind) =>
      kinds.has(kind),
    ) &&
    document.querySelector('.review-feedback-rail') !== null
  )
}

function markupMenuReady(layout: 'wide' | 'compact') {
  if (!allMarkupReady()) return false
  const menu = document.querySelector('[role="menu"][aria-label="标记工具"]')
  if (menu === null) {
    clickEnabled('标记', `open-markup-menu-${layout}`)
    return false
  }
  const entries = [...menu.querySelectorAll('[role="menuitemradio"]')]
  return (
    entries.length === 5 &&
    ['点', '箭头', '画笔', '矩形', '椭圆'].every((label) =>
      entries.some((entry) => entry.textContent?.includes(label)),
    )
  )
}

function selectedExtendedShapeReady(kind: 'image_arrow' | 'image_ellipse', handleCount: number) {
  if (!allMarkupReady()) return false
  const marker = document.querySelector<HTMLButtonElement>(`[data-anchor-kind="${kind}"]`)
  if (marker === null) return false
  if (marker.dataset.selected !== 'true') {
    if (marker.dataset.acceptanceAction !== `select-${kind}`) {
      marker.dataset.acceptanceAction = `select-${kind}`
      marker.click()
    }
    return false
  }
  const handles = [...marker.querySelectorAll<HTMLButtonElement>('.annotation-geometry-handle')]
  const edgeHandle = handles.at(-1)
  if (edgeHandle !== undefined && document.activeElement !== edgeHandle) edgeHandle.focus()
  return handles.length === handleCount && document.activeElement === edgeHandle
}

function firstAnnotation() {
  if (!previewReady()) return false
  if (document.querySelectorAll('.annotation-marker').length === 1) {
    return (
      document.querySelector('.inline-feedback-editor') === null &&
      document.querySelector('[aria-label="图片评审工具"]') !== null
    )
  }
  if (!createDraft(OPINIONS[0] ?? '', 'first')) return false
  clickEnabled('保存', 'save-first')
  return false
}

function draftReady(mode: 'error' | 'leave') {
  if (!fourAnnotationsReady()) return false
  if (!createDraft(UNSAVED_TEXT, mode)) return false
  if (mode === 'error') {
    clickEnabled('保存', 'save-error')
    return (
      document.querySelector('.annotation-canvas[data-has-draft-anchor="true"]') !== null &&
      document
        .querySelector('.inline-feedback-editor__error')
        ?.textContent?.includes('尚未保存') === true &&
      document.querySelector<HTMLTextAreaElement>('[aria-label="标注意见"]')?.value === UNSAVED_TEXT
    )
  }
  clickEnabled('返回网格', 'leave-draft')
  const dialog = [...document.querySelectorAll('[role="dialog"]')].find((element) =>
    element.textContent?.includes('放弃未保存的标注？'),
  )
  return (
    dialog?.textContent?.includes('放弃未保存的标注？') === true &&
    dialog.contains(document.activeElement)
  )
}

function createDraft(text: string, key: string) {
  const input = document.querySelector<HTMLTextAreaElement>('[aria-label="标注意见"]')
  if (input === null) {
    clickEnabled('矩形', `rectangle-${key}`)
    if (
      document.querySelector('.annotation-canvas-layer')?.getAttribute('data-tool') !== 'rectangle'
    )
      return false
    drawOnce(key, [
      { x: 0.4, y: 0.25 },
      { x: 0.6, y: 0.45 },
    ])
    return false
  }
  if (input.value !== text) {
    const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set
    setter?.call(input, text)
    input.dispatchEvent(new Event('input', { bubbles: true }))
    input.dispatchEvent(new Event('change', { bubbles: true }))
    return false
  }
  return !input.disabled
}

function rectangleEditReady() {
  if (!fourAnnotationsReady()) return false
  clickEnabled('调整意见 1 区域', 'select-rectangle')
  const handle = document.querySelector<HTMLElement>('[aria-label="调整意见 1 右下角"]')
  if (handle === null) return false
  handle.focus()
  return (
    document.querySelectorAll('.annotation-rect-handle').length === 4 &&
    document.activeElement === handle
  )
}

function brushRedrawReady() {
  if (!fourAnnotationsReady()) return false
  const canvas = document.querySelector<HTMLElement>('.annotation-canvas')
  const tool = document.querySelector('.annotation-canvas-layer')?.getAttribute('data-tool')
  if (canvas?.dataset.acceptanceDrawing !== 'redraw') {
    clickEnabled('重绘意见 2', 'select-brush')
    if (tool !== 'brush') return false
    drawOnce('redraw', [
      { x: 0.7, y: 0.3 },
      { x: 0.8, y: 0.5 },
      { x: 0.73, y: 0.65 },
    ])
    return false
  }
  return (
    tool === 'browse' &&
    brushReplacementCommitted() &&
    !canvas.hasAttribute('data-has-candidate') &&
    !canvas.hasAttribute('data-has-draft-anchor') &&
    document.querySelector('.inline-feedback-editor, .review-feedback-rail__error') === null &&
    document
      .querySelector('.annotation-marker[data-selected="true"]')
      ?.getAttribute('aria-label') === `意见 2：${OPINIONS[1]}` &&
    document.querySelector(
      '.review-feedback-rail [aria-label="意见 2"][data-selected="true"] strong',
    )?.textContent === OPINIONS[1]
  )
}

function brushReplacementCommitted() {
  const original = annotatedSnapshot().feedback[1]
  const feedback = brushRedrawSnapshot?.feedback.find(
    (item) => item.feedbackId === original?.feedbackId,
  )
  const target = feedback?.targets[0]
  return (
    original !== undefined &&
    brushRedrawSnapshot?.feedback.length === 4 &&
    feedback?.text === original.text &&
    feedback.createdAtMs === original.createdAtMs &&
    feedback.targets.length === 1 &&
    target?.assetVersionId === FIRST_VERSION &&
    target.entityId === FIRST_ENTITY &&
    target.anchor.kind === 'image_stroke' &&
    JSON.stringify(target.anchor) !== JSON.stringify(ANCHORS[1])
  )
}

function clickEnabled(name: string, action: string) {
  const button = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
    (element) =>
      element.getAttribute('aria-label') === name || element.textContent?.trim() === name,
  )
  if (button === undefined || button.disabled || button.dataset.acceptanceAction === action) return
  button.dataset.acceptanceAction = action
  button.click()
}

function drawOnce(key: string, points: ReadonlyArray<{ x: number; y: number }>) {
  const canvas = document.querySelector<HTMLCanvasElement>(
    '.annotation-canvas[data-interactive="true"]',
  )
  const image = document.querySelector<HTMLImageElement>('.image-preview-image')
  if (canvas === null || image === null || canvas.dataset.acceptanceDrawing === key) return
  const rect = image.getBoundingClientRect()
  if (rect.width <= 0 || rect.height <= 0) return
  canvas.dataset.acceptanceDrawing = key
  // Synthetic pointer events have no native pointer registration. Skip only capture;
  // the real canvas handlers still perform projection, validation and persistence.
  const capture = canvas.setPointerCapture
  canvas.setPointerCapture = () => undefined
  try {
    points.forEach((point, index) => {
      canvas.dispatchEvent(
        new PointerEvent(index === 0 ? 'pointerdown' : 'pointermove', {
          bubbles: true,
          pointerId: 1,
          button: 0,
          buttons: 1,
          clientX: rect.left + rect.width * point.x,
          clientY: rect.top + rect.height * point.y,
        }),
      )
    })
    const last = points.at(-1)
    if (last !== undefined)
      canvas.dispatchEvent(
        new PointerEvent('pointerup', {
          bubbles: true,
          pointerId: 1,
          button: 0,
          clientX: rect.left + rect.width * last.x,
          clientY: rect.top + rect.height * last.y,
        }),
      )
  } finally {
    canvas.setPointerCapture = capture
  }
}
