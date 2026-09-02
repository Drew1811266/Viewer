import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { ReviewAnchor } from '../../api/types'
import { defined } from '../../defined'
import { REVIEW_WORKBENCH_SCENES, workbenchBridgeOverrides } from './reviewWorkbenchScenes'

const redrawAnchor: ReviewAnchor = {
  kind: 'image_stroke',
  points: [
    { x: 0.7, y: 0.3 },
    { x: 0.8, y: 0.5 },
    { x: 0.73, y: 0.65 },
  ],
}

function redrawScene() {
  const bridge = defined(workbenchBridgeOverrides('RVW-20'), 'redraw bridge')
  const ready = defined(REVIEW_WORKBENCH_SCENES['RVW-20'], 'redraw readiness')
  const { container } = render(
    <section className="image-preview">
      <header className="viewer-toolbar">
        <button type="button">浏览</button>
      </header>
      <div className="image-preview-stage">
        <img className="image-preview-image" data-visible="true" alt="fixture" />
        <div className="annotation-canvas-layer" data-tool="browse">
          <canvas className="annotation-canvas" data-acceptance-drawing="redraw" />
          <button type="button" className="annotation-marker">
            1
          </button>
          <button
            type="button"
            className="annotation-marker"
            data-anchor-kind="image_stroke"
            data-selected="true"
            aria-label="意见 2：右袖边缘有伪影，请重画这一段。"
          >
            2
          </button>
          <button type="button" className="annotation-marker">
            3
          </button>
          <button type="button" className="annotation-marker">
            4
          </button>
        </div>
        <nav className="preview-navigation-float" />
      </div>
      <aside className="review-feedback-rail">
        <ol>
          <li aria-label="意见 2" data-selected="true">
            <strong>右袖边缘有伪影，请重画这一段。</strong>
            <button type="button" aria-label="重绘意见 2">
              重绘
            </button>
          </li>
        </ol>
      </aside>
    </section>,
  )
  const query = <T extends Element>(selector: string) =>
    defined(container.querySelector<T>(selector), selector)
  const image = query<HTMLImageElement>('img')
  Object.defineProperties(image, { complete: { value: true }, naturalWidth: { value: 560 } })
  query<HTMLElement>('header').getBoundingClientRect = () => new DOMRect(0, 0, 1024, 50)
  query<HTMLElement>('header button').getBoundingClientRect = () => new DOMRect(10, 10, 60, 30)
  query<HTMLElement>('.image-preview-stage').getBoundingClientRect = () =>
    new DOMRect(0, 50, 704, 670)
  query<HTMLElement>('nav').getBoundingClientRect = () => new DOMRect(250, 650, 150, 40)
  const canvas = query<HTMLCanvasElement>('canvas')
  const layer = query<HTMLElement>('.annotation-canvas-layer')
  async function commit(anchor = redrawAnchor, feedbackId = 'acceptance-annotation-2') {
    return defined(
      bridge.reviewReplaceFeedbackAnchor,
      'replace anchor',
    )({
      sessionId: 'acceptance-session',
      generation: 1,
      reviewRoundId: 'acceptance-review-round',
      expectedRevision: 4,
      feedbackId,
      target: { entityId: 'acceptance-image-01', anchor },
    })
  }
  return { ready, canvas, layer, query, commit }
}

function compositeMagnifierScene() {
  const ready = defined(REVIEW_WORKBENCH_SCENES['RVW-33'], 'composite magnifier readiness')
  const { container } = render(
    <section className="image-preview">
      <header className="viewer-toolbar">
        <button type="button" aria-label="放大镜" aria-pressed="true">
          放大镜
        </button>
      </header>
      <div className="image-preview-stage" data-acceptance-magnifier-pointer="true">
        <img className="image-preview-image" data-visible="true" alt="fixture" />
        <div className="annotation-canvas-layer">
          {[1, 2, 3, 4].map((ordinal) => (
            <button key={ordinal} type="button" className="annotation-marker">
              {ordinal}
            </button>
          ))}
        </div>
        <div className="image-magnifier" data-visible="true">
          <canvas className="image-magnifier__overlay" data-has-content="true" />
        </div>
        <nav className="preview-navigation-float" />
      </div>
      <aside className="review-feedback-rail" />
    </section>,
  )
  const query = <T extends Element>(selector: string) =>
    defined(container.querySelector<T>(selector), selector)
  const image = query<HTMLImageElement>('img')
  Object.defineProperties(image, { complete: { value: true }, naturalWidth: { value: 560 } })
  query<HTMLElement>('header').getBoundingClientRect = () => new DOMRect(0, 0, 1024, 50)
  query<HTMLElement>('header button').getBoundingClientRect = () => new DOMRect(10, 10, 80, 30)
  query<HTMLElement>('.image-preview-stage').getBoundingClientRect = () =>
    new DOMRect(0, 50, 704, 670)
  image.getBoundingClientRect = () => new DOMRect(40, 100, 624, 416)
  query<HTMLElement>('nav').getBoundingClientRect = () => new DOMRect(250, 650, 150, 40)
  return { ready, query }
}

describe('brush-redraw acceptance readiness', () => {
  it('accepts the committed replacement in Browse with its original identity and text', async () => {
    const scene = redrawScene()
    const snapshot = await scene.commit()
    expect(snapshot.feedback[1]).toMatchObject({
      feedbackId: 'acceptance-annotation-2',
      text: '右袖边缘有伪影，请重画这一段。',
      createdAtMs: 1_767_600_000_001,
      targets: [{ anchor: redrawAnchor }],
    })
    expect(scene.ready()).toBe(true)
  })

  it('does not treat a dispatched gesture or unchanged saved anchor as a completed redraw', async () => {
    const scene = redrawScene()
    scene.layer.dataset.tool = 'brush'
    expect(scene.ready()).toBe(false)
    scene.layer.dataset.tool = 'browse'
    expect(scene.ready()).toBe(false)
    await scene.commit({
      kind: 'image_stroke',
      points: [
        { x: 0.71, y: 0.32 },
        { x: 0.75, y: 0.42 },
        { x: 0.79, y: 0.57 },
        { x: 0.75, y: 0.64 },
        { x: 0.7, y: 0.52 },
      ],
    })
    expect(scene.ready()).toBe(false)
  })

  it('requires Brush before drawing and does not retry a committed gesture', async () => {
    const scene = redrawScene()
    delete scene.canvas.dataset.acceptanceDrawing
    expect(scene.ready()).toBe(false)
    expect(scene.canvas.dataset.acceptanceDrawing).toBeUndefined()
    scene.canvas.dataset.acceptanceDrawing = 'redraw'
    await scene.commit()
    expect(scene.ready()).toBe(true)
    expect(scene.ready()).toBe(true)
  })

  it('rejects an anchor replacement for a different feedback identity', async () => {
    const scene = redrawScene()
    await scene.commit(redrawAnchor, 'acceptance-annotation-3')
    expect(scene.ready()).toBe(false)
  })

  it('waits for both draft geometry and all pending UI work to clear', async () => {
    const scene = redrawScene()
    await scene.commit()
    expect(scene.ready()).toBe(true)
    for (const attribute of ['data-has-candidate', 'data-has-draft-anchor']) {
      scene.canvas.setAttribute(attribute, 'true')
      expect(scene.ready()).toBe(false)
      scene.canvas.removeAttribute(attribute)
    }
    const pending = document.createElement('div')
    scene.query('aside').append(pending)
    for (const className of ['task-bar', 'inline-feedback-editor', 'review-feedback-rail__error']) {
      pending.className = className
      expect(scene.ready()).toBe(false)
    }
    pending.remove()
    expect(scene.ready()).toBe(true)
  })
})

describe('composite magnifier acceptance readiness', () => {
  it('samples the first rectangle edge and ordinal inside the lens', () => {
    const scene = compositeMagnifierScene()
    const stage = scene.query<HTMLElement>('.image-preview-stage')
    delete stage.dataset.acceptanceMagnifierPointer
    const sampledPoints: Array<{ x: number; y: number }> = []
    stage.addEventListener(
      'pointermove',
      (event) => {
        sampledPoints.push({ x: event.clientX, y: event.clientY })
      },
      { once: true },
    )

    expect(scene.ready()).toBe(false)
    expect(sampledPoints).toHaveLength(1)
    expect(sampledPoints[0]?.x).toBeCloseTo(40 + 624 * 0.57)
    expect(sampledPoints[0]?.y).toBeCloseTo(100 + 416 * 0.09)
  })

  it('requires the visible lens, painted overlay, ordinary annotations, and idle workbench', () => {
    const scene = compositeMagnifierScene()
    expect(scene.ready()).toBe(true)

    const overlay = scene.query<HTMLCanvasElement>('.image-magnifier__overlay')
    overlay.removeAttribute('data-has-content')
    expect(scene.ready()).toBe(false)
    overlay.dataset.hasContent = 'true'

    const lens = scene.query<HTMLElement>('.image-magnifier')
    lens.removeAttribute('data-visible')
    expect(scene.ready()).toBe(false)
    lens.dataset.visible = 'true'

    scene.query('.annotation-marker').remove()
    expect(scene.ready()).toBe(false)
  })

  it('rejects pending work and an open inline editor', () => {
    const scene = compositeMagnifierScene()
    const pending = document.createElement('div')
    scene.query('.image-preview-stage').append(pending)
    pending.className = 'task-bar'
    expect(scene.ready()).toBe(false)
    pending.className = 'inline-feedback-editor'
    expect(scene.ready()).toBe(false)
  })
})
